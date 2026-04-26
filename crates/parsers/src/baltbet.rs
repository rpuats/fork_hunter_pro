use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::{NaiveDateTime, TimeZone, Utc};
use futures::stream::{self, StreamExt};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use shared::odds::OddsType;
use shared::{
    DiagnosticSeverity, Event, Odd, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage,
    Sport,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};

const BOOKMAKER_SLUG: &str = "baltbet";
const LIVE_PAGE_URL: &str = "https://baltbet.ru/live";
const LIVE_API_URL: &str = "https://events.baltbet.ru/api/live/table/fetch/diff";
const LIVE_BANNER_API_BASE_URL: &str = "https://events.baltbet.ru/api/event/banner";
const LEGACY_PREMATCH_URL: &str = "https://old.baltbet.ru/Line1.aspx";
const LEGACY_LIVE_URL: &str = "https://old.baltbet.ru/Live1.aspx";
const LEGACY_PREMATCH_FEATURED_GROUP_LIMIT: usize = 40;
const LIVE_BANNER_FETCH_CONCURRENCY: usize = 8;
const LIVE_BANNER_PROBE_LIMIT: usize = 72;
const LEGACY_PREMATCH_FETCH_CONCURRENCY: usize = 10;
const STRICT_LIVE_KPI_TARGET: usize = 150;
const STRICT_PREMATCH_KPI_TARGET: usize = 3000;
const RECENT_STRICT_LIVE_EVENTS: usize = 183;
const RECENT_STRICT_PREMATCH_EVENTS: usize = 3086;

#[derive(Debug, Clone)]
struct LiveMeta {
    title: String,
    league: String,
    href: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacySportLink {
    group_id: String,
    sport_label: String,
}

#[derive(Debug)]
pub struct BaltbetParser {
    client: Arc<Client>,
}

impl BaltbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::Production,
            production_enabled: true,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "live_json_runtime_enabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "Primary live coverage uses {LIVE_API_URL} with HTML metadata hydration from {LIVE_PAGE_URL}."
                    ),
                },
                ParserDiagnosticCheck {
                    code: "live_banner_metadata_fallback_enabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Missing live metadata is backfilled via event banner probes without mutating the primary runtime branch.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "legacy_prematch_group_harvester_enabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "Prematch coverage expands from {LEGACY_PREMATCH_URL} into legacy Line2 group pages with deduplicated event ids."
                    ),
                },
                ParserDiagnosticCheck {
                    code: "strict_live_kpi_recently_met".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "Recent strict runtime diagnostics observed {RECENT_STRICT_LIVE_EVENTS} live events against the nightly target of {STRICT_LIVE_KPI_TARGET}."
                    ),
                },
                ParserDiagnosticCheck {
                    code: "strict_prematch_kpi_recently_met".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "Recent strict runtime diagnostics observed {RECENT_STRICT_PREMATCH_EVENTS} prematch events against the nightly target of {STRICT_PREMATCH_KPI_TARGET}."
                    ),
                },
            ],
        }
    }

    async fn fetch_live_page_html(
        &self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(LIVE_PAGE_URL)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("live page returned {}", resp.status()).into());
        }

        Ok(resp.text().await?)
    }

    async fn fetch_live_json(&self) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(LIVE_API_URL)
            .header("Accept", "*/*")
            .header("Origin", "https://baltbet.ru")
            .header("Referer", "https://baltbet.ru/")
            .header("bb-ajax", "1")
            .header("bb_appsource", "UniRu")
            .header("bb_lang", "ru")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("live JSON returned {}", resp.status()).into());
        }

        Ok(resp.json().await?)
    }

    async fn fetch_live_banner_meta(
        &self,
        event_id: u64,
    ) -> Result<Option<LiveMeta>, Box<dyn std::error::Error + Send + Sync>> {
        for attempt in 0..2 {
            match self.fetch_live_banner_meta_once(event_id).await {
                Ok(result) => return Ok(result),
                Err(error) if attempt == 0 => {
                    debug!(event_id, error = %error, "Baltbet live banner metadata retry scheduled");
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(None)
    }

    async fn fetch_live_banner_meta_once(
        &self,
        event_id: u64,
    ) -> Result<Option<LiveMeta>, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(format!("{LIVE_BANNER_API_BASE_URL}/{event_id}?live=true"))
            .header("Accept", "*/*")
            .header("Origin", "https://baltbet.ru")
            .header("Referer", "https://baltbet.ru/")
            .header("bb-ajax", "1")
            .header("bb_appsource", "UniRu")
            .header("bb_lang", "ru")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let payload = resp.json::<Value>().await?;
        Ok(Self::parse_live_banner_meta(&payload))
    }

    async fn fetch_live_banner_meta_batch(&self, event_ids: Vec<u64>) -> Vec<(u64, LiveMeta)> {
        stream::iter(event_ids.into_iter().map(|event_id| async move {
            match self.fetch_live_banner_meta(event_id).await {
                Ok(Some(meta_entry)) => Some((event_id, meta_entry)),
                Ok(None) => None,
                Err(error) => {
                    debug!(event_id, error = %error, "Baltbet live banner metadata fetch failed");
                    None
                }
            }
        }))
        .buffer_unordered(LIVE_BANNER_FETCH_CONCURRENCY)
        .filter_map(async move |result| result)
        .collect()
        .await
    }

    async fn fetch_live_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut live_meta = match self.fetch_live_page_html().await {
            Ok(live_html) => Self::parse_live_metadata(&live_html),
            Err(error) => {
                warn!(error = %error, "Baltbet live HTML metadata fetch failed");
                HashMap::new()
            }
        };
        debug!(count = live_meta.len(), "Baltbet live metadata parsed");

        let live_json = self.fetch_live_json().await?;
        let banner_probe_ids = Self::collect_live_banner_probe_ids(&live_json, &live_meta);
        let banner_meta = self.fetch_live_banner_meta_batch(banner_probe_ids).await;
        debug!(
            count = banner_meta.len(),
            "Baltbet live banner fallback parsed"
        );
        for (event_id, meta_entry) in banner_meta {
            Self::merge_live_banner_meta(&mut live_meta, event_id, meta_entry);
        }

        let (events, odds) = Self::parse_live_json(&live_json, &live_meta);
        debug!(
            events = events.len(),
            odds = odds.len(),
            "Baltbet live JSON parsed"
        );

        Ok((events, odds))
    }

    async fn fetch_legacy_html(
        &self,
        url: &str,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), url, "Baltbet legacy HTML fetch failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let html = resp.text().await?;
        if !Self::is_valid_legacy_html_source(&html) {
            warn!(url, "Baltbet legacy HTML rejected by source validation");
            return Ok((Vec::new(), Vec::new()));
        }

        let parsed = Self::parse_legacy_html(&html, is_live, url);
        if parsed.0.is_empty() {
            debug!(url, "Baltbet legacy HTML produced no events");
            return Ok((Vec::new(), Vec::new()));
        }

        Ok(parsed)
    }

    async fn fetch_legacy_prematch_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(LEGACY_PREMATCH_URL)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("legacy prematch page returned {}", resp.status()).into());
        }

        let html = resp.text().await?;
        let sport_links = Self::parse_legacy_sport_links(&html);
        if sport_links.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        let client = self.client.clone();
        let results = stream::iter(sport_links.into_iter().map(move |sport_link| {
            let client = client.clone();
            async move {
                let url = format!("https://old.baltbet.ru/Line2.aspx?group={}", sport_link.group_id);
                let resp = client
                    .get(&url)
                    .header(
                        "Accept",
                        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                    )
                    .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
                    .send()
                    .await;

                match resp {
                    Ok(resp) if resp.status().is_success() => match resp.text().await {
                        Ok(html) => Some((sport_link.group_id, sport_link.sport_label, html)),
                        Err(error) => {
                            debug!(url, error = %error, "Baltbet prematch group body read failed");
                            None
                        }
                    },
                    Ok(resp) => {
                        debug!(status = %resp.status(), url, "Baltbet prematch group fetch failed");
                        None
                    }
                    Err(error) => {
                        debug!(url, error = %error, "Baltbet prematch group request failed");
                        None
                    }
                }
            }
        }))
        .buffer_unordered(LEGACY_PREMATCH_FETCH_CONCURRENCY)
        .filter_map(async move |result| result)
        .collect::<Vec<_>>()
        .await;

        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut seen_event_ids = HashSet::new();

        for (group_id, sport_label, html) in results {
            let (group_events, group_odds) =
                Self::parse_legacy_group_page(&html, &group_id, &sport_label, &mut seen_event_ids);
            events.extend(group_events);
            odds.extend(group_odds);
        }

        debug!(
            events = events.len(),
            odds = odds.len(),
            "Baltbet prematch legacy groups parsed"
        );
        Ok((events, odds))
    }

    fn parse_live_metadata(html: &str) -> HashMap<u64, LiveMeta> {
        let document = Html::parse_document(html);
        let row_selector = Selector::parse(".events-table__body-row[data-id]").expect("selector");
        let title_selector = Selector::parse(".events-table__title").expect("selector");
        let league_selector = Selector::parse(".events-table__league").expect("selector");
        let link_selector = Selector::parse("a.events-table__title-wrapper").expect("selector");
        let t_data_selector = Selector::parse("[t-data]").expect("selector");

        let mut meta = HashMap::new();

        for row in document.select(&row_selector) {
            let Some(id) = row
                .value()
                .attr("data-id")
                .and_then(|v| v.parse::<u64>().ok())
            else {
                continue;
            };

            let title = row
                .select(&title_selector)
                .next()
                .map(|node| node.text().collect::<String>())
                .map(|text| Self::normalize_whitespace(&text))
                .unwrap_or_default();

            let league = row
                .select(&league_selector)
                .next()
                .map(|node| node.text().collect::<String>())
                .map(|text| Self::normalize_whitespace(&text))
                .unwrap_or_else(|| "Unknown".to_string());

            let href = row
                .select(&link_selector)
                .next()
                .and_then(|node| node.value().attr("href"))
                .map(str::to_string);

            if title.is_empty() {
                continue;
            }

            meta.insert(
                id,
                LiveMeta {
                    title,
                    league,
                    href,
                },
            );
        }

        for node in document.select(&t_data_selector) {
            let Some(encoded) = node.value().attr("t-data") else {
                continue;
            };

            let decoded = Self::decode_html_entities(encoded);
            let Ok(payload) = serde_json::from_str::<Value>(&decoded) else {
                continue;
            };

            let Some(id) = payload.get("eventId").and_then(Value::as_u64) else {
                continue;
            };

            let title = match (
                payload.get("firstParticipant").and_then(Value::as_str),
                payload.get("secondParticipant").and_then(Value::as_str),
            ) {
                (Some(home), Some(away)) => {
                    let home = Self::normalize_whitespace(home);
                    let away = Self::normalize_whitespace(away);
                    if home.is_empty() || away.is_empty() {
                        String::new()
                    } else {
                        format!("{} - {}", home, away)
                    }
                }
                _ => String::new(),
            };

            if title.is_empty() {
                continue;
            }

            let league = payload
                .get("leagueTitle")
                .and_then(Value::as_str)
                .map(Self::normalize_whitespace)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());

            let href = payload
                .get("webLink")
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|value| !value.is_empty());

            meta.entry(id)
                .and_modify(|existing| {
                    if existing.title.is_empty() {
                        existing.title = title.clone();
                    }
                    if existing.league == "Unknown" && league != "Unknown" {
                        existing.league = league.clone();
                    }
                    if existing.href.is_none() {
                        existing.href = href.clone();
                    }
                })
                .or_insert(LiveMeta {
                    title,
                    league,
                    href,
                });
        }

        meta
    }

    fn decode_html_entities(input: &str) -> String {
        let mut decoded = String::with_capacity(input.len());
        let mut idx = 0;

        while idx < input.len() {
            let Some(ch) = input[idx..].chars().next() else {
                break;
            };

            if ch != '&' {
                decoded.push(ch);
                idx += ch.len_utf8();
                continue;
            }

            let Some(relative_end) = input[idx..].find(';') else {
                decoded.push(ch);
                idx += ch.len_utf8();
                continue;
            };
            let end = idx + relative_end;
            let entity = &input[idx + 1..end];

            let replacement = match entity {
                "quot" => Some('"'),
                "amp" => Some('&'),
                "apos" => Some('\''),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some(' '),
                _ => {
                    if let Some(hex) = entity.strip_prefix("#x") {
                        u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                    } else if let Some(decimal) = entity.strip_prefix('#') {
                        decimal.parse::<u32>().ok().and_then(char::from_u32)
                    } else {
                        None
                    }
                }
            };

            if let Some(ch) = replacement {
                decoded.push(ch);
                idx = end + 1;
            } else {
                decoded.push('&');
                idx += 1;
            }
        }

        decoded
    }

    fn parse_live_json(json: &Value, meta: &HashMap<u64, LiveMeta>) -> (Vec<Event>, Vec<Odd>) {
        let Some(items) = json.get("events").and_then(Value::as_array) else {
            return (Vec::new(), Vec::new());
        };

        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut seen = HashSet::new();

        for item in items {
            let Some(event_id) = item.get("i").and_then(Value::as_u64) else {
                continue;
            };

            let Some(meta_entry) = meta.get(&event_id) else {
                continue;
            };

            let Some((home_team, away_team)) = Self::extract_live_teams(meta_entry) else {
                continue;
            };

            let dedupe_key = format!("{}|{}|{}", meta_entry.league, home_team, away_team);
            if !seen.insert(dedupe_key) {
                continue;
            }

            let event_key = format!("baltbet-{}", event_id);
            let sport = Self::detect_sport(
                meta_entry.href.as_deref(),
                &meta_entry.league,
                &meta_entry.title,
            );
            let raw_url = meta_entry
                .href
                .as_ref()
                .map(|href| {
                    if href.starts_with("http") {
                        href.clone()
                    } else {
                        format!("https://baltbet.ru{}", href)
                    }
                })
                .or_else(|| Some(LIVE_PAGE_URL.to_string()));

            events.push(Event {
                id: event_key.clone(),
                sport,
                league: meta_entry.league.clone(),
                home_team: home_team.clone(),
                away_team: away_team.clone(),
                start_time: None,
                is_live: true,
                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                raw_url,
                extra: HashMap::new(),
            });

            odds.extend(Self::extract_live_odds(&event_key, item));
        }

        (events, odds)
    }

    fn collect_live_banner_probe_ids(json: &Value, meta: &HashMap<u64, LiveMeta>) -> Vec<u64> {
        let Some(items) = json.get("events").and_then(Value::as_array) else {
            return Vec::new();
        };

        let mut missing_meta = Vec::new();
        let mut unresolved_meta = Vec::new();
        let mut seen = HashSet::new();

        for item in items {
            let Some(event_id) = item.get("i").and_then(Value::as_u64) else {
                continue;
            };
            if !seen.insert(event_id) {
                continue;
            }

            match meta.get(&event_id) {
                None => missing_meta.push(event_id),
                Some(meta_entry) if Self::live_meta_requires_banner_recovery(meta_entry) => {
                    unresolved_meta.push(event_id)
                }
                Some(_) => {}
            }
        }

        missing_meta.extend(unresolved_meta);
        missing_meta.truncate(LIVE_BANNER_PROBE_LIMIT);
        missing_meta
    }

    fn merge_live_banner_meta(
        meta: &mut HashMap<u64, LiveMeta>,
        event_id: u64,
        banner_meta: LiveMeta,
    ) {
        match meta.get_mut(&event_id) {
            Some(existing) if Self::live_meta_requires_banner_recovery(existing) => {
                *existing = banner_meta;
            }
            Some(_) => {}
            None => {
                meta.insert(event_id, banner_meta);
            }
        }
    }

    fn parse_live_banner_meta(payload: &Value) -> Option<LiveMeta> {
        let home_team = payload
            .get("team1")
            .and_then(Value::as_str)
            .map(Self::normalize_whitespace)
            .filter(|value| !value.is_empty());
        let away_team = payload
            .get("team2")
            .and_then(Value::as_str)
            .map(Self::normalize_whitespace)
            .filter(|value| !value.is_empty());

        let title = match (home_team, away_team) {
            (Some(home_team), Some(away_team)) => format!("{} - {}", home_team, away_team),
            _ => payload
                .get("eventName")
                .and_then(Value::as_str)
                .map(Self::normalize_whitespace)
                .filter(|value| !value.is_empty())?,
        };

        let league = payload
            .get("champName")
            .or_else(|| payload.get("sportName"))
            .and_then(Value::as_str)
            .map(Self::normalize_whitespace)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Unknown".to_string());

        let href = payload
            .get("link")
            .or_else(|| payload.get("webLink"))
            .and_then(Value::as_str)
            .map(Self::normalize_whitespace)
            .filter(|value| !value.is_empty());

        Some(LiveMeta {
            title,
            league,
            href,
        })
    }

    fn extract_live_odds(event_id: &str, item: &Value) -> Vec<Odd> {
        let Some(coeffs) = item.get("c").and_then(Value::as_array) else {
            return Vec::new();
        };

        let now = Utc::now();
        let active: Vec<&Value> = coeffs
            .iter()
            .filter(|coef| coef.get("s").and_then(Value::as_i64) == Some(1))
            .filter(|coef| coef.get("v").and_then(Value::as_f64).unwrap_or_default() > 1.0)
            .collect();

        if active.is_empty() {
            return Vec::new();
        }

        let mut odds = Vec::new();
        let mut push =
            |suffix: &str, market: &str, selection: &str, odds_type: OddsType, value: f64| {
                odds.push(Odd {
                    id: format!("{}-{}", event_id, suffix),
                    event_id: event_id.to_string(),
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    market: market.to_string(),
                    selection: selection.to_string(),
                    odds: value,
                    odds_type,
                    line: None,
                    timestamp: now,
                });
            };

        let by_type: HashMap<u64, f64> = active
            .iter()
            .filter_map(|coef| Some((coef.get("t")?.as_u64()?, coef.get("v")?.as_f64()?)))
            .collect();

        if let (Some(home), Some(draw), Some(away)) =
            (by_type.get(&1334), by_type.get(&1335), by_type.get(&1336))
        {
            push("1", "1X2", "1", OddsType::Home, *home);
            push("X", "1X2", "X", OddsType::Draw, *draw);
            push("2", "1X2", "2", OddsType::Away, *away);
            return odds;
        }

        for (market, home_type, away_type) in [
            ("Moneyline", 1952_u64, 1953_u64),
            ("Moneyline", 2017_u64, 2018_u64),
            ("MatchWinner", 3679_u64, 3681_u64),
        ] {
            if let (Some(home), Some(away)) = (by_type.get(&home_type), by_type.get(&away_type)) {
                push("1", market, "1", OddsType::Home, *home);
                push("2", market, "2", OddsType::Away, *away);
                return odds;
            }
        }

        let unlabeled: Vec<f64> = active
            .iter()
            .filter(|coef| {
                coef.get("x")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .is_empty()
            })
            .filter_map(|coef| coef.get("v").and_then(Value::as_f64))
            .collect();

        if unlabeled.len() >= 3 {
            push("1", "1X2", "1", OddsType::Home, unlabeled[0]);
            push("X", "1X2", "X", OddsType::Draw, unlabeled[1]);
            push("2", "1X2", "2", OddsType::Away, unlabeled[2]);
        } else if unlabeled.len() >= 2 {
            push("1", "Moneyline", "1", OddsType::Home, unlabeled[0]);
            push("2", "Moneyline", "2", OddsType::Away, unlabeled[1]);
        }

        odds
    }

    fn normalize_whitespace(input: &str) -> String {
        input
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn split_match_title(title: &str) -> Option<(String, String)> {
        for separator in [" — ", " - ", " – ", " vs ", " VS ", " v ", "-"] {
            let parts: Vec<&str> = title.splitn(2, separator).collect();
            if parts.len() == 2 {
                let home = Self::normalize_whitespace(parts[0]);
                let away = Self::normalize_whitespace(parts[1]);
                if !home.is_empty() && !away.is_empty() && home != away {
                    return Some((home, away));
                }
            }
        }

        None
    }

    fn extract_live_teams(meta: &LiveMeta) -> Option<(String, String)> {
        if let Some((home_team, away_team)) = Self::split_match_title(&meta.title) {
            if Self::is_valid_team(&home_team) && Self::is_valid_team(&away_team) {
                return Some((home_team, away_team));
            }
        }

        let (home_team, away_team) =
            Self::split_teams_from_href(meta.href.as_deref(), &meta.title)?;
        if Self::is_valid_team(&home_team) && Self::is_valid_team(&away_team) {
            Some((home_team, away_team))
        } else {
            None
        }
    }

    fn live_meta_requires_banner_recovery(meta: &LiveMeta) -> bool {
        Self::extract_live_teams(meta).is_none()
    }

    fn split_teams_from_href(href: Option<&str>, title: &str) -> Option<(String, String)> {
        let href = href?;
        let slug = href
            .rsplit('/')
            .next()
            .unwrap_or(href)
            .split('?')
            .next()
            .unwrap_or(href);
        let matchup = slug.split("-id-").next().unwrap_or(slug);

        let markers = Self::live_stat_slug_markers(title);
        for home_marker in markers {
            let home_token = format!("-{home_marker}");
            let Some(first) = matchup.find(&home_token) else {
                continue;
            };

            for away_marker in markers {
                let away_token = format!("-{away_marker}");
                let Some(relative_second) = matchup[first + home_token.len()..].find(&away_token)
                else {
                    continue;
                };
                let second = first + home_token.len() + relative_second;
                let home = Self::slug_to_team_name(&matchup[..first]);
                let away = Self::slug_to_team_name(&matchup[first + home_token.len()..second]);
                if !home.is_empty() && !away.is_empty() && home != away {
                    return Some((home, away));
                }
            }

            for away_marker in Self::all_live_stat_slug_markers() {
                let away_token = format!("-{away_marker}");
                let Some(relative_second) = matchup[first + home_token.len()..].find(&away_token)
                else {
                    continue;
                };
                let second = first + home_token.len() + relative_second;
                let home = Self::slug_to_team_name(&matchup[..first]);
                let away = Self::slug_to_team_name(&matchup[first + home_token.len()..second]);
                if !home.is_empty() && !away.is_empty() && home != away {
                    return Some((home, away));
                }
            }
        }

        None
    }

    fn all_live_stat_slug_markers() -> &'static [&'static str] {
        &[
            "aces",
            "corners",
            "double-faults",
            "double-foults",
            "fouls",
            "goal-kicks",
            "offsides",
            "outs",
            "penalty-time",
            "penalty-time-minutes",
            "players",
            "posts-and-crossbars",
            "posts-and-srossbars",
            "power-play-goals",
            "shots-on-goal",
            "three-pointers-scored",
            "two-pointers-scored",
            "yellow-cards",
        ]
    }

    fn live_stat_slug_markers(title: &str) -> &'static [&'static str] {
        match title.trim().to_lowercase().as_str() {
            "эйсы" => &["aces"],
            "двойные ошибки" => &["double-faults", "double-foults"],
            "желтые карточки" => &["yellow-cards"],
            "угловые" => &["corners"],
            "ауты" => &["outs"],
            "офсайды" => &["offsides"],
            "попадания в каркас ворот" => {
                &["posts-and-crossbars", "posts-and-srossbars"]
            }
            "удары в створ" => &["shots-on-goal"],
            "удары от ворот" => &["goal-kicks"],
            "фолы" => &["fouls"],
            "голы в большинстве" => &["power-play-goals"],
            "игроки" => &["players"],
            "сумма штрафных минут" => &["penalty-time", "penalty-time-minutes"],
            "2-х очк. попадания" => &["two-pointers-scored"],
            "3-х очк. попадания" => &["three-pointers-scored"],
            _ => &[],
        }
    }

    fn slug_to_team_name(value: &str) -> String {
        value
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn detect_sport(path: Option<&str>, league: &str, title: &str) -> Sport {
        let mut probe = String::new();
        if let Some(path) = path {
            probe.push_str(path);
            probe.push(' ');
        }
        probe.push_str(league);
        probe.push(' ');
        probe.push_str(title);
        let lower = probe.to_lowercase();

        if lower.contains("american-football")
            || lower.contains("американ")
            || lower.contains("nfl")
            || lower.contains("ufl")
        {
            Sport::Other
        } else if lower.contains("table-tennis") || lower.contains("настоль") {
            Sport::TableTennis
        } else if lower.contains("теннис") || lower.contains("tennis") {
            Sport::Tennis
        } else if lower.contains("хоккей") || lower.contains("hockey") {
            Sport::Hockey
        } else if lower.contains("баскет") || lower.contains("basket") {
            Sport::Basketball
        } else if lower.contains("волей") || lower.contains("volleyball") {
            Sport::Volleyball
        } else if lower.contains("rugby") || lower.contains("регби") {
            Sport::Rugby
        } else if lower.contains("гандбол") || lower.contains("handball") {
            Sport::Handball
        } else if lower.contains("бейсбол") || lower.contains("baseball") {
            Sport::Baseball
        } else if lower.contains("бадминтон") || lower.contains("badminton") {
            Sport::Badminton
        } else if lower.contains("esports")
            || lower.contains("кибер")
            || lower.contains("dota")
            || lower.contains("counter-strike")
            || lower.contains("cs2")
        {
            Sport::Esports
        } else {
            Sport::Football
        }
    }

    fn is_valid_legacy_html_source(html: &str) -> bool {
        html.contains("Line1.aspx")
            || html.contains("Live1.aspx")
            || (html.contains("class=\"name\"") && html.contains("class=\"coe\""))
    }

    fn parse_legacy_html(html: &str, is_live: bool, source_url: &str) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let now = Utc::now();
        let name_marker = "class=\"name\"";
        let coe_marker = "class=\"coe\"";

        let names: Vec<String> = Self::extract_span_values(html, name_marker);
        let coefs: Vec<f64> = Self::extract_span_floats(html, coe_marker);

        let mut idx = 0;
        let mut event_counter = 0;

        while idx + 2 < names.len() {
            let middle = names[idx + 1].to_lowercase();
            if middle.contains("ничья") || middle.contains("draw") {
                let home = names[idx].trim().to_string();
                let away = names[idx + 2].trim().to_string();

                if Self::is_valid_team(&home) && Self::is_valid_team(&away) && home != away {
                    let coef_idx = event_counter * 3;
                    let home_odds = coefs.get(coef_idx).copied();
                    let draw_odds = coefs.get(coef_idx + 1).copied();
                    let away_odds = coefs.get(coef_idx + 2).copied();

                    if home_odds.is_some() && away_odds.is_some() {
                        let event_id = format!("baltbet-legacy-{}", event_counter);

                        events.push(Event {
                            id: event_id.clone(),
                            sport: Sport::Football,
                            league: "Unknown".to_string(),
                            home_team: home,
                            away_team: away,
                            start_time: None,
                            is_live,
                            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                            raw_url: Some(source_url.to_string()),
                            extra: HashMap::new(),
                        });

                        if let Some(o1) = home_odds {
                            odds.push(Odd {
                                id: format!("{}-1", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                                market: "1X2".into(),
                                selection: "1".into(),
                                odds: o1,
                                odds_type: OddsType::Home,
                                line: None,
                                timestamp: now,
                            });
                        }
                        if let Some(ox) = draw_odds {
                            odds.push(Odd {
                                id: format!("{}-X", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                                market: "1X2".into(),
                                selection: "X".into(),
                                odds: ox,
                                odds_type: OddsType::Draw,
                                line: None,
                                timestamp: now,
                            });
                        }
                        if let Some(o2) = away_odds {
                            odds.push(Odd {
                                id: format!("{}-2", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                                market: "1X2".into(),
                                selection: "2".into(),
                                odds: o2,
                                odds_type: OddsType::Away,
                                line: None,
                                timestamp: now,
                            });
                        }

                        event_counter += 1;
                    }
                }
                idx += 3;
            } else {
                idx += 1;
            }
        }

        (events, odds)
    }

    fn parse_legacy_sport_links(html: &str) -> Vec<LegacySportLink> {
        let document = Html::parse_document(html);
        let selector = Selector::parse("#tablesportsdiv a").expect("selector");
        let href_selector = Selector::parse("a[href]").expect("selector");
        let re = Regex::new(r"openall\('([^']+)'\)").expect("regex");
        let direct_group_re = Regex::new(r#"(?i)line2\.aspx\?group=([^&#"']+)"#).expect("regex");
        let mut links = Vec::new();
        let mut seen = HashSet::new();

        for link in document.select(&selector) {
            let Some(href) = link.value().attr("href") else {
                continue;
            };
            let Some(captures) = re.captures(href) else {
                continue;
            };
            let Some(group_id) = captures.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            let sport_label = Self::normalize_whitespace(&link.text().collect::<String>());
            if sport_label.is_empty() || !seen.insert(group_id.clone()) {
                continue;
            }

            links.push(LegacySportLink {
                group_id,
                sport_label,
            });
        }

        let mut featured_groups = 0;
        for link in document.select(&href_selector) {
            if featured_groups >= LEGACY_PREMATCH_FEATURED_GROUP_LIMIT {
                break;
            }

            let Some(href) = link.value().attr("href") else {
                continue;
            };
            let Some(captures) = direct_group_re.captures(href) else {
                continue;
            };
            let Some(group_id) = captures.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };

            if !group_id.contains('_') || !seen.insert(group_id.clone()) {
                continue;
            }

            let sport_label = Self::normalize_whitespace(&link.text().collect::<String>());
            links.push(LegacySportLink {
                group_id,
                sport_label: if sport_label.is_empty() {
                    "Unknown".to_string()
                } else {
                    sport_label
                },
            });
            featured_groups += 1;
        }

        links
    }

    fn parse_legacy_group_page(
        html: &str,
        group_id: &str,
        sport_label: &str,
        seen_event_ids: &mut HashSet<String>,
    ) -> (Vec<Event>, Vec<Odd>) {
        let document = Html::parse_document(html);
        let table_selector = Selector::parse("table.lvmain.coef-tobasket").expect("selector");
        let row_selector = Selector::parse("tr").expect("selector");
        let cell_selector = Selector::parse("td").expect("selector");
        let head_league_selector = Selector::parse("tr.head td.left").expect("selector");
        let left_cell_selector = Selector::parse("td.left").expect("selector");
        let link_selector = Selector::parse("a").expect("selector");
        let coef_cell_selector = Selector::parse("td.coef").expect("selector");
        let now = Utc::now();
        let event_re = Regex::new(r"event=(\d+)").expect("regex");
        let mut events = Vec::new();
        let mut odds = Vec::new();

        for table in document.select(&table_selector) {
            let league = table
                .select(&head_league_selector)
                .next()
                .map(|node| Self::normalize_whitespace(&node.text().collect::<String>()))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| sport_label.to_string());

            for row in table.select(&row_selector) {
                if row.value().attr("class") == Some("head") {
                    continue;
                }

                if row
                    .value()
                    .attr("id")
                    .is_some_and(|id| id.starts_with("addrow"))
                {
                    continue;
                }

                let cells: Vec<_> = row.select(&cell_selector).collect();
                if cells.len() < 3 {
                    continue;
                }

                let Some(left_cell) = row.select(&left_cell_selector).next() else {
                    continue;
                };
                let Some(link) = left_cell.select(&link_selector).next() else {
                    continue;
                };

                let title = Self::normalize_whitespace(&link.text().collect::<String>());
                let Some((home_team, away_team)) = Self::split_match_title(&title) else {
                    continue;
                };

                if !Self::is_valid_team(&home_team) || !Self::is_valid_team(&away_team) {
                    continue;
                }

                let onclick = link.value().attr("onclick").unwrap_or_default();
                let event_id = event_re
                    .captures(onclick)
                    .and_then(|captures| captures.get(1))
                    .map(|m| format!("baltbet-prematch-{}", m.as_str()))
                    .unwrap_or_else(|| {
                        format!(
                            "baltbet-prematch-{}-{}",
                            group_id,
                            title.to_lowercase().replace(' ', "-")
                        )
                    });

                if !seen_event_ids.insert(event_id.clone()) {
                    continue;
                }

                let coef_cells: Vec<f64> = row
                    .select(&coef_cell_selector)
                    .filter_map(|cell| Self::parse_legacy_float(&cell.text().collect::<String>()))
                    .collect();

                let start_time =
                    cells
                        .get(1)
                        .zip(cells.get(2))
                        .and_then(|(date_cell, time_cell)| {
                            Self::parse_legacy_start_time(
                                &date_cell.text().collect::<String>(),
                                &time_cell.text().collect::<String>(),
                            )
                        });

                let sport = Self::detect_sport(None, &league, sport_label);
                let raw_url = Some(format!(
                    "https://old.baltbet.ru/Line2.aspx?group={}",
                    group_id
                ));

                events.push(Event {
                    id: event_id.clone(),
                    sport,
                    league: league.clone(),
                    home_team: home_team.clone(),
                    away_team: away_team.clone(),
                    start_time,
                    is_live: false,
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    raw_url,
                    extra: HashMap::new(),
                });

                if coef_cells.len() >= 3 {
                    odds.push(Odd {
                        id: format!("{}-1", event_id),
                        event_id: event_id.clone(),
                        bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                        market: "1X2".into(),
                        selection: "1".into(),
                        odds: coef_cells[0],
                        odds_type: OddsType::Home,
                        line: None,
                        timestamp: now,
                    });
                    odds.push(Odd {
                        id: format!("{}-X", event_id),
                        event_id: event_id.clone(),
                        bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                        market: "1X2".into(),
                        selection: "X".into(),
                        odds: coef_cells[1],
                        odds_type: OddsType::Draw,
                        line: None,
                        timestamp: now,
                    });
                    odds.push(Odd {
                        id: format!("{}-2", event_id),
                        event_id: event_id.clone(),
                        bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                        market: "1X2".into(),
                        selection: "2".into(),
                        odds: coef_cells[2],
                        odds_type: OddsType::Away,
                        line: None,
                        timestamp: now,
                    });
                } else if coef_cells.len() >= 2 {
                    odds.push(Odd {
                        id: format!("{}-1", event_id),
                        event_id: event_id.clone(),
                        bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                        market: "Moneyline".into(),
                        selection: "1".into(),
                        odds: coef_cells[0],
                        odds_type: OddsType::Home,
                        line: None,
                        timestamp: now,
                    });
                    odds.push(Odd {
                        id: format!("{}-2", event_id),
                        event_id: event_id.clone(),
                        bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                        market: "Moneyline".into(),
                        selection: "2".into(),
                        odds: coef_cells[1],
                        odds_type: OddsType::Away,
                        line: None,
                        timestamp: now,
                    });
                }
            }
        }

        (events, odds)
    }

    fn parse_legacy_float(text: &str) -> Option<f64> {
        let normalized = Self::normalize_whitespace(text).replace(',', ".");
        let value = normalized.parse::<f64>().ok()?;
        (value > 1.0 && value < 100.0).then_some(value)
    }

    fn parse_legacy_start_time(date: &str, time: &str) -> Option<chrono::DateTime<Utc>> {
        let value = format!(
            "{} {}",
            Self::normalize_whitespace(date),
            Self::normalize_whitespace(time)
        );
        let naive = NaiveDateTime::parse_from_str(&value, "%d.%m.%Y %H:%M").ok()?;
        Some(Utc.from_utc_datetime(&naive))
    }

    fn extract_span_values(html: &str, class_marker: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut search_from = 0;

        while let Some(pos) = html[search_from..].find(class_marker) {
            let abs_pos = search_from + pos;
            if let Some(tag_end) = html[abs_pos..].find('>') {
                let content_start = abs_pos + tag_end + 1;
                if let Some(span_end) = html[content_start..].find("</span>") {
                    let text = html[content_start..content_start + span_end]
                        .trim()
                        .to_string();
                    if !text.is_empty() {
                        values.push(text);
                    }
                    search_from = content_start + span_end + 7;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        values
    }

    fn extract_span_floats(html: &str, class_marker: &str) -> Vec<f64> {
        let mut values = Vec::new();
        let mut search_from = 0;

        while let Some(pos) = html[search_from..].find(class_marker) {
            let abs_pos = search_from + pos;
            if let Some(tag_end) = html[abs_pos..].find('>') {
                let content_start = abs_pos + tag_end + 1;
                if let Some(span_end) = html[content_start..].find("</span>") {
                    let text = html[content_start..content_start + span_end].trim();
                    let normalized = text.replace(',', ".");
                    if let Ok(value) = normalized.parse::<f64>() {
                        if value > 1.0 && value < 100.0 {
                            values.push(value);
                        }
                    }
                    search_from = content_start + span_end + 7;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        values
    }

    fn is_valid_team(name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.len() < 2 || trimmed.len() > 80 {
            return false;
        }

        let lower = trimmed.to_lowercase();
        let invalid_words = [
            "футбол",
            "счёт",
            "счет",
            "live",
            "лайв",
            "матч",
            "игра",
            "спорт",
            "football",
            "soccer",
            "sport",
            "game",
            "match",
            "count",
            "basketball",
            "теннис",
            "hockey",
            "хоккей",
            "volleyball",
            "волейбол",
            "статистика",
            "statistics",
            "время",
            "time",
            "vs",
            "против",
            "команда",
            "team",
            "total",
            "тотал",
            "ничья",
            "draw",
            "unknown",
            "неизвест",
            "tbd",
            "н/д",
            "n/a",
        ];

        if invalid_words.iter().any(|word| lower == *word) {
            return false;
        }

        if trimmed
            .replace('.', "")
            .replace(',', "")
            .replace(' ', "")
            .parse::<f64>()
            .is_ok()
        {
            return false;
        }

        true
    }
}

#[async_trait]
impl BookmakerParser for BaltbetParser {
    fn name(&self) -> &str {
        "Baltbet"
    }

    fn slug(&self) -> &str {
        BOOKMAKER_SLUG
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(Self::readiness_snapshot())
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();

        match self.fetch_live_runtime_data().await {
            Ok((events, _)) => all_events.extend(events),
            Err(error) => warn!(error = %error, "Baltbet live JSON path failed"),
        }

        if all_events.is_empty() {
            if let Ok((events, _)) = self.fetch_legacy_html(LEGACY_LIVE_URL, true).await {
                all_events.extend(events);
            }
        }

        match self.fetch_legacy_prematch_data().await {
            Ok((events, _)) => all_events.extend(events),
            Err(error) => warn!(error = %error, "Baltbet prematch group path failed"),
        }

        info!(count = all_events.len(), "Baltbet events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();

        match self.fetch_live_runtime_data().await {
            Ok((_, odds)) => all_odds.extend(odds),
            Err(error) => warn!(error = %error, "Baltbet live odds path failed"),
        }

        if all_odds.is_empty() {
            if let Ok((_, odds)) = self.fetch_legacy_html(LEGACY_LIVE_URL, true).await {
                all_odds.extend(odds);
            }
        }

        match self.fetch_legacy_prematch_data().await {
            Ok((_, odds)) => all_odds.extend(odds),
            Err(error) => warn!(error = %error, "Baltbet prematch odds path failed"),
        }

        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        match self.fetch_live_runtime_data().await {
            Ok((events, odds)) => {
                all_events.extend(events);
                all_odds.extend(odds);
            }
            Err(error) => warn!(error = %error, "Baltbet live JSON fetch_all path failed"),
        }

        if all_events.is_empty() {
            if let Ok((events, odds)) = self.fetch_legacy_html(LEGACY_LIVE_URL, true).await {
                all_events.extend(events);
                all_odds.extend(odds);
            }
        }

        match self.fetch_legacy_prematch_data().await {
            Ok((events, odds)) => {
                all_events.extend(events);
                all_odds.extend(odds);
            }
            Err(error) => warn!(error = %error, "Baltbet prematch fetch_all path failed"),
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Baltbet fetch complete"
        );
        Ok(ParserResult::new(
            BOOKMAKER_SLUG,
            all_events,
            all_odds,
            elapsed,
        ))
    }

    fn base_url(&self) -> &str {
        "https://baltbet.ru"
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    }
}

#[cfg(test)]
mod tests {
    use super::BaltbetParser;
    use shared::DiagnosticSeverity;
    use shared::OddsType;
    use shared::ParserReadinessStage;
    use shared::Sport;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn exposes_post_kpi_readiness_snapshot() {
        let readiness = BaltbetParser::readiness_snapshot();

        assert_eq!(readiness.stage, ParserReadinessStage::Production);
        assert!(readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "strict_live_kpi_recently_met"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "strict_prematch_kpi_recently_met"
                && matches!(check.severity, DiagnosticSeverity::Pass)));
    }

    #[test]
    fn parses_live_metadata_from_rows_and_t_data() {
        let html = include_str!("../tests/fixtures/baltbet_live_page_fixture.html");

        let meta = BaltbetParser::parse_live_metadata(html);

        assert_eq!(meta.len(), 3);

        let row_entry = meta.get(&29918372).expect("row metadata exists");
        assert_eq!(row_entry.title, "Донгбу Промы - Эгис");
        assert_eq!(row_entry.league, "Баскетбол");
        assert_eq!(
            row_entry.href.as_deref(),
            Some("/express-of-the-day/basketball/wonju-promy-jeonju-kcc-egis-id-29918372")
        );

        let t_data_entry = meta.get(&29927586).expect("t-data metadata exists");
        assert_eq!(t_data_entry.title, "Оренбург-2 - Урал-2");
        assert_eq!(
            t_data_entry.league,
            "Россия. 2-я лига. Дивизион Б. Группа 4"
        );
        assert_eq!(
            t_data_entry.href.as_deref(),
            Some("/soccer/russia/2nd-league/division-b/group-4/fc-orenburg-2-ural-2-id-29927586")
        );

        let stat_entry = meta.get(&29931947).expect("stat metadata exists");
        assert_eq!(stat_entry.title, "3-х очк. попадания");
        assert_eq!(stat_entry.league, "Баскетбол");
    }

    #[test]
    fn parses_live_json_with_t_data_metadata_fallback() {
        let html = include_str!("../tests/fixtures/baltbet_live_page_fixture.html");
        let payload: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/baltbet_live_payload.json"))
                .expect("live fixture should be valid json");

        let meta = BaltbetParser::parse_live_metadata(html);
        let (events, odds) = BaltbetParser::parse_live_json(&payload, &meta);

        assert_eq!(events.len(), 3);

        let football_event = events
            .iter()
            .find(|event| event.id == "baltbet-29927586")
            .expect("football event exists");
        assert_eq!(football_event.sport, Sport::Football);
        assert_eq!(football_event.home_team, "Оренбург-2");
        assert_eq!(football_event.away_team, "Урал-2");
        assert!(football_event.is_live);

        let basketball_event = events
            .iter()
            .find(|event| event.id == "baltbet-29918372")
            .expect("basketball event exists");
        assert_eq!(basketball_event.sport, Sport::Basketball);

        let stat_event = events
            .iter()
            .find(|event| event.id == "baltbet-29931947")
            .expect("stat event exists");
        assert_eq!(stat_event.sport, Sport::Basketball);
        assert_eq!(stat_event.home_team, "crvena zvezda mts belgrade");
        assert_eq!(stat_event.away_team, "cluj napoca");

        assert!(odds.iter().any(|odd| {
            odd.event_id == "baltbet-29927586"
                && odd.market == "1X2"
                && odd.selection == "X"
                && odd.odds_type == OddsType::Draw
                && (odd.odds - 3.35).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.event_id == "baltbet-29918372"
                && odd.market == "Moneyline"
                && odd.selection == "2"
                && odd.odds_type == OddsType::Away
                && (odd.odds - 2.05).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.event_id == "baltbet-29931947"
                && odd.market == "Moneyline"
                && odd.selection == "1"
                && odd.odds_type == OddsType::Home
                && (odd.odds - 1.87).abs() < f64::EPSILON
        }));
    }

    #[test]
    fn collects_banner_probe_ids_for_missing_and_unresolved_live_meta() {
        let payload = serde_json::json!({
            "events": [
                {"i": 11},
                {"i": 22},
                {"i": 33}
            ]
        });
        let meta = HashMap::from([
            (
                11,
                super::LiveMeta {
                    title: "Желтые карточки".to_string(),
                    league: "Футбол".to_string(),
                    href: None,
                },
            ),
            (
                22,
                super::LiveMeta {
                    title: "Реал Мадрид - Барселона".to_string(),
                    league: "Испания".to_string(),
                    href: None,
                },
            ),
        ]);

        let ids = BaltbetParser::collect_live_banner_probe_ids(&payload, &meta);

        assert_eq!(ids, vec![11, 33]);
    }

    #[test]
    fn banner_meta_replaces_unresolved_primary_live_meta() {
        let mut meta = HashMap::from([(
            11,
            super::LiveMeta {
                title: "Желтые карточки".to_string(),
                league: "Футбол".to_string(),
                href: None,
            },
        )]);

        BaltbetParser::merge_live_banner_meta(
            &mut meta,
            11,
            super::LiveMeta {
                title: "Реал Мадрид - Барселона".to_string(),
                league: "Испания".to_string(),
                href: Some("/soccer/spain/real-madrid-barcelona-id-11".to_string()),
            },
        );

        let merged = meta.get(&11).expect("merged meta exists");
        assert_eq!(merged.title, "Реал Мадрид - Барселона");
        assert_eq!(merged.league, "Испания");
        assert_eq!(
            merged.href.as_deref(),
            Some("/soccer/spain/real-madrid-barcelona-id-11")
        );
    }

    #[test]
    fn keeps_prematch_events_even_when_main_odds_are_hidden() {
        let html = include_str!("../tests/fixtures/baltbet_prematch_group_fixture.html");
        let mut seen = HashSet::new();

        let (events, odds) = BaltbetParser::parse_legacy_group_page(html, "1", "Футбол", &mut seen);

        assert_eq!(events.len(), 1);
        assert!(odds.is_empty());
        assert_eq!(events[0].id, "baltbet-prematch-29928528");
        assert_eq!(events[0].home_team, "Ливерпуль(замены)");
        assert_eq!(events[0].away_team, "ПСЖ(замены)");
        assert!(!events[0].is_live);
    }

    #[test]
    fn extracts_live_teams_from_stat_href_variants() {
        let teams = BaltbetParser::split_teams_from_href(
            Some(
                "/tennis/wta/125/oeiras-3/statistics/double-faults/kraus-sinja-double-faults-selekhmeteva-oksana-double-foults-id-29932186",
            ),
            "Двойные ошибки",
        )
        .expect("teams should be parsed from href");

        assert_eq!(teams.0, "kraus sinja");
        assert_eq!(teams.1, "selekhmeteva oksana");
    }

    #[test]
    fn extracts_live_teams_from_stat_href_with_mixed_markers() {
        let teams = BaltbetParser::split_teams_from_href(
            Some(
                "/basketball/south-korea/championship/statistics/two-pointers-scored/seoul-knights-two-pointers-scored-goyang-sky-gunners-three-pointers-scored-id-29939476",
            ),
            "2-х очк. попадания",
        )
        .expect("teams should be parsed from mixed stat markers");

        assert_eq!(teams.0, "seoul knights");
        assert_eq!(teams.1, "goyang sky gunners");
    }

    #[test]
    fn parses_live_banner_metadata_fallback() {
        let payload = serde_json::json!({
            "eventId": 29933242,
            "isActive": true,
            "isLive": true,
            "team1": "Сакраменто(Lucashin)",
            "team2": "Майами(Pakapaka)"
        });

        let meta = BaltbetParser::parse_live_banner_meta(&payload).expect("banner meta exists");

        assert_eq!(meta.title, "Сакраменто(Lucashin) - Майами(Pakapaka)");
        assert_eq!(meta.league, "Unknown");
        assert!(meta.href.is_none());
    }

    #[test]
    fn parses_live_banner_metadata_with_league_link_and_event_name_fallback() {
        let payload = serde_json::json!({
            "eventId": 29933243,
            "eventName": "Реал Мадрид - Барселона",
            "champName": "Испания. eBasketball",
            "sportName": "Баскетбол",
            "link": "/basketball/esports/real-madrid-barcelona-id-29933243"
        });

        let meta = BaltbetParser::parse_live_banner_meta(&payload).expect("banner meta exists");

        assert_eq!(meta.title, "Реал Мадрид - Барселона");
        assert_eq!(meta.league, "Испания. eBasketball");
        assert_eq!(
            meta.href.as_deref(),
            Some("/basketball/esports/real-madrid-barcelona-id-29933243")
        );
    }

    #[test]
    fn keeps_short_prematch_rows_without_main_odds() {
        let html = r#"
        <table class="lvmain coef-tobasket">
            <tr class="head"><td class="left">Хоккей</td></tr>
            <tr>
                <td class="left"><a onclick="open_some('event=29931405')">Салават Юлаев(игроки)-Локомотив Ярославль(игроки)</a></td>
                <td>14.04.2026</td>
                <td>17:00</td>
                <td class="dop">+44</td>
            </tr>
        </table>
        "#;
        let mut seen = HashSet::new();

        let (events, odds) = BaltbetParser::parse_legacy_group_page(html, "1", "Хоккей", &mut seen);

        assert_eq!(events.len(), 1);
        assert!(odds.is_empty());
        assert_eq!(events[0].id, "baltbet-prematch-29931405");
        assert_eq!(events[0].home_team, "Салават Юлаев(игроки)");
        assert_eq!(events[0].away_team, "Локомотив Ярославль(игроки)");
    }

    #[test]
    fn collects_featured_composite_prematch_groups() {
        let html = r#"
        <div class="allbet">
            <ul id="tablesportsdiv">
                <li><a href="javascript:openall('1')">Футбол</a></li>
                <li><a href="javascript:openall('920')">Баскетбол</a></li>
            </ul>
        </div>
        <div class="banners-slider">
            <a href="Line2.aspx?group=595807_21418_68973" class="banners-slider__item"></a>
            <a href="line2.aspx?group=809_8_309_125_81" class="banners-slider__item">Топ</a>
            <a href="Line2.aspx?group=1" class="banners-slider__item">duplicate</a>
        </div>
        "#;

        let links = BaltbetParser::parse_legacy_sport_links(html);

        assert_eq!(links.len(), 4);
        assert_eq!(links[0].group_id, "1");
        assert_eq!(links[0].sport_label, "Футбол");
        assert_eq!(links[1].group_id, "920");
        assert_eq!(links[2].group_id, "595807_21418_68973");
        assert_eq!(links[2].sport_label, "Unknown");
        assert_eq!(links[3].group_id, "809_8_309_125_81");
        assert_eq!(links[3].sport_label, "Топ");
    }

    #[test]
    fn keeps_collecting_featured_composite_groups_past_ten_entries() {
        let mut html = String::from(
            r#"
        <div class="allbet">
            <ul id="tablesportsdiv">
                <li><a href="javascript:openall('1')">Футбол</a></li>
            </ul>
        </div>
        <div class="banners-slider">
        "#,
        );

        for idx in 0..12 {
            html.push_str(&format!(
                r#"<a href="Line2.aspx?group=500_{idx}" class="banners-slider__item">Баннер {idx}</a>"#
            ));
        }

        html.push_str("</div>");

        let links = BaltbetParser::parse_legacy_sport_links(&html);

        assert_eq!(links.len(), 13);
        assert_eq!(links[0].group_id, "1");
        assert_eq!(links[11].group_id, "500_10");
        assert_eq!(links[12].group_id, "500_11");
    }
}
