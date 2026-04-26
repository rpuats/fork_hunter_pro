use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::{Datelike, NaiveDateTime, TimeZone, Utc};
use reqwest::Client;
use shared::odds::OddsType;
use shared::{
    DiagnosticSeverity, Event, Odd, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage,
    Sport,
};
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Zenit API parser — чистые HTTP запросы без Python/Playwright
///
/// API требует заголовки `imprinthash` и `frontversion`, захваченные из браузера.
/// Эндпоинты:
///   - Prematch: `https://zenit.win/ajax/line/printer/react`
///   - Live:     `https://zenit.win/ajax/live/printer/react`
///
/// Ответ: `{ games: { id: { c1_id, c2_id, tid, f_l: [{o, h}, ...] } }, dict: { cmd: { id: name }, tournament: { id: { name } } } }`
#[derive(Debug)]
pub struct ZenitParser {
    client: Arc<Client>,
}

#[derive(Debug, Default)]
struct ParsedPage {
    events: Vec<Event>,
    odds: Vec<Odd>,
    raw_game_ids: Vec<String>,
    discovered_game_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct ZenitSport {
    id: u64,
    count: usize,
}

impl ZenitParser {
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    const DEFAULT_IMPRINT_HASH: &str = "d01d68e5a9775b90a0c7239e7f078895";
    const DEFAULT_FRONT_VERSION: &str = "1.72.1";
    const IMPRINT_HASH_ENV: &str = "ZENIT_IMPRINT_HASH";
    const FRONT_VERSION_ENV: &str = "ZENIT_FRONT_VERSION";
    const STRICT_LIVE_KPI_TARGET: usize = 150;
    const STRICT_PREMATCH_KPI_TARGET: usize = 3000;
    const RECENT_RUNTIME_LIVE_EVENTS: usize = 182;
    const RECENT_RUNTIME_PREMATCH_EVENTS: usize = 3497;

    // Retry configuration
    const MAX_RETRIES: u32 = 3;
    const INITIAL_BACKOFF_MS: u64 = 500;
    const MAX_BACKOFF_MS: u64 = 5000;
    const REQUEST_TIMEOUT_SECS: u64 = 30;

    // Sport IDs
    const SPORT_FOOTBALL: u64 = 1;
    const SPORT_HOCKEY: u64 = 2;
    const SPORT_BASKETBALL: u64 = 3;
    const SPORT_VOLLEYBALL: u64 = 4;
    const SPORT_TENNIS: u64 = 5;
    const SPORT_TABLE_TENNIS: u64 = 6;
    const SPORT_ESPORTS: u64 = 7;
    const SPORT_FUTSAL: u64 = 8;
    const SPORT_HANDBALL: u64 = 9;
    const SPORT_BADMINTON: u64 = 11;
    const SPORT_BASEBALL: u64 = 12;
    const SPORT_MMA: u64 = 13;
    const SPORT_BOXING: u64 = 14;
    const PAGE_STEP: usize = 3000;
    const PAGE_LENGTH: &str = "3000";
    const GAME_BATCH_SIZE: usize = 200;

    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::RolloutReady,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "http_runtime_path_registered".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Zenit is registered in ParserFactory and runtime diagnostics through direct line/live JSON endpoints backed by imprinthash/frontversion headers instead of a browser bridge.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "runtime_kpi_previously_met".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "A recent Zenit runtime snapshot observed {} live and {} prematch events, proving the HTTP path can clear the older 100/2000 nightly gate.",
                        Self::RECENT_RUNTIME_LIVE_EVENTS,
                        Self::RECENT_RUNTIME_PREMATCH_EVENTS,
                    ),
                },
                ParserDiagnosticCheck {
                    code: "strict_nightly_regressed_to_zero".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: format!(
                        "Production promotion stays blocked because the latest strict nightly run returned zero Zenit events against the current {} live / {} prematch targets without a transport error, so readiness is rollout-only until the runtime regression is explained.",
                        Self::STRICT_LIVE_KPI_TARGET,
                        Self::STRICT_PREMATCH_KPI_TARGET,
                    ),
                },
            ],
        }
    }

    fn sport_referer(sport_id: u64, is_live: bool) -> &'static str {
        match (sport_id, is_live) {
            (Self::SPORT_FOOTBALL, false) => "https://zenit.win/line/football",
            (Self::SPORT_FOOTBALL, true) => "https://zenit.win/live/football",
            (Self::SPORT_HOCKEY, false) => "https://zenit.win/line/hockey",
            (Self::SPORT_HOCKEY, true) => "https://zenit.win/live/hockey",
            (Self::SPORT_BASKETBALL, false) => "https://zenit.win/line/basketball",
            (Self::SPORT_BASKETBALL, true) => "https://zenit.win/live/basketball",
            (Self::SPORT_VOLLEYBALL, false) => "https://zenit.win/line/volleyball",
            (Self::SPORT_VOLLEYBALL, true) => "https://zenit.win/live/volleyball",
            (Self::SPORT_TENNIS, false) => "https://zenit.win/line/tennis",
            (Self::SPORT_TENNIS, true) => "https://zenit.win/live/tennis",
            (Self::SPORT_TABLE_TENNIS, false) => "https://zenit.win/line/table-tennis",
            (Self::SPORT_TABLE_TENNIS, true) => "https://zenit.win/live/table-tennis",
            (Self::SPORT_ESPORTS, false) => "https://zenit.win/line/esports",
            (Self::SPORT_ESPORTS, true) => "https://zenit.win/live/esports",
            (Self::SPORT_FUTSAL, false) => "https://zenit.win/line/futsal",
            (Self::SPORT_FUTSAL, true) => "https://zenit.win/live/futsal",
            (Self::SPORT_HANDBALL, false) => "https://zenit.win/line/handball",
            (Self::SPORT_HANDBALL, true) => "https://zenit.win/live/handball",
            (Self::SPORT_BADMINTON, false) => "https://zenit.win/line/badminton",
            (Self::SPORT_BADMINTON, true) => "https://zenit.win/live/badminton",
            (Self::SPORT_BASEBALL, false) => "https://zenit.win/line/baseball",
            (Self::SPORT_BASEBALL, true) => "https://zenit.win/live/baseball",
            (Self::SPORT_MMA, false) => "https://zenit.win/line/mma",
            (Self::SPORT_MMA, true) => "https://zenit.win/live/mma",
            (Self::SPORT_BOXING, false) => "https://zenit.win/line/boxing",
            (Self::SPORT_BOXING, true) => "https://zenit.win/live/boxing",
            _ if is_live => "https://zenit.win/live/football",
            _ => "https://zenit.win/line/football",
        }
    }

    fn collect_game_ids(value: &serde_json::Value, game_ids: &mut HashSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(games) = map.get("games").and_then(|value| value.as_array()) {
                    for game_id in games {
                        if let Some(game_id) = game_id.as_u64() {
                            game_ids.insert(game_id.to_string());
                        } else if let Some(game_id) = game_id.as_str() {
                            game_ids.insert(game_id.to_string());
                        }
                    }
                }

                for child in map.values() {
                    Self::collect_game_ids(child, game_ids);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    Self::collect_game_ids(item, game_ids);
                }
            }
            _ => {}
        }
    }

    fn imprinthash() -> String {
        env::var(Self::IMPRINT_HASH_ENV).unwrap_or_else(|_| Self::DEFAULT_IMPRINT_HASH.to_string())
    }

    fn frontversion() -> String {
        env::var(Self::FRONT_VERSION_ENV)
            .unwrap_or_else(|_| Self::DEFAULT_FRONT_VERSION.to_string())
    }

    fn line_query(offset: usize, sport_id: u64, games: Option<&str>) -> Vec<(String, String)> {
        vec![
            ("all".to_string(), "0".to_string()),
            ("onlyview".to_string(), "0".to_string()),
            ("timeline".to_string(), "0".to_string()),
            ("tournaments_mode".to_string(), "1".to_string()),
            ("sport".to_string(), sport_id.to_string()),
            ("tournament".to_string(), String::new()),
            ("tournament_region".to_string(), String::new()),
            ("tournament_info".to_string(), String::new()),
            ("league".to_string(), String::new()),
            ("games".to_string(), games.unwrap_or_default().to_string()),
            ("ross".to_string(), "0".to_string()),
            ("lang_id".to_string(), "1".to_string()),
            ("timezone".to_string(), "3".to_string()),
            ("offset".to_string(), offset.to_string()),
            ("show_from_main".to_string(), "0".to_string()),
            ("client_v".to_string(), String::new()),
            ("length".to_string(), Self::PAGE_LENGTH.to_string()),
            ("sort_mode".to_string(), "2".to_string()),
            ("b_id".to_string(), String::new()),
            ("popular".to_string(), "1".to_string()),
        ]
    }

    fn live_query() -> Vec<(String, String)> {
        vec![
            ("ross".to_string(), "0".to_string()),
            ("all".to_string(), "0".to_string()),
            ("timezone".to_string(), "3".to_string()),
            ("lang_id".to_string(), "1".to_string()),
            ("sort_mode".to_string(), "2".to_string()),
            ("onlyview".to_string(), "0".to_string()),
            ("print_mode".to_string(), "react".to_string()),
            ("show_from_main".to_string(), "0".to_string()),
        ]
    }

    fn left_menu_query() -> [(&'static str, &'static str); 3] {
        [
            ("lang_id", "1"),
            ("sort_mode", "2"),
            ("tournaments_mode", "1"),
        ]
    }

    /// Check if error is transient and worth retrying
    fn is_transient_error(error: &str) -> bool {
        error.contains("timeout")
            || error.contains("connection")
            || error.contains("ConnectError")
            || error.contains("429")
            || error.contains("502")
            || error.contains("503")
            || error.contains("504")
            || error.contains("Temporary failure")
            || error.contains("Too Many Requests")
    }

    /// Calculate backoff duration with exponential growth
    fn backoff_duration(attempt: u32) -> Duration {
        let base_ms = Self::INITIAL_BACKOFF_MS;
        let backoff_ms = base_ms * 2_u64.pow(attempt);
        let capped_ms = backoff_ms.min(Self::MAX_BACKOFF_MS);
        Duration::from_millis(capped_ms)
    }

    /// Retry helper for network operations with exponential backoff
    async fn retry_with_backoff<F, Fut, T>(
        &self,
        description: &str,
        mut operation: F,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
    {
        let mut last_error: Option<String> = None;

        for attempt in 0..Self::MAX_RETRIES {
            debug!(attempt, description, "Zenit retry attempt");

            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            attempt,
                            description, "Zenit operation succeeded after retries"
                        );
                    }
                    return Ok(result);
                }
                Err(err) => {
                    let err_str = err.to_string();
                    last_error = Some(err_str.clone());

                    if !Self::is_transient_error(&err_str) {
                        error!(
                            attempt,
                            error = &err_str,
                            description,
                            "Zenit permanent error (not retrying)"
                        );
                        return Err(err);
                    }

                    if attempt < Self::MAX_RETRIES - 1 {
                        let backoff = Self::backoff_duration(attempt);
                        warn!(
                            attempt,
                            error = &err_str,
                            backoff_ms = backoff.as_millis(),
                            description,
                            "Zenit transient error, retrying after backoff"
                        );
                        sleep(backoff).await;
                    } else {
                        error!(
                            attempt,
                            error = &err_str,
                            max_retries = Self::MAX_RETRIES,
                            description,
                            "Zenit operation failed after all retries"
                        );
                    }
                }
            }
        }

        Err(format!(
            "Zenit {}: {} (failed after {} retries)",
            description,
            last_error.unwrap_or_else(|| "unknown error".to_string()),
            Self::MAX_RETRIES
        )
        .into())
    }

    fn parse_u64_value(value: &serde_json::Value) -> Option<u64> {
        match value {
            serde_json::Value::Number(number) => number.as_u64(),
            serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        }
    }

    fn parse_string_id(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::Number(number) => Some(number.to_string()),
            serde_json::Value::String(text) => {
                let trimmed = text.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }
            _ => None,
        }
    }

    fn parse_start_time(
        game_obj: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<chrono::DateTime<Utc>> {
        game_obj
            .get("time")
            .and_then(Self::parse_i64_value)
            .and_then(|value| Utc.timestamp_opt(value, 0).single())
            .or_else(|| game_obj.get("date").and_then(Self::parse_date_value))
    }

    fn parse_i64_value(value: &serde_json::Value) -> Option<i64> {
        match value {
            serde_json::Value::Number(number) => number.as_i64(),
            serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        }
    }

    fn parse_date_value(value: &serde_json::Value) -> Option<chrono::DateTime<Utc>> {
        let text = value.as_str()?.trim();
        if text.is_empty() {
            return None;
        }

        if let Ok(timestamp) = text.parse::<i64>() {
            if timestamp > 0 {
                return Utc.timestamp_opt(timestamp, 0).single();
            }
        }

        if let Ok(date_time) = chrono::DateTime::parse_from_rfc3339(text) {
            return Some(date_time.with_timezone(&Utc));
        }

        for format in [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d %H:%M",
        ] {
            if let Ok(date_time) = NaiveDateTime::parse_from_str(text, format) {
                return Some(Utc.from_utc_datetime(&date_time));
            }
        }

        Self::parse_short_date(text)
    }

    fn parse_short_date(text: &str) -> Option<chrono::DateTime<Utc>> {
        let mut parts = text.split_whitespace();
        let raw_date = parts.next()?;
        let raw_time = parts.next()?;
        if parts.next().is_some() {
            return None;
        }

        let mut date_parts = raw_date.split('/');
        let first = date_parts.next()?.trim().parse::<u32>().ok()?;
        let second = date_parts.next()?.trim().parse::<u32>().ok()?;
        if date_parts.next().is_some() {
            return None;
        }

        let mut time_parts = raw_time.split(':');
        let hour = time_parts.next()?.trim().parse::<u32>().ok()?;
        let minute = time_parts.next()?.trim().parse::<u32>().ok()?;
        if time_parts.next().is_some() {
            return None;
        }

        let now = Utc::now();
        let year = now.year();
        let candidates = if first > 12 {
            [(second, first), (first, second)]
        } else if second > 12 {
            [(first, second), (second, first)]
        } else {
            [(second, first), (first, second)]
        };

        for (month, day) in candidates {
            if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day)
                .and_then(|date| date.and_hms_opt(hour, minute, 0))
            {
                let mut parsed = Utc.from_utc_datetime(&date);
                if parsed < now - chrono::TimeDelta::days(180) {
                    if let Some(next_year) = chrono::NaiveDate::from_ymd_opt(year + 1, month, day)
                        .and_then(|date| date.and_hms_opt(hour, minute, 0))
                    {
                        parsed = Utc.from_utc_datetime(&next_year);
                    }
                }
                return Some(parsed);
            }
        }

        None
    }

    fn adjacent_line_value(
        bets: &[serde_json::Value],
        index: usize,
        prefer_previous: bool,
    ) -> Option<f64> {
        let previous = index
            .checked_sub(1)
            .and_then(|prev| bets.get(prev))
            .and_then(|value| value.get("h"))
            .and_then(Self::parse_numeric_value);
        let next = bets
            .get(index + 1)
            .and_then(|value| value.get("h"))
            .and_then(Self::parse_numeric_value);

        if prefer_previous {
            previous.or(next)
        } else {
            next.or(previous)
        }
    }

    async fn fetch_page(
        &self,
        base_url: &str,
        sport_id: u64,
        is_live: bool,
        offset: usize,
        games: Option<&str>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let referer = Self::sport_referer(sport_id, is_live);
        let imprinthash = Self::imprinthash();
        let frontversion = Self::frontversion();
        let query = Self::line_query(offset, sport_id, games);

        let operation = || async {
            debug!(
                base_url,
                sport = sport_id,
                is_live,
                offset,
                has_games = games.is_some(),
                imprinthash,
                frontversion,
                "Zenit fetch_page request"
            );

            let resp = self
                .client
                .get(base_url)
                .timeout(Duration::from_secs(Self::REQUEST_TIMEOUT_SECS))
                .header("User-Agent", Self::USER_AGENT)
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header(
                    "sec-ch-ua",
                    "\"Not:A-Brand\";v=\"99\", \"Chromium\";v=\"145\", \"HeadlessChrome\";v=\"145\"",
                )
                .header("sec-ch-ua-mobile", "?0")
                .header("sec-ch-ua-platform", "\"Windows\"")
                .header("Referer", referer)
                .header("X-Requested-With", "XMLHttpRequest")
                .header("imprinthash", &imprinthash)
                .header("frontversion", &frontversion)
                .query(&query)
                .send()
                .await
                .map_err(|e| {
                    error!(error = %e, base_url, sport = sport_id, "Zenit fetch_page HTTP error");
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                })?;

            let status = resp.status();
            debug!(status = %status, sport = sport_id, offset, "Zenit fetch_page response");

            if !status.is_success() {
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read body>".to_string());
                let error_msg = format!(
                    "Zenit API returned HTTP {} for sport {} at offset {}. Body: {}",
                    status, sport_id, offset, body
                );
                error!(error = &error_msg, "Zenit fetch_page HTTP error");
                return Err(error_msg.into());
            }

            resp.json::<serde_json::Value>().await.map_err(|e| {
                error!(error = %e, sport = sport_id, "Zenit fetch_page JSON parse error");
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })
        };

        self.retry_with_backoff(
            &format!("fetch_page(sport={}, offset={})", sport_id, offset),
            operation,
        )
        .await
    }

    async fn fetch_live_page(
        &self,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let query = Self::live_query();

        let operation = || async {
            debug!("Zenit fetch_live_page request");

            let resp = self
                .client
                .get("https://zenit.win/ajax/live/printer/react")
                .timeout(Duration::from_secs(Self::REQUEST_TIMEOUT_SECS))
                .header("User-Agent", Self::USER_AGENT)
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header(
                    "sec-ch-ua",
                    "\"Not:A-Brand\";v=\"99\", \"Chromium\";v=\"145\", \"HeadlessChrome\";v=\"145\"",
                )
                .header("sec-ch-ua-mobile", "?0")
                .header("sec-ch-ua-platform", "\"Windows\"")
                .header("Referer", "https://zenit.win/live/football")
                .header("X-Requested-With", "XMLHttpRequest")
                .header("imprinthash", Self::imprinthash())
                .header("frontversion", Self::frontversion())
                .query(&query)
                .send()
                .await
                .map_err(|e| {
                    error!(error = %e, "Zenit fetch_live_page HTTP error");
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                })?;

            let status = resp.status();
            debug!(status = %status, "Zenit fetch_live_page response");

            if !status.is_success() {
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read body>".to_string());
                let error_msg = format!("Zenit live API returned HTTP {}. Body: {}", status, body);
                error!(error = &error_msg, "Zenit fetch_live_page HTTP error");
                return Err(error_msg.into());
            }

            resp.json::<serde_json::Value>().await.map_err(|e| {
                error!(error = %e, "Zenit fetch_live_page JSON parse error");
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })
        };

        self.retry_with_backoff("fetch_live_page", operation).await
    }

    async fn fetch_available_sports(
        &self,
    ) -> Result<Vec<ZenitSport>, Box<dyn std::error::Error + Send + Sync>> {
        let operation = || async {
            debug!("Zenit fetch_available_sports request");

            let resp = self
                .client
                .get("https://zenit.win/ajax/line/left_menu/get")
                .timeout(Duration::from_secs(Self::REQUEST_TIMEOUT_SECS))
                .header("User-Agent", Self::USER_AGENT)
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header(
                    "sec-ch-ua",
                    "\"Not:A-Brand\";v=\"99\", \"Chromium\";v=\"145\", \"HeadlessChrome\";v=\"145\"",
                )
                .header("sec-ch-ua-mobile", "?0")
                .header("sec-ch-ua-platform", "\"Windows\"")
                .header("Referer", "https://zenit.win/line/football")
                .header("X-Requested-With", "XMLHttpRequest")
                .header("imprinthash", Self::imprinthash())
                .header("frontversion", Self::frontversion())
                .query(&Self::left_menu_query())
                .send()
                .await
                .map_err(|e| {
                    error!(error = %e, "Zenit fetch_available_sports HTTP error");
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                })?;

            let status = resp.status();
            debug!(status = %status, "Zenit fetch_available_sports response");

            if !status.is_success() {
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read body>".to_string());
                let error_msg = format!("Zenit left menu returned HTTP {}. Body: {}", status, body);
                error!(
                    error = &error_msg,
                    "Zenit fetch_available_sports HTTP error"
                );
                return Err(error_msg.into());
            }

            resp.json::<serde_json::Value>().await.map_err(|e| {
                error!(error = %e, "Zenit fetch_available_sports JSON parse error");
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })
        };

        let json = self
            .retry_with_backoff("fetch_available_sports", operation)
            .await?;

        let sports = json
            .get("result")
            .and_then(|value| value.get("sport"))
            .and_then(|value| value.as_array())
            .map(|sports| {
                sports
                    .iter()
                    .filter_map(|sport| {
                        Some(ZenitSport {
                            id: sport.get("id")?.as_u64()?,
                            count: sport
                                .get("count")
                                .and_then(|value| value.as_u64())
                                .unwrap_or(0) as usize,
                        })
                    })
                    .filter(|sport| sport.count > 0)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        debug!(sports_count = sports.len(), "Zenit available sports parsed");
        Ok(sports)
    }

    fn parse_numeric_value(value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(text) => text.trim().replace(',', ".").parse::<f64>().ok(),
            _ => None,
        }
    }

    fn parse_game_dict(
        dict: &serde_json::Map<String, serde_json::Value>,
    ) -> HashMap<String, String> {
        dict.iter()
            .filter_map(|(key, value)| {
                value
                    .as_str()
                    .map(|name| (key.clone(), name.to_string()))
                    .or_else(|| {
                        value
                            .get("name")
                            .and_then(|name| name.as_str())
                            .map(|name| (key.clone(), name.to_string()))
                    })
            })
            .collect()
    }

    fn resolve_league(
        game_obj: &serde_json::Map<String, serde_json::Value>,
        league_names: &HashMap<String, String>,
        tournament_names: &HashMap<String, String>,
        region_names: &HashMap<String, String>,
        info_names: &HashMap<String, String>,
    ) -> String {
        let lid = game_obj.get("lid").and_then(|value| value.as_u64());
        let rid = game_obj.get("rid").and_then(|value| value.as_u64());
        let tid = game_obj.get("tid").and_then(|value| value.as_u64());
        let ti_id = game_obj.get("ti_id").and_then(|value| value.as_u64());

        if let Some(name) = lid
            .map(|value| value.to_string())
            .and_then(|key| league_names.get(&key).cloned())
        {
            return name;
        }

        let mut parts = Vec::new();

        if let Some(region) = rid
            .map(|value| value.to_string())
            .and_then(|key| region_names.get(&key).cloned())
        {
            parts.push(region);
        }

        if let Some(tournament) = tid
            .map(|value| value.to_string())
            .and_then(|key| tournament_names.get(&key).cloned())
        {
            if !parts.iter().any(|part| part == &tournament) {
                parts.push(tournament);
            }
        }

        if let Some(info) = ti_id
            .map(|value| value.to_string())
            .and_then(|key| info_names.get(&key).cloned())
        {
            if !parts.iter().any(|part| part == &info) {
                parts.push(info);
            }
        }

        if parts.is_empty() {
            "Unknown".to_string()
        } else {
            parts.join(". ")
        }
    }

    /// Fetch events from a single endpoint (line or live) for a specific sport
    async fn fetch_sport(
        &self,
        base_url: &str,
        sport_id: u64,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen_raw_games = HashSet::new();
        let mut seen_events = HashSet::new();
        let mut seen_odds = HashSet::new();
        let mut discovered_game_ids = HashSet::new();
        let mut offset = 0usize;
        let mut expected_total: Option<usize> = None;

        loop {
            let json = self
                .fetch_page(base_url, sport_id, is_live, offset, None)
                .await?;
            if json.is_null() {
                break;
            }

            if json.get("errorCode").is_some() {
                let msg = json
                    .get("msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                warn!(
                    error = msg,
                    sport = sport_id,
                    is_live,
                    offset,
                    "Zenit API application error"
                );
                break;
            }

            if expected_total.is_none() {
                expected_total = json
                    .get("sport")
                    .and_then(|value| value.as_array())
                    .and_then(|sports| {
                        sports.iter().find_map(|sport| {
                            let id = sport.get("id").and_then(|value| value.as_u64())?;
                            if id == sport_id {
                                sport
                                    .get("count")
                                    .and_then(|value| value.as_u64())
                                    .map(|value| value as usize)
                            } else {
                                None
                            }
                        })
                    });
            }

            let parsed = Self::parse_response(&json, Some(sport_id), is_live);
            let previous_raw_count = seen_raw_games.len();

            for game_id in &parsed.discovered_game_ids {
                discovered_game_ids.insert(game_id.clone());
            }

            for raw_game_id in parsed.raw_game_ids {
                seen_raw_games.insert(raw_game_id);
            }

            for event in parsed.events {
                if seen_events.insert(event.id.clone()) {
                    all_events.push(event);
                }
            }

            for odd in parsed.odds {
                if seen_odds.insert(odd.id.clone()) {
                    all_odds.push(odd);
                }
            }

            if is_live {
                break;
            }

            let current_raw_count = seen_raw_games.len();
            let reached_expected_total = expected_total
                .map(|expected| current_raw_count >= expected)
                .unwrap_or(false);
            let page_added_new_games = current_raw_count > previous_raw_count;

            if reached_expected_total || !page_added_new_games {
                break;
            }

            offset += Self::PAGE_STEP;
            if expected_total
                .map(|expected| offset > expected + Self::PAGE_STEP)
                .unwrap_or(offset > 2000)
            {
                break;
            }
        }

        let missing_game_ids = discovered_game_ids
            .iter()
            .filter(|game_id| !seen_raw_games.contains(*game_id))
            .cloned()
            .collect::<Vec<_>>();

        if !is_live && !missing_game_ids.is_empty() {
            debug!(
                sport = sport_id,
                missing = missing_game_ids.len(),
                discovered = discovered_game_ids.len(),
                parsed = seen_raw_games.len(),
                "Zenit fetching missing prematch games"
            );

            for chunk in missing_game_ids.chunks(Self::GAME_BATCH_SIZE) {
                let games = chunk.join("-");
                let json = match self
                    .fetch_page(base_url, sport_id, false, 0, Some(&games))
                    .await
                {
                    Ok(json) => json,
                    Err(error) => {
                        warn!(error = %error, sport = sport_id, batch = chunk.len(), "Zenit missing games fetch failed");
                        continue;
                    }
                };

                if json.is_null() {
                    continue;
                }

                let parsed = Self::parse_response(&json, Some(sport_id), false);

                for raw_game_id in parsed.raw_game_ids {
                    seen_raw_games.insert(raw_game_id);
                }

                for event in parsed.events {
                    if seen_events.insert(event.id.clone()) {
                        all_events.push(event);
                    }
                }

                for odd in parsed.odds {
                    if seen_odds.insert(odd.id.clone()) {
                        all_odds.push(odd);
                    }
                }
            }
        }

        Ok((all_events, all_odds))
    }

    async fn fetch_live(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let json = self.fetch_live_page().await?;
        if json.is_null() {
            return Ok((Vec::new(), Vec::new()));
        }

        if json.get("errorCode").is_some() {
            let msg = json
                .get("msg")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            return Err(format!("Zenit live API application error: {msg}").into());
        }

        let parsed = Self::parse_response(&json, None, true);
        Ok((parsed.events, parsed.odds))
    }

    /// Parse the JSON response from Zenit API
    fn parse_response(
        json: &serde_json::Value,
        sport_id: Option<u64>,
        is_live: bool,
    ) -> ParsedPage {
        let mut parsed = ParsedPage::default();
        let now = Utc::now();

        let games = match json.get("games").and_then(|v| v.as_object()) {
            Some(g) => g,
            None => return parsed,
        };

        let dict = match json.get("dict").and_then(|v| v.as_object()) {
            Some(d) => d,
            None => return parsed,
        };

        let team_names = dict
            .get("cmd")
            .and_then(|v| v.as_object())
            .map(Self::parse_game_dict)
            .unwrap_or_default();

        let league_names = dict
            .get("league")
            .and_then(|v| v.as_object())
            .map(Self::parse_game_dict)
            .unwrap_or_default();
        let tournament_names = dict
            .get("tournament")
            .and_then(|v| v.as_object())
            .map(Self::parse_game_dict)
            .unwrap_or_default();
        let region_names = dict
            .get("tournament_region")
            .and_then(|v| v.as_object())
            .map(Self::parse_game_dict)
            .unwrap_or_default();
        let info_names = dict
            .get("tournament_info")
            .and_then(|v| v.as_object())
            .map(Self::parse_game_dict)
            .unwrap_or_default();

        let mut discovered_game_ids = HashSet::new();

        if let Some(sports) = json.get("sport") {
            Self::collect_game_ids(sports, &mut discovered_game_ids);
        }

        for (game_id, game) in games {
            let game_obj = match game.as_object() {
                Some(g) => g,
                None => continue,
            };

            parsed.raw_game_ids.push(game_id.clone());

            let c1_id = game_obj.get("c1_id").and_then(Self::parse_string_id);
            let c2_id = game_obj.get("c2_id").and_then(Self::parse_string_id);

            let (home, away) = match (c1_id, c2_id) {
                (Some(id1), Some(id2)) => {
                    let h = team_names.get(&id1).cloned().unwrap_or_default();
                    let a = team_names.get(&id2).cloned().unwrap_or_default();
                    if h.is_empty() || a.is_empty() {
                        continue;
                    }
                    (h, a)
                }
                _ => continue,
            };

            let league = Self::resolve_league(
                game_obj,
                &league_names,
                &tournament_names,
                &region_names,
                &info_names,
            );

            let event_sport_id = game_obj
                .get("sid")
                .and_then(Self::parse_u64_value)
                .or(sport_id)
                .unwrap_or_default();
            let sport = Self::sport_id_to_sport(event_sport_id);

            let start_time = Self::parse_start_time(game_obj);

            let event_id = format!("zenit-{}", game_id);

            let headers: Vec<String> = game_obj
                .get("hd")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            item.get("n")
                                .and_then(|value| value.as_str())
                                .unwrap_or_default()
                                .to_string()
                        })
                        .collect()
                })
                .unwrap_or_default();

            let odds_data = game_obj.get("f_l").and_then(|value| value.as_array());

            let mut event_odds = Vec::new();

            if let Some(bets) = odds_data {
                for (index, bet) in bets.iter().enumerate() {
                    let bet_obj = match bet.as_object() {
                        Some(b) => b,
                        None => continue,
                    };

                    let header = headers
                        .get(index)
                        .map(|value| value.as_str())
                        .unwrap_or_default();
                    let odd_value = bet_obj
                        .get("h")
                        .and_then(Self::parse_numeric_value)
                        .filter(|value| *value > 1.0);

                    let Some(odd_value) = odd_value else {
                        continue;
                    };

                    let odd_id = bet_obj
                        .get("id")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| {
                            format!(
                                "{}-{}-{}",
                                event_id,
                                bet_obj
                                    .get("o")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("na"),
                                index
                            )
                        });

                    let line = match bet_obj.get("o").and_then(|value| value.as_str()) {
                        Some("7" | "8" | "10") => Self::adjacent_line_value(bets, index, true),
                        Some("9") => Self::adjacent_line_value(bets, index, false),
                        _ => None,
                    };

                    let mapped = match bet_obj.get("o").and_then(|value| value.as_str()) {
                        Some("1") if header == "1" => Some(("1X2", "1", OddsType::Home, None)),
                        Some("2") if header == "X" || header == "Х" => {
                            Some(("1X2", "X", OddsType::Draw, None))
                        }
                        Some("3") if header == "2" => Some(("1X2", "2", OddsType::Away, None)),
                        Some("7") if header == "1" => {
                            Some(("Handicap", "1", OddsType::Handicap, line))
                        }
                        Some("8") if header == "2" => {
                            Some(("Handicap", "2", OddsType::Handicap, line))
                        }
                        Some("9") if header == "М" => Some(("Total", "U", OddsType::Under, line)),
                        Some("10") if header == "Б" => Some(("Total", "O", OddsType::Over, line)),
                        _ => None,
                    };

                    let Some((market, selection, odds_type, line)) = mapped else {
                        continue;
                    };

                    event_odds.push(Odd {
                        id: odd_id,
                        event_id: event_id.clone(),
                        bookmaker_slug: "zenit".to_string(),
                        market: market.into(),
                        selection: selection.into(),
                        odds: odd_value,
                        odds_type,
                        line,
                        timestamp: now,
                    });
                }
            }

            parsed.odds.extend(event_odds);

            parsed.events.push(Event {
                id: event_id.clone(),
                sport,
                league,
                home_team: home.clone(),
                away_team: away.clone(),
                start_time,
                is_live,
                bookmaker_slug: "zenit".to_string(),
                raw_url: Some("https://zenit.win".to_string()),
                extra: HashMap::new(),
            });
        }

        parsed.discovered_game_ids = discovered_game_ids.into_iter().collect();

        parsed
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let result = self.fetch_all().await?;
        Ok((result.events, result.odds))
    }

    fn sport_id_to_sport(id: u64) -> Sport {
        match id {
            1 => Sport::Football,
            2 => Sport::Hockey,
            3 => Sport::Basketball,
            4 => Sport::Volleyball,
            5 => Sport::Tennis,
            6 => Sport::TableTennis,
            7 => Sport::Esports,
            8 => Sport::Futsal,
            9 => Sport::Handball,
            11 => Sport::Badminton,
            12 => Sport::Baseball,
            13 => Sport::Mma,
            14 => Sport::Boxing,
            _ => Sport::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ZenitParser;
    use chrono::{Datelike, Timelike, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn is_transient_error_detects_timeout() {
        assert!(ZenitParser::is_transient_error("timeout"));
        assert!(ZenitParser::is_transient_error("operation timed out"));
        assert!(ZenitParser::is_transient_error("request timeout"));
    }

    #[test]
    fn is_transient_error_detects_connection_errors() {
        assert!(ZenitParser::is_transient_error("connection reset"));
        assert!(ZenitParser::is_transient_error("ConnectError"));
        assert!(ZenitParser::is_transient_error(
            "Temporary failure in name resolution"
        ));
    }

    #[test]
    fn is_transient_error_detects_server_errors() {
        assert!(ZenitParser::is_transient_error("429"));
        assert!(ZenitParser::is_transient_error("502"));
        assert!(ZenitParser::is_transient_error("503"));
        assert!(ZenitParser::is_transient_error("504"));
        assert!(ZenitParser::is_transient_error("Too Many Requests"));
    }

    #[test]
    fn is_transient_error_rejects_permanent_errors() {
        assert!(!ZenitParser::is_transient_error("404 Not Found"));
        assert!(!ZenitParser::is_transient_error("400 Bad Request"));
        assert!(!ZenitParser::is_transient_error("401 Unauthorized"));
        assert!(!ZenitParser::is_transient_error("JSON parsing error"));
    }

    #[test]
    fn backoff_duration_increases_exponentially() {
        let d0 = ZenitParser::backoff_duration(0).as_millis() as u64;
        let d1 = ZenitParser::backoff_duration(1).as_millis() as u64;
        let d2 = ZenitParser::backoff_duration(2).as_millis() as u64;

        assert_eq!(d0, 500); // INITIAL_BACKOFF_MS
        assert_eq!(d1, 1000); // 500 * 2^1
        assert_eq!(d2, 2000); // 500 * 2^2

        // Test that it caps at MAX_BACKOFF_MS
        let d_high = ZenitParser::backoff_duration(10).as_millis() as u64;
        assert_eq!(d_high, 5000); // MAX_BACKOFF_MS
    }

    #[test]
    fn line_query_matches_browser_capture_shape() {
        let query = ZenitParser::line_query(150, 3, Some("111-222"));
        let query = query
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(query.get("all").map(String::as_str), Some("0"));
        assert_eq!(query.get("onlyview").map(String::as_str), Some("0"));
        assert_eq!(query.get("popular").map(String::as_str), Some("1"));
        assert_eq!(query.get("length").map(String::as_str), Some("3000"));
        assert_eq!(query.get("offset").map(String::as_str), Some("150"));
        assert_eq!(query.get("sport").map(String::as_str), Some("3"));
        assert_eq!(query.get("games").map(String::as_str), Some("111-222"));
        assert!(!query.contains_key("dict"));
        assert!(!query.contains_key("pagination"));
    }

    #[test]
    fn parse_response_supports_string_dates_and_numeric_strings() {
        let payload = json!({
            "games": {
                "42": {
                    "sid": "5",
                    "lid": 100,
                    "rid": "7",
                    "tid": "12",
                    "time": "1776031200",
                    "date": "13/04 01:00",
                    "c1_id": "101",
                    "c2_id": 202,
                    "f_l": [
                        {"o": "1", "h": "1.70", "id": "o1"},
                        {},
                        {"o": "3", "h": "2.05", "id": "o3"},
                        {},
                        {},
                        {},
                        {"h": "-2.5"},
                        {"o": "7", "h": "1.90", "id": "o7"},
                        {"h": "2.5"},
                        {"o": "8", "h": "1.85", "id": "o8"},
                        {"o": "9", "h": "1.80", "id": "o9"},
                        {"h": "22.5"},
                        {"o": "10", "h": "1.95", "id": "o10"}
                    ],
                    "hd": [
                        {"n": "1"}, {"n": "Х"}, {"n": "2"}, {"n": "1Х"}, {"n": "12"}, {"n": "Х2"},
                        {"n": "Фора"}, {"n": "1"}, {"n": "Фора"}, {"n": "2"}, {"n": "М"}, {"n": "Тотал"}, {"n": "Б"}
                    ]
                }
            },
            "dict": {
                "cmd": {"101": "Home", "202": "Away"},
                "league": {"100": "League"},
                "tournament": {"12": {"name": "Tournament"}},
                "tournament_region": {"7": {"name": "Region"}},
                "tournament_info": {}
            }
        });

        let parsed = ZenitParser::parse_response(&payload, Some(5), false);

        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.odds.len(), 6);
        assert_eq!(parsed.events[0].home_team, "Home");
        assert_eq!(parsed.events[0].away_team, "Away");
        assert_eq!(parsed.events[0].league, "League");
        assert_eq!(
            parsed.events[0].start_time.map(|dt| dt.timestamp()),
            Some(1776031200)
        );
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "1X2" && odd.selection == "1"));
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "1X2" && odd.selection == "2"));
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "Handicap" && odd.selection == "1" && odd.line == Some(-2.5)));
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "Handicap" && odd.selection == "2" && odd.line == Some(2.5)));
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "Total" && odd.selection == "U" && odd.line == Some(22.5)));
        assert!(parsed
            .odds
            .iter()
            .any(|odd| odd.market == "Total" && odd.selection == "O" && odd.line == Some(22.5)));
    }

    #[test]
    fn parse_date_value_accepts_short_formats() {
        let now = Utc::now();

        let parsed =
            ZenitParser::parse_date_value(&json!("04/13 01:00")).expect("short date parses");

        assert_eq!(parsed.hour(), 1);
        assert_eq!(parsed.minute(), 0);
        assert_eq!(parsed.month(), 4);
        assert_eq!(parsed.day(), 13);
        assert!(parsed.year() == now.year() || parsed.year() == now.year() + 1);
    }

    #[tokio::test]
    #[ignore = "runtime-only network diagnostic"]
    async fn zenit_runtime_counts_against_live_output() {
        let client = Arc::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent(ZenitParser::USER_AGENT)
                .build()
                .expect("client"),
        );
        let parser = ZenitParser::new(client);
        let (events, _odds) = parser.fetch_runtime_data().await.expect("runtime fetch");
        let live = events.iter().filter(|event| event.is_live).count();
        let prematch = events.len().saturating_sub(live);

        println!(
            "zenit runtime counts: live={}, prematch={}, total={}",
            live,
            prematch,
            events.len()
        );
    }

    #[tokio::test]
    #[ignore = "runtime-only network diagnostic"]
    async fn zenit_runtime_request_branch_probe() {
        let client = Arc::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(40))
                .user_agent(ZenitParser::USER_AGENT)
                .build()
                .expect("client"),
        );
        let parser = ZenitParser::new(client);

        let sports = parser
            .fetch_available_sports()
            .await
            .expect("left menu fetch");
        let line = parser
            .fetch_page(
                "https://zenit.win/ajax/line/printer/react",
                ZenitParser::SPORT_FOOTBALL,
                false,
                0,
                None,
            )
            .await
            .expect("football line fetch");
        let live = parser.fetch_live_page().await.expect("live fetch");

        let line_raw_games = line
            .get("games")
            .and_then(|value| value.as_object())
            .map(|games| games.len())
            .unwrap_or_default();
        let live_raw_games = live
            .get("games")
            .and_then(|value| value.as_object())
            .map(|games| games.len())
            .unwrap_or_default();
        let line_parsed =
            ZenitParser::parse_response(&line, Some(ZenitParser::SPORT_FOOTBALL), false);
        let live_parsed = ZenitParser::parse_response(&live, None, true);

        println!(
            "zenit branch probe: sports={}, football_line_raw={}, football_line_events={}, live_raw={}, live_events={}",
            sports.len(),
            line_raw_games,
            line_parsed.events.len(),
            live_raw_games,
            live_parsed.events.len()
        );

        assert!(
            !sports.is_empty(),
            "left_menu branch returned zero sports; prematch fanout would be skipped"
        );
        assert!(
            !line_parsed.events.is_empty(),
            "football line branch returned zero parsed events"
        );
        assert!(
            !live_parsed.events.is_empty(),
            "live branch returned zero parsed events"
        );
    }
}

#[async_trait]
impl BookmakerParser for ZenitParser {
    fn name(&self) -> &str {
        "Zenit"
    }
    fn slug(&self) -> &str {
        "zenit"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(Self::readiness_snapshot())
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();
        let sport_ids = self.fetch_available_sports().await.unwrap_or_else(|error| {
            warn!(error = %error, "Zenit left menu fetch failed, falling back to core sports");
            vec![
                ZenitSport {
                    id: Self::SPORT_FOOTBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_HOCKEY,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_BASKETBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_TENNIS,
                    count: 0,
                },
            ]
        });

        for sport in sport_ids {
            // Prematch
            match self
                .fetch_sport("https://zenit.win/ajax/line/printer/react", sport.id, false)
                .await
            {
                Ok((events, _)) => {
                    debug!(
                        count = events.len(),
                        sport = sport.id,
                        expected = sport.count,
                        "Zenit prematch"
                    );
                    all_events.extend(events);
                }
                Err(e) => warn!(error = %e, sport = sport.id, "Zenit prematch failed"),
            }
        }

        match self.fetch_live().await {
            Ok((events, _)) => {
                debug!(count = events.len(), "Zenit live");
                all_events.extend(events);
            }
            Err(e) => warn!(error = %e, "Zenit live failed"),
        }

        info!(count = all_events.len(), "Zenit events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();
        let sport_ids = self.fetch_available_sports().await.unwrap_or_else(|_| {
            vec![
                ZenitSport {
                    id: Self::SPORT_FOOTBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_HOCKEY,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_BASKETBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_TENNIS,
                    count: 0,
                },
            ]
        });

        for sport in sport_ids {
            if let Ok((_, odds)) = self
                .fetch_sport("https://zenit.win/ajax/line/printer/react", sport.id, false)
                .await
            {
                all_odds.extend(odds);
            }
        }

        if let Ok((_, odds)) = self.fetch_live().await {
            all_odds.extend(odds);
        }

        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        let sport_ids = self.fetch_available_sports().await.unwrap_or_else(|error| {
            warn!(error = %error, "Zenit left menu fetch failed, falling back to core sports");
            vec![
                ZenitSport {
                    id: Self::SPORT_FOOTBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_HOCKEY,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_BASKETBALL,
                    count: 0,
                },
                ZenitSport {
                    id: Self::SPORT_TENNIS,
                    count: 0,
                },
            ]
        });

        for sport in sport_ids {
            // Prematch
            if let Ok((events, odds)) = self
                .fetch_sport("https://zenit.win/ajax/line/printer/react", sport.id, false)
                .await
            {
                all_events.extend(events);
                all_odds.extend(odds);
            }
        }

        if let Ok((events, odds)) = self.fetch_live().await {
            all_events.extend(events);
            all_odds.extend(odds);
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Zenit fetch complete"
        );
        Ok(ParserResult::new("zenit", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://zenit.win"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    }
}
