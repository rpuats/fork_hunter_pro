use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{
    collections::{HashMap, VecDeque},
    fmt::Write,
};

use chrono::{DateTime, Utc};
use shared::{EventBus, Surebet};
use teloxide::prelude::*;
use teloxide::types::ChatId;
use tracing::{error, info};

const INFO_ALERT_COOLDOWN: Duration = Duration::from_secs(15 * 60);
const RECENT_SUREBETS_LIMIT: usize = 12;
const RECENT_ALERTS_LIMIT: usize = 8;

pub struct TelegramBot {
    bot: Bot,
    admin_chats: Vec<i64>,
    min_profit: f64,
    silent: bool,
    event_bus: Option<Arc<EventBus>>,
    metrics: Arc<TelegramMetrics>,
    info_alert_gate: Mutex<InfoAlertGate>,
    state: Mutex<TelegramState>,
}

impl TelegramBot {
    pub fn new(
        token: &str,
        admin_chats: Vec<i64>,
        min_profit: f64,
        silent: bool,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        Self {
            bot: Bot::new(token),
            admin_chats,
            min_profit,
            silent,
            event_bus,
            metrics: Arc::new(TelegramMetrics::default()),
            info_alert_gate: Mutex::new(InfoAlertGate::default()),
            state: Mutex::new(TelegramState::default()),
        }
    }

    pub fn metrics(&self) -> Arc<TelegramMetrics> {
        self.metrics.clone()
    }

    pub async fn notify_surebet(&self, surebet: &Surebet) -> bool {
        if surebet.profit_percent < self.min_profit || self.admin_chats.is_empty() {
            return false;
        }

        let message = self.format_surebet_message(surebet);
        self.send_to_admins(&message).await
    }

    pub async fn notify_system(&self, message: &str) -> bool {
        if self.silent || self.admin_chats.is_empty() {
            return false;
        }

        self.send_to_admins(message).await
    }

    pub fn prepare_system_alert(
        &self,
        level: &str,
        message: &str,
        timestamp: DateTime<Utc>,
    ) -> Option<String> {
        if self.silent || !self.should_forward_system_alert(level, message) {
            return None;
        }

        Some(format_system_alert(level, message, timestamp))
    }

    pub fn format_surebet_message(&self, surebet: &Surebet) -> String {
        let time_str = surebet
            .start_time
            .map(|t| t.format("%d.%m %H:%M UTC").to_string())
            .unwrap_or_else(|| "N/A".to_string());
        let mut msg = format!(
            "🔥 Вилка {:.2}%\n{} vs {}\n{}\nСтарт: {}\nСтатус: {}{}\n",
            surebet.profit_percent,
            surebet.home_team,
            surebet.away_team,
            surebet.league,
            time_str,
            if surebet.verified { "verified" } else { "raw" },
            if surebet.mirror { ", mirror" } else { "" }
        );

        for (i, leg) in surebet.legs.iter().enumerate() {
            let line = leg
                .line
                .map(|value| format!(" {:.2}", value))
                .unwrap_or_default();
            msg.push_str(&format!(
                "{}. {} | {} {} @ {:.2} | stake {:.0}\n",
                i + 1,
                leg.bookmaker,
                leg.selection,
                line.trim(),
                leg.odds,
                leg.stake
            ));
        }

        msg.push_str(&format!(
            "Total: {:.0} | Payout: {:.0}\nID: {}",
            surebet.total_stake,
            surebet.legs.first().map(|leg| leg.payout).unwrap_or(0.0),
            surebet.id
        ));

        msg
    }

    pub fn status_message(&self) -> String {
        let metrics = self.metrics.snapshot();
        let state = self.state.lock().expect("telegram state poisoned");
        let (bus_events, bus_subscribers) = self
            .event_bus
            .as_ref()
            .map(|bus| (bus.event_count(), bus.subscriber_count()))
            .unwrap_or((0, 0));

        format!(
            "🤖 Telegram bridge\nChats: {}\nMin profit: {:.2}%\nSilent: {}\nUptime: {}\nBus events: {}\nBus subscribers: {}\nLast bus activity: {}\nForwarded surebets: {}\nForwarded alerts: {}\nLagged events: {}\nLast surebet: {}\nLast alert: {}",
            self.admin_chats.len(),
            self.min_profit,
            if self.silent { "on" } else { "off" },
            format_duration(state.started_at.elapsed()),
            bus_events,
            bus_subscribers,
            format_optional_timestamp(state.last_bus_event_at),
            metrics.surebets_forwarded,
            metrics.system_alerts_forwarded,
            metrics.lagged_events,
            metrics.last_surebet.unwrap_or_else(|| "-".to_string()),
            metrics.last_alert.unwrap_or_else(|| "-".to_string()),
        )
    }

    pub fn alerts_message(&self) -> String {
        let metrics = self.metrics.snapshot();
        let state = self.state.lock().expect("telegram state poisoned");
        let recent_alerts = if state.recent_alerts.is_empty() {
            "-".to_string()
        } else {
            state
                .recent_alerts
                .iter()
                .take(3)
                .map(SystemAlertDigest::short_line)
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "🔔 Alerts\nSeen events: {}\nSurebets sent: {}\nSystem alerts sent: {}\nLagged: {}\nLast surebet: {}\nLast alert: {}\nRecent alerts:\n{}",
            metrics.bus_events_seen,
            metrics.surebets_forwarded,
            metrics.system_alerts_forwarded,
            metrics.lagged_events,
            metrics.last_surebet.unwrap_or_else(|| "-".to_string()),
            metrics.last_alert.unwrap_or_else(|| "-".to_string()),
            recent_alerts,
        )
    }

    pub fn recent_message(&self) -> String {
        let state = self.state.lock().expect("telegram state poisoned");
        if state.recent_surebets.is_empty() {
            return "🕘 Recent surebets\nNo surebets received from EventBus yet.".to_string();
        }

        let mut message = String::from("🕘 Recent surebets");
        for (index, surebet) in state.recent_surebets.iter().take(5).enumerate() {
            let _ = write!(message, "\n{}. {}", index + 1, surebet.short_line());
        }
        message
    }

    pub fn top_message(&self) -> String {
        let state = self.state.lock().expect("telegram state poisoned");
        if state.recent_surebets.is_empty() {
            return "🏆 Top surebets\nNo surebets received from EventBus yet.".to_string();
        }

        let mut top = state.recent_surebets.iter().cloned().collect::<Vec<_>>();
        top.sort_by(|left, right| right.profit_percent.total_cmp(&left.profit_percent));

        let mut message = String::from("🏆 Top recent surebets");
        for (index, surebet) in top.into_iter().take(5).enumerate() {
            let _ = write!(message, "\n{}. {}", index + 1, surebet.short_line());
        }
        message
    }

    pub fn health_message(&self) -> String {
        let state = self.state.lock().expect("telegram state poisoned");
        let health = state.parser_health_rollup();
        let latest_alert = state
            .recent_alerts
            .front()
            .map(SystemAlertDigest::short_line)
            .unwrap_or_else(|| "-".to_string());
        let unhealthy = state.unhealthy_parsers_line();

        format!(
            "🩺 Bridge health\nUptime: {}\nLast bus activity: {}\nEvents seen: {} raw / {} normalized / {} surebets / {} alerts / {} parser health\nParser health: {} healthy, {} degraded, {} unhealthy\nWatchlist: {}\nLatest alert: {}",
            format_duration(state.started_at.elapsed()),
            format_optional_timestamp(state.last_bus_event_at),
            state.event_counters.raw_odds,
            state.event_counters.normalized_events,
            state.event_counters.surebets,
            state.event_counters.system_alerts,
            state.event_counters.parser_health,
            health.healthy,
            health.degraded,
            health.unhealthy,
            unhealthy,
            latest_alert,
        )
    }

    pub fn observe_event(&self, event: &shared::BusEvent) {
        let mut state = self.state.lock().expect("telegram state poisoned");
        state.observe_event(event);
    }

    pub fn record_seen_surebet(&self, surebet: &Surebet) {
        let mut state = self.state.lock().expect("telegram state poisoned");
        state.push_surebet(SurebetDigest::from_surebet(surebet));
    }

    pub fn record_seen_system_alert(&self, level: &str, message: &str, timestamp: DateTime<Utc>) {
        let mut state = self.state.lock().expect("telegram state poisoned");
        state.push_alert(SystemAlertDigest::new(level, message, timestamp));
    }

    pub fn reply_for_text(&self, text: &str) -> Option<String> {
        match text.trim() {
            "/start" => Some(
                "Ghost Imperium Bot\n/status - bridge status\n/health - EventBus and parser health\n/recent - last surebets from bridge\n/top - best recent surebets\n/alerts - recent alert counters\n/help - command list"
                    .to_string(),
            ),
            "/status" => Some(self.status_message()),
            "/health" => Some(self.health_message()),
            "/recent" => Some(self.recent_message()),
            "/top" => Some(self.top_message()),
            "/alerts" => Some(self.alerts_message()),
            "/help" => Some(
                "Commands:\n/start - bot intro\n/status - bridge counters and uptime\n/health - event flow and parser health rollup\n/recent - latest surebets from EventBus\n/top - highest-profit recent surebets\n/alerts - forwarded alert counters and recent alerts\n/help - this help"
                    .to_string(),
            ),
            command if command.starts_with('/') => {
                Some("Unknown command. Use /help to see available commands.".to_string())
            }
            _ => None,
        }
    }

    pub fn spawn(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let bot = self.bot.clone();
        let this = self.clone();

        tokio::spawn(async move {
            match bot.get_me().await {
                Ok(me) => {
                    let username = me.username.clone().unwrap_or_else(|| "unknown".to_string());
                    info!("Telegram bot authorized as @{}", username);
                }
                Err(e) => {
                    error!(
                        "Telegram bot failed to start — check TELEGRAM_BOT_TOKEN: {}",
                        e
                    );
                    return;
                }
            }

            let handler = move |msg: Message, bot: Bot| {
                let this = this.clone();
                async move {
                    if let Some(text) = msg.text() {
                        let reply = this.reply_for_text(text);

                        if let Some(reply) = reply {
                            let _ = bot.send_message(msg.chat.id, reply).await;
                        }
                    }
                    Ok(())
                }
            };

            info!("Telegram bot starting (async spawn mode)...");
            teloxide::repl(bot, handler).await;
            info!("Telegram bot stopped");
        })
    }

    fn should_forward_system_alert(&self, level: &str, message: &str) -> bool {
        let normalized = level.trim().to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "error" | "warn" | "warning" | "critical"
        ) {
            return true;
        }

        if normalized != "info" {
            return false;
        }

        let fingerprint = system_alert_fingerprint(message);
        let mut gate = self
            .info_alert_gate
            .lock()
            .expect("info alert gate poisoned");
        if let Some(last_sent_at) = gate.last_sent_at {
            if last_sent_at.elapsed() < INFO_ALERT_COOLDOWN && gate.last_fingerprint == fingerprint
            {
                return false;
            }
        }

        gate.last_sent_at = Some(Instant::now());
        gate.last_fingerprint = fingerprint;
        true
    }

    async fn send_to_admins(&self, message: &str) -> bool {
        let mut delivered = false;

        for &chat_id in &self.admin_chats {
            match self.bot.send_message(ChatId(chat_id), message).await {
                Ok(_) => delivered = true,
                Err(e) => {
                    error!(
                        chat_id,
                        error = e.to_string(),
                        "Failed to send Telegram message"
                    );
                }
            }
        }

        delivered
    }
}

#[derive(Default)]
struct InfoAlertGate {
    last_sent_at: Option<Instant>,
    last_fingerprint: String,
}

struct TelegramState {
    started_at: Instant,
    last_bus_event_at: Option<DateTime<Utc>>,
    recent_surebets: VecDeque<SurebetDigest>,
    recent_alerts: VecDeque<SystemAlertDigest>,
    parser_health: HashMap<String, ParserHealthDigest>,
    event_counters: EventCounters,
}

impl Default for TelegramState {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            last_bus_event_at: None,
            recent_surebets: VecDeque::with_capacity(RECENT_SUREBETS_LIMIT),
            recent_alerts: VecDeque::with_capacity(RECENT_ALERTS_LIMIT),
            parser_health: HashMap::new(),
            event_counters: EventCounters::default(),
        }
    }
}

impl TelegramState {
    fn observe_event(&mut self, event: &shared::BusEvent) {
        use shared::BusEvent;

        match event {
            BusEvent::RawOdds { timestamp, .. } => {
                self.event_counters.raw_odds += 1;
                self.last_bus_event_at = Some(*timestamp);
            }
            BusEvent::NormalizedEvent { timestamp, .. } => {
                self.event_counters.normalized_events += 1;
                self.last_bus_event_at = Some(*timestamp);
            }
            BusEvent::SurebetFound { timestamp, .. } => {
                self.event_counters.surebets += 1;
                self.last_bus_event_at = Some(*timestamp);
            }
            BusEvent::ParserHealth {
                bookmaker,
                status,
                timestamp,
            } => {
                self.event_counters.parser_health += 1;
                self.last_bus_event_at = Some(*timestamp);
                self.parser_health.insert(
                    bookmaker.clone(),
                    ParserHealthDigest::new(bookmaker, status, *timestamp),
                );
            }
            BusEvent::SystemAlert {
                level,
                message,
                timestamp,
            } => {
                self.event_counters.system_alerts += 1;
                self.last_bus_event_at = Some(*timestamp);
                self.push_alert(SystemAlertDigest::new(level, message, *timestamp));
            }
        }
    }

    fn push_surebet(&mut self, surebet: SurebetDigest) {
        self.recent_surebets.push_front(surebet);
        while self.recent_surebets.len() > RECENT_SUREBETS_LIMIT {
            self.recent_surebets.pop_back();
        }
    }

    fn push_alert(&mut self, alert: SystemAlertDigest) {
        self.recent_alerts.push_front(alert);
        while self.recent_alerts.len() > RECENT_ALERTS_LIMIT {
            self.recent_alerts.pop_back();
        }
    }

    fn parser_health_rollup(&self) -> ParserHealthRollup {
        let mut rollup = ParserHealthRollup::default();
        for parser in self.parser_health.values() {
            match parser.severity() {
                HealthSeverity::Healthy => rollup.healthy += 1,
                HealthSeverity::Degraded => rollup.degraded += 1,
                HealthSeverity::Unhealthy => rollup.unhealthy += 1,
            }
        }
        rollup
    }

    fn unhealthy_parsers_line(&self) -> String {
        let mut watchlist = self
            .parser_health
            .values()
            .filter(|parser| !matches!(parser.severity(), HealthSeverity::Healthy))
            .cloned()
            .collect::<Vec<_>>();

        watchlist.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
        if watchlist.is_empty() {
            return "none".to_string();
        }

        watchlist
            .into_iter()
            .take(4)
            .map(|parser| parser.short_line())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Default)]
struct EventCounters {
    raw_odds: u64,
    normalized_events: u64,
    surebets: u64,
    system_alerts: u64,
    parser_health: u64,
}

#[derive(Clone)]
struct SurebetDigest {
    profit_percent: f64,
    home_team: String,
    away_team: String,
    league: String,
    detected_at: DateTime<Utc>,
    verified: bool,
    is_live: bool,
}

impl SurebetDigest {
    fn from_surebet(surebet: &Surebet) -> Self {
        Self {
            profit_percent: surebet.profit_percent,
            home_team: surebet.home_team.clone(),
            away_team: surebet.away_team.clone(),
            league: surebet.league.clone(),
            detected_at: surebet.detected_at,
            verified: surebet.verified,
            is_live: surebet.is_live,
        }
    }

    fn short_line(&self) -> String {
        format!(
            "{:.2}% {} vs {} | {} | {}{}{}",
            self.profit_percent,
            self.home_team,
            self.away_team,
            shorten(&self.league, 28),
            self.detected_at.format("%H:%M:%S UTC"),
            if self.verified { " | verified" } else { "" },
            if self.is_live { " | live" } else { "" },
        )
    }
}

#[derive(Clone)]
struct SystemAlertDigest {
    level: String,
    message: String,
    timestamp: DateTime<Utc>,
}

impl SystemAlertDigest {
    fn new(level: &str, message: &str, timestamp: DateTime<Utc>) -> Self {
        Self {
            level: level.to_string(),
            message: shorten(message, 90),
            timestamp,
        }
    }

    fn short_line(&self) -> String {
        format!(
            "{} {} @ {}",
            self.level.to_ascii_uppercase(),
            self.message,
            self.timestamp.format("%H:%M:%S UTC")
        )
    }
}

#[derive(Clone)]
struct ParserHealthDigest {
    bookmaker: String,
    status: String,
    timestamp: DateTime<Utc>,
}

impl ParserHealthDigest {
    fn new(bookmaker: &str, status: &str, timestamp: DateTime<Utc>) -> Self {
        Self {
            bookmaker: bookmaker.to_string(),
            status: status.to_string(),
            timestamp,
        }
    }

    fn severity(&self) -> HealthSeverity {
        health_severity(&self.status)
    }

    fn short_line(&self) -> String {
        format!(
            "{}={} @ {}",
            self.bookmaker,
            self.status,
            self.timestamp.format("%H:%M:%S UTC")
        )
    }
}

#[derive(Default)]
struct ParserHealthRollup {
    healthy: usize,
    degraded: usize,
    unhealthy: usize,
}

#[derive(Clone, Copy)]
enum HealthSeverity {
    Healthy,
    Degraded,
    Unhealthy,
}

fn health_severity(status: &str) -> HealthSeverity {
    let normalized = status.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "ok" | "healthy" | "up" | "ready" | "running"
    ) {
        HealthSeverity::Healthy
    } else if normalized.contains("warn")
        || normalized.contains("degrad")
        || normalized.contains("slow")
        || normalized.contains("timeout")
    {
        HealthSeverity::Degraded
    } else {
        HealthSeverity::Unhealthy
    }
}

#[derive(Default)]
pub struct TelegramMetrics {
    bus_events_seen: AtomicU64,
    surebets_forwarded: AtomicU64,
    system_alerts_forwarded: AtomicU64,
    lagged_events: AtomicU64,
    last_surebet: Mutex<Option<String>>,
    last_alert: Mutex<Option<String>>,
}

impl TelegramMetrics {
    pub fn record_bus_event(&self) {
        self.bus_events_seen.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_surebet(&self, surebet: &Surebet) {
        self.surebets_forwarded.fetch_add(1, Ordering::Relaxed);
        *self.last_surebet.lock().expect("last_surebet poisoned") = Some(format!(
            "{:.2}% {} vs {}",
            surebet.profit_percent, surebet.home_team, surebet.away_team
        ));
    }

    pub fn record_system_alert(&self, level: &str, message: &str) {
        self.system_alerts_forwarded.fetch_add(1, Ordering::Relaxed);
        *self.last_alert.lock().expect("last_alert poisoned") = Some(format!(
            "{}: {}",
            level.to_ascii_uppercase(),
            shorten(message, 72)
        ));
    }

    pub fn record_lag(&self, skipped: u64) {
        self.lagged_events.fetch_add(skipped, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TelegramMetricsSnapshot {
        TelegramMetricsSnapshot {
            bus_events_seen: self.bus_events_seen.load(Ordering::Relaxed),
            surebets_forwarded: self.surebets_forwarded.load(Ordering::Relaxed),
            system_alerts_forwarded: self.system_alerts_forwarded.load(Ordering::Relaxed),
            lagged_events: self.lagged_events.load(Ordering::Relaxed),
            last_surebet: self
                .last_surebet
                .lock()
                .expect("last_surebet poisoned")
                .clone(),
            last_alert: self.last_alert.lock().expect("last_alert poisoned").clone(),
        }
    }
}

pub struct TelegramMetricsSnapshot {
    pub bus_events_seen: u64,
    pub surebets_forwarded: u64,
    pub system_alerts_forwarded: u64,
    pub lagged_events: u64,
    pub last_surebet: Option<String>,
    pub last_alert: Option<String>,
}

fn format_system_alert(level: &str, message: &str, timestamp: DateTime<Utc>) -> String {
    format!(
        "{} {}\n{}\n{}",
        level_emoji(level),
        level.to_ascii_uppercase(),
        shorten(message, 300),
        timestamp.format("%d.%m %H:%M:%S UTC")
    )
}

fn system_alert_fingerprint(message: &str) -> String {
    if message.starts_with("Cycle:") {
        "cycle".to_string()
    } else {
        shorten(message, 120)
    }
}

fn level_emoji(level: &str) -> &'static str {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" | "critical" => "🚨",
        "warn" | "warning" => "⚠️",
        _ => "ℹ️",
    }
}

fn shorten(input: &str, limit: usize) -> String {
    let mut shortened = input.trim().replace('\n', " ");
    if shortened.chars().count() <= limit {
        return shortened;
    }

    shortened = shortened.chars().take(limit.saturating_sub(3)).collect();
    shortened.push_str("...");
    shortened
}

fn format_optional_timestamp(timestamp: Option<DateTime<Utc>>) -> String {
    timestamp
        .map(|value| value.format("%d.%m %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}h {:02}m {:02}s", hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use shared::Sport;
    use shared::SurebetLeg;
    use uuid::Uuid;

    #[test]
    fn surebet_message_contains_important_fields() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        let surebet = Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Premier League".to_string(),
            home_team: "Arsenal".to_string(),
            away_team: "Chelsea".to_string(),
            start_time: Some(Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 0).unwrap()),
            is_live: false,
            profit_percent: 2.45,
            total_stake: 1000.0,
            legs: vec![SurebetLeg {
                bookmaker: "pari".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: 2.1,
                line: None,
                stake: 480.0,
                payout: 1008.0,
                url: None,
            }],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        };

        let message = bot.format_surebet_message(&surebet);
        assert!(message.contains("Вилка 2.45%"));
        assert!(message.contains("Arsenal vs Chelsea"));
        assert!(message.contains("Premier League"));
        assert!(message.contains("pari"));
        assert!(message.contains("ID: 00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn info_alerts_are_throttled_by_fingerprint() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        assert!(bot.should_forward_system_alert("info", "Cycle: 10ms, 20 events, 1 opportunities"));
        assert!(!bot.should_forward_system_alert("info", "Cycle: 11ms, 21 events, 2 opportunities"));
        assert!(bot.should_forward_system_alert("warn", "Parser timeout"));
    }

    #[test]
    fn status_message_includes_bridge_counters() {
        let bot = TelegramBot::new("token", vec![1, 2], 1.5, true, None);
        bot.metrics().record_bus_event();
        bot.metrics()
            .record_system_alert("warn", "Parser timeout on pari");
        bot.observe_event(&shared::BusEvent::SystemAlert {
            level: "warn".to_string(),
            message: "Parser timeout on pari".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 4, 13, 15, 1, 0).unwrap(),
        });

        let message = bot.status_message();
        assert!(message.contains("Chats: 2"));
        assert!(message.contains("Min profit: 1.50%"));
        assert!(message.contains("Forwarded alerts: 1"));
        assert!(message.contains("Uptime:"));
    }

    #[test]
    fn recent_and_top_messages_include_buffered_surebets() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let first = Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Premier League".to_string(),
            home_team: "Arsenal".to_string(),
            away_team: "Chelsea".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.45,
            total_stake: 1000.0,
            legs: vec![],
            detected_at: Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 0).unwrap(),
            verified: true,
            mirror: false,
        };
        let second = Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Serie A".to_string(),
            home_team: "Inter".to_string(),
            away_team: "Milan".to_string(),
            start_time: None,
            is_live: true,
            profit_percent: 3.80,
            total_stake: 1000.0,
            legs: vec![],
            detected_at: Utc.with_ymd_and_hms(2026, 4, 13, 15, 1, 0).unwrap(),
            verified: false,
            mirror: false,
        };

        bot.record_seen_surebet(&first);
        bot.record_seen_surebet(&second);

        let recent = bot.recent_message();
        assert!(recent.contains("1. 3.80% Inter vs Milan"));
        assert!(recent.contains("2. 2.45% Arsenal vs Chelsea"));

        let top = bot.top_message();
        assert!(top.contains("1. 3.80% Inter vs Milan"));
        assert!(top.contains("2. 2.45% Arsenal vs Chelsea"));
    }

    #[test]
    fn health_message_rolls_up_parser_health_and_latest_alert() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        bot.observe_event(&shared::BusEvent::ParserHealth {
            bookmaker: "pari".to_string(),
            status: "ok".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 0).unwrap(),
        });
        bot.observe_event(&shared::BusEvent::ParserHealth {
            bookmaker: "fonbet".to_string(),
            status: "degraded".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 5).unwrap(),
        });
        bot.observe_event(&shared::BusEvent::ParserHealth {
            bookmaker: "leon".to_string(),
            status: "down".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 10).unwrap(),
        });
        bot.record_seen_system_alert(
            "warn",
            "Parser timeout on fonbet",
            Utc.with_ymd_and_hms(2026, 4, 13, 15, 0, 30).unwrap(),
        );

        let health = bot.health_message();
        assert!(health.contains("Parser health: 1 healthy, 1 degraded, 1 unhealthy"));
        assert!(health.contains("fonbet=degraded"));
        assert!(health.contains("leon=down"));
        assert!(health.contains("Latest alert: WARN Parser timeout on fonbet"));
    }

    #[test]
    fn unknown_commands_return_help_hint() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply = bot.reply_for_text("/unknown");
        assert_eq!(
            reply.as_deref(),
            Some("Unknown command. Use /help to see available commands.")
        );
    }
}
