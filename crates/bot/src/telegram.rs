use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{
    collections::{HashMap, VecDeque},
    fmt::Write,
};

use crate::notifier::{
    format_help_message, format_settings_message, format_surebet_alert, AlertManager, AlertStatus,
    TelegramAlertConfig,
};
use crate::rate_limiter::RateLimiter;
use chrono::{DateTime, Utc};
use shared::{EventBus, Surebet};
use teloxide::prelude::*;
use teloxide::types::ChatId;
use tracing::{error, info};

const INFO_ALERT_COOLDOWN: Duration = Duration::from_secs(15 * 60);
const RECENT_SUREBETS_LIMIT: usize = 12;
const RECENT_ALERTS_LIMIT: usize = 8;
const DEFAULT_MAX_ALERTS_PER_MINUTE: f64 = 10.0;
const DASHBOARD_HISTORY_LIMIT: usize = 100;
const ML_CONFIDENCE_THRESHOLD: f64 = 0.65;
const HEDGE_MIN_ODDS: f64 = 1.5;
const HEDGE_MAX_ODDS: f64 = 3.5;

pub struct TelegramBot {
    bot: Bot,
    admin_chats: Vec<i64>,
    min_profit: f64,
    silent: bool,
    event_bus: Option<Arc<EventBus>>,
    metrics: Arc<TelegramMetrics>,
    info_alert_gate: Mutex<InfoAlertGate>,
    state: Mutex<TelegramState>,
    rate_limiter: RateLimiter,
    alert_manager: AlertManager,
    roi_channels: Arc<Mutex<HashMap<String, RoiAlertChannel>>>,
    dashboard: Arc<Mutex<DashboardState>>,
    ml_predictor: Arc<Mutex<SimpleMLPredictor>>,
    admin_users: Vec<i64>,
}

impl TelegramBot {
    pub fn new(
        token: &str,
        admin_chats: Vec<i64>,
        min_profit: f64,
        silent: bool,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        Self::with_config(
            token,
            admin_chats.clone(),
            min_profit,
            silent,
            event_bus,
            TelegramAlertConfig::default(),
            admin_chats,
        )
    }

    pub fn with_config(
        token: &str,
        admin_chats: Vec<i64>,
        min_profit: f64,
        silent: bool,
        event_bus: Option<Arc<EventBus>>,
        alert_config: TelegramAlertConfig,
        admin_users: Vec<i64>,
    ) -> Self {
        let mut config = alert_config;
        config.min_roi_percent = min_profit;

        Self {
            bot: Bot::new(token),
            admin_chats,
            min_profit,
            silent,
            event_bus,
            metrics: Arc::new(TelegramMetrics::default()),
            info_alert_gate: Mutex::new(InfoAlertGate::default()),
            state: Mutex::new(TelegramState::default()),
            rate_limiter: RateLimiter::alerts_per_minute(config.max_alerts_per_minute),
            alert_manager: AlertManager::new(config),
            roi_channels: Arc::new(Mutex::new(HashMap::new())),
            dashboard: Arc::new(Mutex::new(DashboardState::default())),
            ml_predictor: Arc::new(Mutex::new(SimpleMLPredictor::new())),
            admin_users,
        }
    }

    pub fn metrics(&self) -> Arc<TelegramMetrics> {
        self.metrics.clone()
    }

    pub async fn notify_surebet(&self, surebet: &Surebet) -> bool {
        // Check basic filters
        if self.admin_chats.is_empty() {
            return false;
        }

        // Check alert manager filters
        match self.alert_manager.should_alert(surebet) {
            Ok(_) => {}
            Err(reason) => {
                self.alert_manager
                    .record_alert(surebet, AlertStatus::Skipped(reason));
                return false;
            }
        }

        // Check rate limiter
        if !self.rate_limiter.try_consume(1.0) {
            self.alert_manager
                .record_alert(surebet, AlertStatus::Throttled);
            return false;
        }

        // Format and send message
        let message = format_surebet_alert(surebet);
        if self.send_to_admins_html(&message).await {
            self.alert_manager.record_alert(surebet, AlertStatus::Sent);
            self.metrics.record_surebet(surebet);
            true
        } else {
            false
        }
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

    pub fn settings_message(&self) -> String {
        let config = self.alert_manager.get_config();
        let stats = self.alert_manager.get_stats();
        format_settings_message(&config, &stats)
    }

    pub fn history_message(&self) -> String {
        let history = self.alert_manager.get_history(10);
        if history.is_empty() {
            return "📋 <b>Alert History</b>\n\nNo alerts recorded yet.".to_string();
        }

        let mut msg = "📋 <b>Alert History (Last 10)</b>\n\n".to_string();
        for (idx, entry) in history.iter().enumerate() {
            let status_emoji = match entry.status {
                AlertStatus::Sent => "✅",
                AlertStatus::Throttled => "⏸",
                AlertStatus::Skipped(_) => "⏭",
            };
            let _ = write!(
                msg,
                "{}. {} {:.2}% {} | {}\n",
                idx + 1,
                status_emoji,
                entry.roi_percent,
                entry.teams,
                entry.timestamp.format("%H:%M:%S")
            );
        }
        msg
    }

    pub fn dashboard_message(&self) -> String {
        let dashboard = self.dashboard.lock().expect("dashboard poisoned");
        let metrics = self.metrics.snapshot();
        let state = self.state.lock().expect("telegram state poisoned");

        let win_rate = if dashboard.total_surebets > 0 {
            (dashboard.verified_surebets as f64 / dashboard.total_surebets as f64) * 100.0
        } else {
            0.0
        };

        let avg_roi = if dashboard.total_surebets > 0 {
            dashboard.total_roi / dashboard.total_surebets as f64
        } else {
            0.0
        };

        let best_surebet = state
            .recent_surebets
            .iter()
            .max_by(|a, b| {
                a.profit_percent
                    .partial_cmp(&b.profit_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| format!("{:.2}%", s.profit_percent))
            .unwrap_or_else(|| "N/A".to_string());

        format!(
            "📊 <b>TRADING DASHBOARD</b>\n\n\
             <b>Performance Metrics:</b>\n\
             • Total Surebets: {}\n\
             • Verified: {} ({:.1}%)\n\
             • Avg ROI: {:.2}%\n\
             • Best ROI: {}\n\n\
             <b>Live Stats:</b>\n\
             • Alerts Sent: {}\n\
             • Events Processed: {}\n\
             • Uptime: {}\n\n\
             <b>ROI Distribution:</b>\n\
             • 0-1%: {} | 1-2%: {} | 2-3%: {} | 3%+: {}\n\n\
             <b>Channel Activity:</b>\n\
             • Total Channels: {}\n\
             • Active Channels: {}\n\n\
             <b>Last Updated:</b> {}",
            dashboard.total_surebets,
            dashboard.verified_surebets,
            win_rate,
            avg_roi,
            best_surebet,
            metrics.surebets_forwarded,
            metrics.bus_events_seen,
            format_duration(state.started_at.elapsed()),
            dashboard.roi_buckets.low,
            dashboard.roi_buckets.medium,
            dashboard.roi_buckets.high,
            dashboard.roi_buckets.very_high,
            dashboard.channels_count,
            dashboard.active_channels,
            Utc::now().format("%H:%M:%S")
        )
    }

    pub fn channels_message(&self) -> String {
        let channels = self.roi_channels.lock().expect("roi_channels poisoned");

        if channels.is_empty() {
            return "📡 <b>ROI Alert Channels</b>\n\nNo channels configured. Use /setchannel to create one.".to_string();
        }

        let mut msg = "📡 <b>ROI Alert Channels</b>\n\n".to_string();
        for (name, channel) in channels.iter() {
            let status = if channel.active {
                "✅ ACTIVE"
            } else {
                "⛔ INACTIVE"
            };
            let _ = write!(
                msg,
                "<b>{}</b> {}\n\
                 • Min ROI: {:.2}% | Max ROI: {:.2}%\n\
                 • Alerts: {} | Last: {}\n\n",
                name,
                status,
                channel.min_roi,
                channel.max_roi,
                channel.alert_count,
                channel
                    .last_alert
                    .map(|t| t.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
        msg
    }

    pub fn predict_message(&self, surebet: &Surebet) -> String {
        let predictor = self.ml_predictor.lock().expect("ml_predictor poisoned");
        let prediction = predictor.predict_odds(&surebet.legs);

        format!(
            "🤖 <b>ML ODDS PREDICTION</b>\n\n\
             <b>Event:</b> {} vs {}\n\
             <b>League:</b> {}\n\n\
             <b>Predicted Odds Adjustment:</b>\n\
             • Current Avg Odds: {:.2}\n\
             • Predicted Trend: {}\n\
             • Confidence: {:.1}%\n\
             • Recommendation: {}\n\n\
             <b>Market Analysis:</b>\n\
             • Volatility Index: {:.2}\n\
             • Efficiency Score: {:.1}%\n\
             • Edge Probability: {:.1}%",
            surebet.home_team,
            surebet.away_team,
            surebet.league,
            prediction.avg_odds,
            prediction.trend,
            prediction.confidence * 100.0,
            prediction.recommendation,
            prediction.volatility,
            prediction.efficiency,
            prediction.edge_probability * 100.0
        )
    }

    pub fn hedge_message(&self, surebet: &Surebet) -> String {
        let hedge = calculate_hedge_strategy(surebet);

        let mut msg = format!(
            "🛡️ <b>HEDGING STRATEGY</b>\n\n\
             <b>Original Bet:</b>\n\
             • ROI: {:.2}%\n\
             • Total Stake: {:.0}\n\
             • Max Payout: {:.0}\n\n\
             <b>Hedge Position:</b>\n\
             • Hedge Type: {}\n\
             • Hedge Stake: {:.0}\n\
             • Hedge Odds: {:.2}\n\
             • Hedge Payout: {:.0}\n\n\
             <b>After Hedge:</b>\n\
             • Guaranteed Profit: {:.0}\n\
             • Risk Reduction: {:.1}%\n\
             • New ROI: {:.2}%\n\n\
             <b>Scenarios:</b>\n",
            surebet.profit_percent,
            surebet.total_stake,
            surebet.legs.first().map(|l| l.payout).unwrap_or(0.0),
            hedge.hedge_type,
            hedge.hedge_stake,
            hedge.hedge_odds,
            hedge.hedge_payout,
            hedge.guaranteed_profit,
            hedge.risk_reduction,
            hedge.new_roi
        );

        for (i, scenario) in hedge.scenarios.iter().enumerate() {
            let _ = write!(
                msg,
                "{}. {} Wins → Profit: {:.0}\n",
                i + 1,
                scenario.outcome,
                scenario.profit
            );
        }

        msg
    }

    pub fn set_channel(&self, name: &str, min_roi: f64, max_roi: f64) -> String {
        if min_roi < 0.0 || max_roi < min_roi || max_roi > 100.0 {
            return "❌ Invalid ROI range. Use: /setchannel <name> <min> <max>".to_string();
        }

        let mut channels = self.roi_channels.lock().expect("roi_channels poisoned");
        let channel = RoiAlertChannel {
            name: name.to_string(),
            min_roi,
            max_roi,
            active: true,
            alert_count: 0,
            last_alert: None,
        };

        channels.insert(name.to_string(), channel);

        let mut dashboard = self.dashboard.lock().expect("dashboard poisoned");
        dashboard.channels_count = channels.len();
        dashboard.active_channels = channels.values().filter(|c| c.active).count();

        format!(
            "✅ Channel '{}' created (ROI: {:.2}% - {:.2}%)",
            name, min_roi, max_roi
        )
    }

    pub fn toggle_channel(&self, name: &str) -> String {
        let mut channels = self.roi_channels.lock().expect("roi_channels poisoned");

        match channels.get_mut(name) {
            Some(channel) => {
                channel.active = !channel.active;
                let status = if channel.active {
                    "✅ ACTIVE"
                } else {
                    "⛔ INACTIVE"
                };

                let mut dashboard = self.dashboard.lock().expect("dashboard poisoned");
                dashboard.active_channels = channels.values().filter(|c| c.active).count();

                format!("Channel '{}' is now {}", name, status)
            }
            None => format!("❌ Channel '{}' not found", name),
        }
    }

    pub fn admin_help(&self) -> String {
        "🔑 <b>ADMIN COMMANDS</b>\n\n\
         <b>Dashboard & Monitoring:</b>\n\
         /dashboard - Live trading dashboard\n\
         /channels - Manage alert channels\n\
         /health - System health status\n\
         /metrics - Detailed metrics\n\n\
         <b>Channel Management:</b>\n\
         /setchannel <name> <min> <max> - Create ROI channel\n\
         /togglechannel <name> - Toggle channel on/off\n\
         /deletechannel <name> - Delete channel\n\n\
         <b>Alert Configuration:</b>\n\
         /setminroi <percentage> - Set minimum ROI\n\
         /clearhistory - Clear alert history\n\
         /exportstats - Export statistics\n\n\
         <b>Prediction & Hedging:</b>\n\
         /predict - Enable ML predictions\n\
         /hedge - Show hedging options\n\n\
         <b>System:</b>\n\
         /status - Bot status\n\
         /settings - Alert settings\n\
         /help - General help"
            .to_string()
    }

    pub fn set_min_roi(&self, roi: f64) -> String {
        if roi < 0.0 || roi > 50.0 {
            return "❌ Invalid ROI. Must be between 0% and 50%".to_string();
        }
        format!("✅ Minimum ROI set to {:.2}%", roi)
    }

    pub fn clear_history(&self) -> String {
        let mut state = self.state.lock().expect("telegram state poisoned");
        let count = state.recent_surebets.len();
        state.recent_surebets.clear();
        format!("✅ Cleared {} surebets from history", count)
    }

    pub fn export_stats(&self) -> String {
        let metrics = self.metrics.snapshot();
        let dashboard = self.dashboard.lock().expect("dashboard poisoned");
        let state = self.state.lock().expect("telegram state poisoned");

        format!(
            "📥 <b>STATISTICS EXPORT</b>\n\n\
             <b>Performance:</b>\n\
             • Total Alerts: {}\n\
             • Surebets Forwarded: {}\n\
             • System Alerts: {}\n\
             • Lagged Events: {}\n\n\
             <b>Dashboard:</b>\n\
             • Total Surebets: {}\n\
             • Verified: {}\n\
             • Total ROI: {:.2}%\n\
             • Channels: {}\n\n\
             <b>Uptime:</b> {}\n\
             <b>Export Time:</b> {}",
            metrics.surebets_forwarded + metrics.system_alerts_forwarded,
            metrics.surebets_forwarded,
            metrics.system_alerts_forwarded,
            metrics.lagged_events,
            dashboard.total_surebets,
            dashboard.verified_surebets,
            dashboard.total_roi,
            dashboard.channels_count,
            format_duration(state.started_at.elapsed()),
            Utc::now().format("%d.%m %H:%M:%S")
        )
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

    pub fn reply_for_text(&self, text: &str, user_id: Option<i64>) -> Option<String> {
        let trimmed = text.trim();
        let is_admin = user_id
            .map(|uid| self.admin_users.contains(&uid))
            .unwrap_or(false);

        match trimmed {
            "/start" => Some(
                "👋 <b>Ghost Imperium Bot</b>\n\n\
                 Real-time Telegram alerts for detected arbitrage opportunities.\n\n\
                 <b>Quick commands:</b>\n\
                 /dashboard - Live trading dashboard\n\
                 /status - Bridge status\n\
                 /settings - Alert configuration\n\
                 /help - Full command list"
                    .to_string(),
            ),
            "/dashboard" => Some(self.dashboard_message()),
            "/channels" => Some(self.channels_message()),
            "/status" => Some(self.status_message()),
            "/health" => Some(self.health_message()),
            "/recent" => Some(self.recent_message()),
            "/top" => Some(self.top_message()),
            "/alerts" => Some(self.alerts_message()),
            "/settings" => Some(self.settings_message()),
            "/history" => Some(self.history_message()),
            "/help" => Some(format_help_message()),
            "/admin" if is_admin => Some(self.admin_help()),
            "/predict" => {
                // Get the most recent surebet and predict
                let state = self.state.lock().expect("telegram state poisoned");
                state.recent_surebets.front().map(|digest| {
                    let surebet = Surebet {
                        id: uuid::Uuid::nil(),
                        sport: shared::Sport::Football,
                        league: digest.league.clone(),
                        home_team: digest.home_team.clone(),
                        away_team: digest.away_team.clone(),
                        start_time: Some(digest.detected_at),
                        is_live: digest.is_live,
                        profit_percent: digest.profit_percent,
                        total_stake: 1000.0,
                        legs: vec![],
                        detected_at: digest.detected_at,
                        verified: digest.verified,
                        mirror: false,
                    };
                    self.predict_message(&surebet)
                })
            }
            "/exportstats" if is_admin => Some(self.export_stats()),
            "/clearhistory" if is_admin => Some(self.clear_history()),
            cmd if is_admin && cmd.starts_with("/setchannel ") => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let (Ok(min), Ok(max)) = (parts[2].parse::<f64>(), parts[3].parse::<f64>()) {
                        Some(self.set_channel(parts[1], min, max))
                    } else {
                        Some(
                            "❌ Invalid format. Use: /setchannel <name> <min_roi> <max_roi>"
                                .to_string(),
                        )
                    }
                } else {
                    Some(
                        "❌ Invalid format. Use: /setchannel <name> <min_roi> <max_roi>"
                            .to_string(),
                    )
                }
            }
            cmd if is_admin && cmd.starts_with("/togglechannel ") => {
                let channel_name = cmd.strip_prefix("/togglechannel ").unwrap_or("").trim();
                Some(self.toggle_channel(channel_name))
            }
            cmd if is_admin && cmd.starts_with("/setminroi ") => {
                let roi_str = cmd.strip_prefix("/setminroi ").unwrap_or("").trim();
                if let Ok(roi) = roi_str.parse::<f64>() {
                    Some(self.set_min_roi(roi))
                } else {
                    Some("❌ Invalid ROI value".to_string())
                }
            }
            command if command.starts_with('/') && is_admin => {
                Some("❌ Unknown admin command. Use /admin for help.".to_string())
            }
            command if command.starts_with('/') => {
                Some("❌ Unknown command. Use /help to see available commands.".to_string())
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
                        let user_id = msg.from().as_ref().map(|u| u.id.0 as i64);
                        let reply = this.reply_for_text(text, user_id);

                        if let Some(reply) = reply {
                            let _ = bot
                                .send_message(msg.chat.id, reply)
                                .parse_mode(teloxide::types::ParseMode::Html)
                                .await;
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

    async fn send_to_admins_html(&self, message: &str) -> bool {
        let mut delivered = false;

        for &chat_id in &self.admin_chats {
            match self
                .bot
                .send_message(ChatId(chat_id), message)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await
            {
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

// ============================================================================
// NEW STRUCTURES FOR ENHANCED FEATURES
// ============================================================================

#[derive(Clone, Debug)]
pub struct RoiAlertChannel {
    pub name: String,
    pub min_roi: f64,
    pub max_roi: f64,
    pub active: bool,
    pub alert_count: usize,
    pub last_alert: Option<DateTime<Utc>>,
}

impl RoiAlertChannel {
    pub fn should_alert(&self, roi: f64) -> bool {
        self.active && roi >= self.min_roi && roi <= self.max_roi
    }

    pub fn record_alert(&mut self) {
        self.alert_count += 1;
        self.last_alert = Some(Utc::now());
    }
}

#[derive(Clone, Debug, Default)]
pub struct RoiBuckets {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub very_high: usize,
}

impl RoiBuckets {
    pub fn add_surebet(&mut self, roi: f64) {
        match roi {
            r if r < 1.0 => self.low += 1,
            r if r < 2.0 => self.medium += 1,
            r if r < 3.0 => self.high += 1,
            _ => self.very_high += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.low + self.medium + self.high + self.very_high
    }
}

#[derive(Clone, Debug, Default)]
pub struct DashboardState {
    pub total_surebets: usize,
    pub verified_surebets: usize,
    pub total_roi: f64,
    pub channels_count: usize,
    pub active_channels: usize,
    pub roi_buckets: RoiBuckets,
}

impl DashboardState {
    pub fn record_surebet(&mut self, roi: f64, verified: bool) {
        self.total_surebets += 1;
        if verified {
            self.verified_surebets += 1;
        }
        self.total_roi += roi;
        self.roi_buckets.add_surebet(roi);
    }

    pub fn reset(&mut self) {
        self.total_surebets = 0;
        self.verified_surebets = 0;
        self.total_roi = 0.0;
        self.roi_buckets = RoiBuckets::default();
    }
}

#[derive(Clone, Debug)]
pub struct MLPrediction {
    pub trend: String,
    pub confidence: f64,
    pub avg_odds: f64,
    pub volatility: f64,
    pub efficiency: f64,
    pub edge_probability: f64,
    pub recommendation: String,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleMLPredictor {
    odds_history: VecDeque<f64>,
    roi_history: VecDeque<f64>,
}

impl SimpleMLPredictor {
    pub fn new() -> Self {
        Self {
            odds_history: VecDeque::with_capacity(50),
            roi_history: VecDeque::with_capacity(50),
        }
    }

    pub fn predict_odds(&self, legs: &[shared::SurebetLeg]) -> MLPrediction {
        let avg_odds = if legs.is_empty() {
            2.0
        } else {
            legs.iter().map(|l| l.odds).sum::<f64>() / legs.len() as f64
        };

        let volatility = self.calculate_volatility();
        let efficiency = self.calculate_efficiency();
        let edge_prob = self.calculate_edge_probability();

        let trend = if volatility > 0.15 {
            "📈 HIGH VOLATILITY".to_string()
        } else if volatility < 0.05 {
            "📉 STABLE".to_string()
        } else {
            "➡️ MODERATE".to_string()
        };

        let recommendation = if edge_prob > 0.70 {
            "✅ STRONG BUY".to_string()
        } else if edge_prob > 0.60 {
            "🟢 BUY".to_string()
        } else if edge_prob > 0.50 {
            "🟡 NEUTRAL".to_string()
        } else {
            "🔴 AVOID".to_string()
        };

        MLPrediction {
            trend,
            confidence: (efficiency * edge_prob).min(0.99),
            avg_odds,
            volatility,
            efficiency,
            edge_probability: edge_prob,
            recommendation,
        }
    }

    fn calculate_volatility(&self) -> f64 {
        if self.odds_history.len() < 2 {
            return 0.1;
        }

        let mean = self.odds_history.iter().sum::<f64>() / self.odds_history.len() as f64;
        let variance = self
            .odds_history
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / self.odds_history.len() as f64;

        variance.sqrt()
    }

    fn calculate_efficiency(&self) -> f64 {
        if self.roi_history.is_empty() {
            return 0.5;
        }

        let positive = self.roi_history.iter().filter(|&&x| x > 0.0).count() as f64;
        (positive / self.roi_history.len() as f64).min(1.0)
    }

    fn calculate_edge_probability(&self) -> f64 {
        if self.roi_history.is_empty() {
            return 0.5;
        }

        let avg_roi = self.roi_history.iter().sum::<f64>() / self.roi_history.len() as f64;
        ((avg_roi / 5.0).abs().min(1.0) + 0.3).min(0.95)
    }

    pub fn record_odds(&mut self, odds: f64) {
        self.odds_history.push_back(odds);
        if self.odds_history.len() > 50 {
            self.odds_history.pop_front();
        }
    }

    pub fn record_roi(&mut self, roi: f64) {
        self.roi_history.push_back(roi);
        if self.roi_history.len() > 50 {
            self.roi_history.pop_front();
        }
    }
}

#[derive(Clone, Debug)]
pub struct HedgeStrategy {
    pub hedge_type: String,
    pub hedge_stake: f64,
    pub hedge_odds: f64,
    pub hedge_payout: f64,
    pub guaranteed_profit: f64,
    pub risk_reduction: f64,
    pub new_roi: f64,
    pub scenarios: Vec<HedgeScenario>,
}

#[derive(Clone, Debug)]
pub struct HedgeScenario {
    pub outcome: String,
    pub profit: f64,
}

pub fn calculate_hedge_strategy(surebet: &Surebet) -> HedgeStrategy {
    let original_profit = surebet
        .legs
        .first()
        .map(|l| l.payout - surebet.total_stake)
        .unwrap_or(0.0);

    let original_roi = (original_profit / surebet.total_stake * 100.0).abs();

    // Simple hedge: lay back at 2.0 odds
    let hedge_odds = 2.0;
    let hedge_stake = original_profit / (hedge_odds - 1.0);
    let hedge_payout = hedge_stake * hedge_odds;
    let guaranteed_profit = original_profit - hedge_stake;
    let risk_reduction = (hedge_stake / original_profit * 100.0).min(100.0);
    let new_roi = (guaranteed_profit / (surebet.total_stake + hedge_stake) * 100.0).abs();

    let scenarios = vec![
        HedgeScenario {
            outcome: "Bet Wins".to_string(),
            profit: original_profit - hedge_stake,
        },
        HedgeScenario {
            outcome: "Hedge Wins".to_string(),
            profit: hedge_payout - surebet.total_stake - hedge_stake,
        },
        HedgeScenario {
            outcome: "Push (Tie)".to_string(),
            profit: -hedge_stake,
        },
    ];

    HedgeStrategy {
        hedge_type: "Lay Back @ 2.0".to_string(),
        hedge_stake,
        hedge_odds,
        hedge_payout,
        guaranteed_profit,
        risk_reduction,
        new_roi,
        scenarios,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use shared::Sport;
    use shared::SurebetLeg;
    use uuid::Uuid;

    // ========================================================================
    // BASIC TELEGRAM BOT TESTS
    // ========================================================================

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

        let reply = bot.reply_for_text("/unknown", None);
        assert_eq!(
            reply.as_deref(),
            Some("❌ Unknown command. Use /help to see available commands.")
        );
    }

    // ========================================================================
    // DASHBOARD TESTS
    // ========================================================================

    #[test]
    fn dashboard_message_contains_performance_metrics() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        let mut dashboard = bot.dashboard.lock().expect("dashboard poisoned");
        dashboard.record_surebet(2.5, true);
        dashboard.record_surebet(3.2, true);
        dashboard.record_surebet(1.8, false);
        drop(dashboard);

        let msg = bot.dashboard_message();
        assert!(msg.contains("📊 <b>TRADING DASHBOARD</b>"));
        assert!(msg.contains("Total Surebets: 3"));
        assert!(msg.contains("Verified: 2"));
        assert!(msg.contains("Avg ROI:"));
    }

    #[test]
    fn dashboard_tracks_roi_buckets() {
        let mut dashboard = DashboardState::default();
        dashboard.record_surebet(0.5, true);
        dashboard.record_surebet(1.5, true);
        dashboard.record_surebet(2.5, true);
        dashboard.record_surebet(4.0, true);

        assert_eq!(dashboard.roi_buckets.low, 1);
        assert_eq!(dashboard.roi_buckets.medium, 1);
        assert_eq!(dashboard.roi_buckets.high, 1);
        assert_eq!(dashboard.roi_buckets.very_high, 1);
    }

    #[test]
    fn dashboard_calculates_win_rate() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        let mut dashboard = bot.dashboard.lock().expect("dashboard poisoned");
        dashboard.total_surebets = 10;
        dashboard.verified_surebets = 8;
        drop(dashboard);

        let msg = bot.dashboard_message();
        assert!(msg.contains("80.0%"));
    }

    #[test]
    fn dashboard_resets() {
        let mut dashboard = DashboardState::default();
        dashboard.record_surebet(2.5, true);
        dashboard.record_surebet(1.5, true);

        assert_eq!(dashboard.total_surebets, 2);

        dashboard.reset();
        assert_eq!(dashboard.total_surebets, 0);
        assert_eq!(dashboard.verified_surebets, 0);
        assert_eq!(dashboard.total_roi, 0.0);
    }

    // ========================================================================
    // ROI ALERT CHANNEL TESTS
    // ========================================================================

    #[test]
    fn roi_channel_filters_by_range() {
        let mut channel = RoiAlertChannel {
            name: "medium_roi".to_string(),
            min_roi: 1.5,
            max_roi: 3.0,
            active: true,
            alert_count: 0,
            last_alert: None,
        };

        assert!(channel.should_alert(2.0));
        assert!(channel.should_alert(1.5));
        assert!(channel.should_alert(3.0));
        assert!(!channel.should_alert(1.0));
        assert!(!channel.should_alert(4.0));
    }

    #[test]
    fn roi_channel_respects_active_status() {
        let mut channel = RoiAlertChannel {
            name: "test".to_string(),
            min_roi: 1.0,
            max_roi: 5.0,
            active: false,
            alert_count: 0,
            last_alert: None,
        };

        assert!(!channel.should_alert(2.0));

        channel.active = true;
        assert!(channel.should_alert(2.0));
    }

    #[test]
    fn set_channel_creates_new_roi_channel() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply = bot.set_channel("high_roi", 3.0, 5.0);
        assert!(reply.contains("✅ Channel 'high_roi' created"));

        let channels = bot.roi_channels.lock().expect("channels poisoned");
        let channel = channels.get("high_roi").expect("channel not found");
        assert_eq!(channel.min_roi, 3.0);
        assert_eq!(channel.max_roi, 5.0);
        assert!(channel.active);
    }

    #[test]
    fn set_channel_validates_roi_range() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply1 = bot.set_channel("bad", -1.0, 5.0);
        assert!(reply1.contains("❌"));

        let reply2 = bot.set_channel("bad", 5.0, 3.0);
        assert!(reply2.contains("❌"));

        let reply3 = bot.set_channel("bad", 0.0, 150.0);
        assert!(reply3.contains("❌"));
    }

    #[test]
    fn toggle_channel_switches_active_state() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        bot.set_channel("test", 1.0, 3.0);

        let reply1 = bot.toggle_channel("test");
        assert!(reply1.contains("⛔ INACTIVE"));

        let reply2 = bot.toggle_channel("test");
        assert!(reply2.contains("✅ ACTIVE"));
    }

    #[test]
    fn channels_message_lists_all_channels() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        bot.set_channel("low", 0.5, 1.5);
        bot.set_channel("medium", 1.5, 3.0);
        bot.set_channel("high", 3.0, 10.0);

        let msg = bot.channels_message();
        assert!(msg.contains("low"));
        assert!(msg.contains("medium"));
        assert!(msg.contains("high"));
        assert!(msg.contains("📡 <b>ROI Alert Channels</b>"));
    }

    #[test]
    fn roi_channel_records_alerts() {
        let mut channel = RoiAlertChannel {
            name: "test".to_string(),
            min_roi: 1.0,
            max_roi: 5.0,
            active: true,
            alert_count: 0,
            last_alert: None,
        };

        channel.record_alert();
        assert_eq!(channel.alert_count, 1);
        assert!(channel.last_alert.is_some());

        channel.record_alert();
        assert_eq!(channel.alert_count, 2);
    }

    // ========================================================================
    // ML PREDICTOR TESTS
    // ========================================================================

    #[test]
    fn ml_predictor_predicts_empty_odds() {
        let predictor = SimpleMLPredictor::new();
        let prediction = predictor.predict_odds(&[]);

        assert_eq!(prediction.avg_odds, 2.0);
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
    }

    #[test]
    fn ml_predictor_calculates_avg_odds() {
        let predictor = SimpleMLPredictor::new();
        let legs = vec![
            SurebetLeg {
                bookmaker: "pari".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: 2.0,
                line: None,
                stake: 100.0,
                payout: 200.0,
                url: None,
            },
            SurebetLeg {
                bookmaker: "bet".to_string(),
                market: "1X2".to_string(),
                selection: "X".to_string(),
                odds: 3.0,
                line: None,
                stake: 100.0,
                payout: 300.0,
                url: None,
            },
        ];

        let prediction = predictor.predict_odds(&legs);
        assert_eq!(prediction.avg_odds, 2.5);
    }

    #[test]
    fn ml_predictor_tracks_odds_history() {
        let mut predictor = SimpleMLPredictor::new();
        predictor.record_odds(2.0);
        predictor.record_odds(2.1);
        predictor.record_odds(2.05);

        assert_eq!(predictor.odds_history.len(), 3);
    }

    #[test]
    fn ml_predictor_tracks_roi_history() {
        let mut predictor = SimpleMLPredictor::new();
        predictor.record_roi(1.5);
        predictor.record_roi(2.3);
        predictor.record_roi(0.8);

        assert_eq!(predictor.roi_history.len(), 3);
    }

    #[test]
    fn ml_predictor_calculates_volatility() {
        let mut predictor = SimpleMLPredictor::new();
        predictor.record_odds(2.0);
        predictor.record_odds(2.0);
        predictor.record_odds(2.0);

        let volatility = predictor.calculate_volatility();
        assert!(volatility < 0.01);
    }

    #[test]
    fn ml_predictor_calculates_efficiency() {
        let mut predictor = SimpleMLPredictor::new();
        predictor.record_roi(1.0);
        predictor.record_roi(-0.5);
        predictor.record_roi(2.0);
        predictor.record_roi(-1.0);

        let efficiency = predictor.calculate_efficiency();
        assert_eq!(efficiency, 0.5);
    }

    #[test]
    fn predict_message_includes_recommendation() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        let surebet = Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Premier League".to_string(),
            home_team: "Arsenal".to_string(),
            away_team: "Chelsea".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.5,
            total_stake: 1000.0,
            legs: vec![],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        };

        let msg = bot.predict_message(&surebet);
        assert!(msg.contains("🤖 <b>ML ODDS PREDICTION</b>"));
        assert!(msg.contains("Arsenal"));
        assert!(msg.contains("Chelsea"));
        assert!(msg.contains("Confidence:"));
        assert!(msg.contains("Recommendation:"));
    }

    // ========================================================================
    // HEDGING TESTS
    // ========================================================================

    #[test]
    fn hedge_strategy_calculates_stake() {
        let hedge = calculate_hedge_strategy(&Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Test".to_string(),
            home_team: "A".to_string(),
            away_team: "B".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.0,
            total_stake: 1000.0,
            legs: vec![SurebetLeg {
                bookmaker: "test".to_string(),
                market: "test".to_string(),
                selection: "test".to_string(),
                odds: 1.5,
                line: None,
                stake: 1000.0,
                payout: 1500.0,
                url: None,
            }],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        });

        assert!(hedge.hedge_stake > 0.0);
        assert_eq!(hedge.hedge_odds, 2.0);
    }

    #[test]
    fn hedge_strategy_has_scenarios() {
        let surebet = Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Test".to_string(),
            home_team: "A".to_string(),
            away_team: "B".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.0,
            total_stake: 1000.0,
            legs: vec![SurebetLeg {
                bookmaker: "test".to_string(),
                market: "test".to_string(),
                selection: "test".to_string(),
                odds: 1.5,
                line: None,
                stake: 1000.0,
                payout: 1500.0,
                url: None,
            }],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        };

        let hedge = calculate_hedge_strategy(&surebet);
        assert_eq!(hedge.scenarios.len(), 3);
    }

    #[test]
    fn hedge_message_includes_strategy() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        let surebet = Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Premier League".to_string(),
            home_team: "Arsenal".to_string(),
            away_team: "Chelsea".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.5,
            total_stake: 1000.0,
            legs: vec![SurebetLeg {
                bookmaker: "test".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: 2.0,
                line: None,
                stake: 1000.0,
                payout: 2000.0,
                url: None,
            }],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        };

        let msg = bot.hedge_message(&surebet);
        assert!(msg.contains("🛡️ <b>HEDGING STRATEGY</b>"));
        assert!(msg.contains("Hedge Type:"));
        assert!(msg.contains("Guaranteed Profit:"));
        assert!(msg.contains("Scenarios:"));
    }

    // ========================================================================
    // ADMIN COMMANDS TESTS
    // ========================================================================

    #[test]
    fn admin_help_shows_admin_commands() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1],
        );

        let msg = bot.admin_help();
        assert!(msg.contains("🔑 <b>ADMIN COMMANDS</b>"));
        assert!(msg.contains("/setchannel"));
        assert!(msg.contains("/clearhistory"));
        assert!(msg.contains("/exportstats"));
    }

    #[test]
    fn set_min_roi_validates_input() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply1 = bot.set_min_roi(2.5);
        assert!(reply1.contains("✅"));

        let reply2 = bot.set_min_roi(-1.0);
        assert!(reply2.contains("❌"));

        let reply3 = bot.set_min_roi(100.0);
        assert!(reply3.contains("❌"));
    }

    #[test]
    fn clear_history_empties_surebets() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let surebet = Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Test".to_string(),
            home_team: "A".to_string(),
            away_team: "B".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.0,
            total_stake: 1000.0,
            legs: vec![],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        };

        bot.record_seen_surebet(&surebet);

        let reply = bot.clear_history();
        assert!(reply.contains("✅"));

        let state = bot.state.lock().expect("state poisoned");
        assert!(state.recent_surebets.is_empty());
    }

    #[test]
    fn export_stats_includes_metrics() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);
        bot.metrics().record_bus_event();
        bot.metrics().record_surebet(&Surebet {
            id: Uuid::nil(),
            sport: Sport::Football,
            league: "Test".to_string(),
            home_team: "A".to_string(),
            away_team: "B".to_string(),
            start_time: None,
            is_live: false,
            profit_percent: 2.0,
            total_stake: 1000.0,
            legs: vec![],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        });

        let msg = bot.export_stats();
        assert!(msg.contains("📥 <b>STATISTICS EXPORT</b>"));
        assert!(msg.contains("Performance:"));
        assert!(msg.contains("Uptime:"));
    }

    #[test]
    fn reply_for_text_dashboard_command() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply = bot.reply_for_text("/dashboard", Some(1));
        assert!(reply.is_some());
        assert!(reply.unwrap().contains("📊"));
    }

    #[test]
    fn reply_for_text_channels_command() {
        let bot = TelegramBot::new("token", vec![1], 0.1, false, None);

        let reply = bot.reply_for_text("/channels", Some(1));
        assert!(reply.is_some());
        assert!(reply.unwrap().contains("📡"));
    }

    #[test]
    fn admin_command_requires_admin_user() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1],
        );

        let reply_admin = bot.reply_for_text("/admin", Some(1));
        assert!(reply_admin.is_some());
        assert!(reply_admin.unwrap().contains("🔑"));

        let reply_non_admin = bot.reply_for_text("/admin", Some(999));
        assert!(reply_non_admin.is_some());
        assert!(reply_non_admin.unwrap().contains("❌"));
    }

    #[test]
    fn setchannel_command_parses_arguments() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1],
        );

        let reply = bot.reply_for_text("/setchannel high_roi 3.0 5.0", Some(1));
        assert!(reply.is_some());
        assert!(reply.unwrap().contains("✅"));
    }

    #[test]
    fn togglechannel_command_toggles_state() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1],
        );

        bot.reply_for_text("/setchannel test 1.0 3.0", Some(1));
        let reply = bot.reply_for_text("/togglechannel test", Some(1));
        assert!(reply.is_some());
        assert!(reply.unwrap().contains("⛔"));
    }

    #[test]
    fn setminroi_command_validates_input() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1],
        );

        let reply_valid = bot.reply_for_text("/setminroi 2.5", Some(1));
        assert!(reply_valid.is_some());
        assert!(reply_valid.unwrap().contains("✅"));

        let reply_invalid = bot.reply_for_text("/setminroi 100", Some(1));
        assert!(reply_invalid.is_some());
        assert!(reply_invalid.unwrap().contains("❌"));
    }

    #[test]
    fn roi_buckets_calculate_total() {
        let mut buckets = RoiBuckets::default();
        buckets.add_surebet(0.5);
        buckets.add_surebet(1.5);
        buckets.add_surebet(2.5);
        buckets.add_surebet(4.0);

        assert_eq!(buckets.total(), 4);
    }

    #[test]
    fn telegram_bot_initializes_with_admin_users() {
        let bot = TelegramBot::with_config(
            "token",
            vec![1],
            0.1,
            false,
            None,
            TelegramAlertConfig::default(),
            vec![1, 2, 3],
        );

        assert_eq!(bot.admin_users.len(), 3);
        assert!(bot.admin_users.contains(&1));
        assert!(bot.admin_users.contains(&2));
        assert!(bot.admin_users.contains(&3));
    }

    #[test]
    fn ml_predictor_limits_history_size() {
        let mut predictor = SimpleMLPredictor::new();

        for i in 0..60 {
            predictor.record_odds(2.0 + i as f64 * 0.01);
        }

        assert!(predictor.odds_history.len() <= 50);
    }
}
