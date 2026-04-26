use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use shared::Surebet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for Telegram alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAlertConfig {
    pub min_roi_percent: f64,
    pub max_alerts_per_minute: f64,
    pub only_verified: bool,
    pub only_live: bool,
    pub alert_on_verified_only: bool,
    pub history_size: usize,
    pub batch_window_seconds: u64, // NEW: Batch window for deduplication (default: 60)
    pub batch_max_size: usize,     // NEW: Max surebets per batch (default: 10)
}

impl Default for TelegramAlertConfig {
    fn default() -> Self {
        Self {
            min_roi_percent: 2.0,
            max_alerts_per_minute: 10.0,
            only_verified: false,
            only_live: false,
            alert_on_verified_only: false,
            history_size: 100,
            batch_window_seconds: 60,
            batch_max_size: 10,
        }
    }
}

/// Alert history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertHistoryEntry {
    pub surebet_id: String,
    pub roi_percent: f64,
    pub teams: String,
    pub league: String,
    pub timestamp: DateTime<Utc>,
    pub verified: bool,
    pub is_live: bool,
    pub status: AlertStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertStatus {
    Sent,
    Skipped(String),
    Throttled,
}

/// Alert batch for reducing spam (groups similar surebets within time window)
#[derive(Debug, Clone)]
pub struct AlertBatch {
    pub created_at: Instant,
    pub surebets: Vec<Surebet>,
    pub last_sent: Option<Instant>,
    pub event_key: String, // "sport-league-home-away" for deduplication
}

/// Manages alert history and filtering
pub struct AlertManager {
    config: Arc<Mutex<TelegramAlertConfig>>,
    history: Arc<Mutex<VecDeque<AlertHistoryEntry>>>,
    batches: Arc<Mutex<std::collections::HashMap<String, AlertBatch>>>, // NEW: Event key -> batch
}

impl AlertManager {
    pub fn new(config: TelegramAlertConfig) -> Self {
        let history_size = config.history_size;
        Self {
            config: Arc::new(Mutex::new(config)),
            history: Arc::new(Mutex::new(VecDeque::with_capacity(history_size))),
            batches: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create event key for batching/deduplication
    /// Format: "sport-league-home_team-away_team"
    fn create_event_key(surebet: &Surebet) -> String {
        format!(
            "{}-{}-{}-{}",
            surebet.sport,
            surebet.league.replace(" ", "_"),
            surebet.home_team.replace(" ", "_"),
            surebet.away_team.replace(" ", "_")
        )
    }

    /// Add surebet to batch (deduplicates by event within time window)
    /// Returns true if should send batch immediately
    pub fn add_to_batch(&self, surebet: Surebet) -> bool {
        let config = self.config.lock().clone();
        drop(config); // Release lock early

        let key = Self::create_event_key(&surebet);
        let mut batches = self.batches.lock();

        let batch = batches.entry(key.clone()).or_insert_with(|| AlertBatch {
            created_at: Instant::now(),
            surebets: Vec::new(),
            last_sent: None,
            event_key: key.clone(),
        });

        batch.surebets.push(surebet);

        // Check if should send batch
        let config = self.config.lock();
        let elapsed = batch.created_at.elapsed().as_secs();
        let should_send =
            elapsed >= config.batch_window_seconds || batch.surebets.len() >= config.batch_max_size;

        should_send
    }

    /// Get and clear batch for sending
    pub fn get_batch(&self, event_key: &str) -> Option<AlertBatch> {
        let mut batches = self.batches.lock();
        batches.remove(event_key)
    }

    /// Get all pending batches
    pub fn get_all_pending_batches(&self) -> Vec<AlertBatch> {
        let config = self.config.lock();
        let batches = self.batches.lock();

        batches
            .values()
            .filter(|b| {
                let elapsed = b.created_at.elapsed().as_secs();
                elapsed >= config.batch_window_seconds || b.surebets.len() >= config.batch_max_size
            })
            .cloned()
            .collect()
    }

    /// Check if a surebet should trigger an alert
    pub fn should_alert(&self, surebet: &Surebet) -> Result<(), String> {
        let config = self.config.lock();

        // Check ROI threshold
        if surebet.profit_percent < config.min_roi_percent {
            return Err(format!(
                "ROI {:.2}% below threshold {:.2}%",
                surebet.profit_percent, config.min_roi_percent
            ));
        }

        // Check verified filter
        if config.only_verified && !surebet.verified {
            return Err("Not verified (only_verified=true)".to_string());
        }

        // Check live filter
        if config.only_live && !surebet.is_live {
            return Err("Not live event (only_live=true)".to_string());
        }

        // Check alert_on_verified_only flag (more lenient)
        if config.alert_on_verified_only && !surebet.verified {
            return Err("Not verified (alert_on_verified_only=true)".to_string());
        }

        Ok(())
    }

    /// Record alert in history
    pub fn record_alert(&self, surebet: &Surebet, status: AlertStatus) {
        let entry = AlertHistoryEntry {
            surebet_id: surebet.id.to_string(),
            roi_percent: surebet.profit_percent,
            teams: format!("{} vs {}", surebet.home_team, surebet.away_team),
            league: surebet.league.clone(),
            timestamp: Utc::now(),
            verified: surebet.verified,
            is_live: surebet.is_live,
            status,
        };

        let mut history = self.history.lock();
        let max_size = self.config.lock().history_size;
        history.push_front(entry);
        while history.len() > max_size {
            history.pop_back();
        }
    }

    /// Get alert history
    pub fn get_history(&self, limit: usize) -> Vec<AlertHistoryEntry> {
        let history = self.history.lock();
        history.iter().take(limit).cloned().collect()
    }

    /// Get alert statistics
    pub fn get_stats(&self) -> AlertStats {
        let history = self.history.lock();
        let total = history.len();
        let sent = history
            .iter()
            .filter(|e| e.status == AlertStatus::Sent)
            .count();
        let throttled = history
            .iter()
            .filter(|e| e.status == AlertStatus::Throttled)
            .count();
        let skipped = total - sent - throttled;

        let avg_roi = if !history.is_empty() {
            history.iter().map(|e| e.roi_percent).sum::<f64>() / total as f64
        } else {
            0.0
        };

        AlertStats {
            total_alerts: total,
            sent: sent,
            throttled,
            skipped,
            avg_roi,
            sent_in_last_hour: count_sent_in_last_hour(&history),
            sent_in_last_minute: count_sent_in_last_minute(&history),
        }
    }

    /// Update configuration
    pub fn update_config(&self, config: TelegramAlertConfig) {
        *self.config.lock() = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> TelegramAlertConfig {
        self.config.lock().clone()
    }
}

/// Alert statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertStats {
    pub total_alerts: usize,
    pub sent: usize,
    pub throttled: usize,
    pub skipped: usize,
    pub avg_roi: f64,
    pub sent_in_last_hour: usize,
    pub sent_in_last_minute: usize,
}

fn count_sent_in_last_hour(history: &VecDeque<AlertHistoryEntry>) -> usize {
    let cutoff = Utc::now() - chrono::Duration::hours(1);
    history
        .iter()
        .filter(|e| e.timestamp > cutoff && e.status == AlertStatus::Sent)
        .count()
}

fn count_sent_in_last_minute(history: &VecDeque<AlertHistoryEntry>) -> usize {
    let cutoff = Utc::now() - chrono::Duration::minutes(1);
    history
        .iter()
        .filter(|e| e.timestamp > cutoff && e.status == AlertStatus::Sent)
        .count()
}

/// Format surebet for Telegram message with enhanced details
pub fn format_surebet_alert(surebet: &Surebet) -> String {
    let time_str = surebet
        .start_time
        .map(|t| t.format("%d.%m %H:%M UTC").to_string())
        .unwrap_or_else(|| "N/A".to_string());

    // Calculate profit amount (first leg payout minus total stake)
    let profit_amount = surebet
        .legs
        .first()
        .map(|leg| leg.payout - surebet.total_stake)
        .unwrap_or(0.0);

    let mut msg = format!(
        "🔥 <b>СUREBET FOUND</b>\n\
         💰 ROI: <b>{:.2}%</b>\n\
         💵 Profit: <b>{:.0}</b> RUB\n\
         📊 <b>Match:</b> {} vs {}\n\
         🏆 <b>League:</b> {}\n\
         ⏰ <b>Start:</b> {}\n\
         Status: {}{}\n",
        surebet.profit_percent,
        profit_amount,
        surebet.home_team,
        surebet.away_team,
        surebet.league,
        time_str,
        if surebet.verified {
            "✅ Verified"
        } else {
            "⚠️ Raw"
        },
        if surebet.is_live { " | 🔴 LIVE" } else { "" }
    );

    // Add market and odds details
    if !surebet.legs.is_empty() {
        msg.push_str("\n<b>Legs:</b>\n");
        for (i, leg) in surebet.legs.iter().enumerate() {
            let line_str = leg.line.map(|v| format!(" {:.2}", v)).unwrap_or_default();
            msg.push_str(&format!(
                "{}. <code>{}</code> {}{} @ <b>{:.2}</b> | {} ({}x)\n",
                i + 1,
                leg.bookmaker,
                leg.market,
                line_str.trim(),
                leg.odds,
                leg.selection,
                leg.payout / leg.stake
            ));
        }
    }

    // Add stake and total info
    msg.push_str(&format!(
        "\n<b>Total Stake:</b> {:.0} RUB\n\
         <b>Expected Payout:</b> {:.0} RUB\n\
         <code>ID: {}</code>",
        surebet.total_stake,
        surebet.legs.first().map(|l| l.payout).unwrap_or(0.0),
        surebet.id
    ));

    msg
}

/// Format settings message
pub fn format_settings_message(config: &TelegramAlertConfig, stats: &AlertStats) -> String {
    format!(
        "⚙️ <b>Alert Settings</b>\n\n\
         <b>Filters:</b>\n\
         • Min ROI: {:.2}%\n\
         • Max alerts/min: {:.0}\n\
         • Only verified: {}\n\
         • Only live: {}\n\n\
         <b>Statistics:</b>\n\
         • Total alerts: {}\n\
         • Sent: {}\n\
         • Throttled: {}\n\
         • Skipped: {}\n\
         • Last hour: {} sent\n\
         • Last minute: {} sent\n\
         • Avg ROI: {:.2}%",
        config.min_roi_percent,
        config.max_alerts_per_minute,
        config.only_verified,
        config.only_live,
        stats.total_alerts,
        stats.sent,
        stats.throttled,
        stats.skipped,
        stats.sent_in_last_hour,
        stats.sent_in_last_minute,
        stats.avg_roi
    )
}

/// Format help message
pub fn format_help_message() -> String {
    "<b>🤖 Ghost Imperium Telegram Bot</b>\n\n\
     <b>Available Commands:</b>\n\n\
     <b>Monitoring:</b>\n\
     /status - Bridge status and metrics\n\
     /health - EventBus and parser health\n\
     /recent - Last 5 surebets received\n\
     /top - Highest ROI recent surebets\n\
     /alerts - Alert statistics\n\n\
     <b>Configuration:</b>\n\
     /settings - Show current settings\n\
     /history - Show alert history\n\n\
     <b>Help:</b>\n\
     /help - Show this message\n\
     /start - Quick introduction"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Sport;
    use uuid::Uuid;

    fn create_test_surebet(roi: f64, verified: bool, is_live: bool) -> Surebet {
        Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Test League".to_string(),
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            start_time: Some(Utc::now()),
            is_live,
            profit_percent: roi,
            total_stake: 1000.0,
            legs: vec![shared::SurebetLeg {
                bookmaker: "test_bk".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: 2.0,
                line: None,
                stake: 500.0,
                payout: 1000.0 + (1000.0 * roi / 100.0),
                url: None,
            }],
            detected_at: Utc::now(),
            verified,
            mirror: false,
        }
    }

    #[test]
    fn default_config_allows_2_percent_roi() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(2.5, false, false);

        assert!(manager.should_alert(&surebet).is_ok());
    }

    #[test]
    fn rejects_low_roi() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(1.5, false, false);

        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn respects_only_verified_filter() {
        let mut config = TelegramAlertConfig::default();
        config.only_verified = true;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, false, false);

        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn respects_only_live_filter() {
        let mut config = TelegramAlertConfig::default();
        config.only_live = true;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, true, false);

        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn records_alert_history() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(3.0, true, false);

        manager.record_alert(&surebet, AlertStatus::Sent);
        manager.record_alert(&surebet, AlertStatus::Throttled);

        let history = manager.get_history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].status, AlertStatus::Throttled);
        assert_eq!(history[1].status, AlertStatus::Sent);
    }

    #[test]
    fn history_respects_max_size() {
        let mut config = TelegramAlertConfig::default();
        config.history_size = 5;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(3.0, true, false);

        for _ in 0..10 {
            manager.record_alert(&surebet, AlertStatus::Sent);
        }

        let history = manager.get_history(100);
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn stats_calculation_is_accurate() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, true, false);

        manager.record_alert(&surebet, AlertStatus::Sent);
        manager.record_alert(&surebet, AlertStatus::Throttled);
        manager.record_alert(&surebet, AlertStatus::Sent);

        let stats = manager.get_stats();
        assert_eq!(stats.total_alerts, 3);
        assert_eq!(stats.sent, 2);
        assert_eq!(stats.throttled, 1);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn format_surebet_alert_includes_key_fields() {
        let surebet = create_test_surebet(3.5, true, false);
        let msg = format_surebet_alert(&surebet);

        assert!(msg.contains("3.5%"));
        assert!(msg.contains("Home vs Away"));
        assert!(msg.contains("Test League"));
        assert!(msg.contains("test_bk"));
    }

    #[test]
    fn format_settings_shows_config() {
        let mut config = TelegramAlertConfig::default();
        config.min_roi_percent = 1.5;
        config.max_alerts_per_minute = 15.0;

        let stats = AlertStats {
            total_alerts: 42,
            sent: 35,
            throttled: 3,
            skipped: 4,
            avg_roi: 2.8,
            sent_in_last_hour: 8,
            sent_in_last_minute: 1,
        };

        let msg = format_settings_message(&config, &stats);
        assert!(msg.contains("1.50%"));
        assert!(msg.contains("15"));
        assert!(msg.contains("42"));
        assert!(msg.contains("2.80%"));
    }
}
