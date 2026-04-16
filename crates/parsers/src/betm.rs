use crate::base::{BookmakerParser, ParserResult};
use crate::headless_helper::{is_valid_team_name, HeadlessChromeHelper};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{
    DiagnosticSeverity, Event, Odd, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage,
    Sport,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};

const BOOKMAKER_SLUG: &str = "betm";
const HEADLESS_WAIT_MS: u64 = 6_000;
const HEADLESS_RETRY_DELAY_MS: u64 = 1_000;
const HEADLESS_EVAL_ATTEMPTS: usize = 3;

#[derive(Clone, Copy, Debug)]
struct Probe {
    url: &'static str,
    is_live: bool,
}

const PROBES: &[Probe] = &[
    Probe {
        url: "https://bet-m.net/live",
        is_live: true,
    },
    Probe {
        url: "https://bet-m.net/line",
        is_live: false,
    },
    Probe {
        url: "https://betm.ru/live",
        is_live: true,
    },
    Probe {
        url: "https://betm.ru/line",
        is_live: false,
    },
];

const HEADLESS_EXTRACT_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    const parseOdds = (value) => {
        const normalized = normalizeText(value).replace(',', '.');
        if (!normalized) return null;
        const parsed = Number.parseFloat(normalized);
        return Number.isFinite(parsed) && parsed >= 1.01 && parsed <= 50 ? parsed : null;
    };
    const isName = (value) => {
        if (!value || value.length < 2 || value.length > 80) return false;
        if (/^(live|match|event|game|line)$/i.test(value)) return false;
        if (/^\d+[.,]?\d*$/.test(value)) return false;
        return true;
    };
    const seen = new Set();
    const results = [];
    const selectors = [
        '.event-item', '.match-item', '.game-item', '.sport-event', '.event-line',
        '[class*="event"]', '[class*="match"]', '[class*="game"]'
    ];
    const containers = Array.from(new Set(selectors.flatMap((selector) => Array.from(document.querySelectorAll(selector)))));

    for (const el of containers) {
        try {
            const text = normalizeText(el.innerText || el.textContent || '');
            if (!text || text.length < 15) continue;

            const odds = [];
            el.querySelectorAll('[class*="coef"], [class*="kef"], .coef, .kef, span').forEach((node) => {
                const value = parseOdds(node.textContent || '');
                if (value !== null) odds.push(value);
            });

            const lines = text.split(/\n+/).map(normalizeText).filter(Boolean);
            let home = '';
            let away = '';
            for (const line of lines) {
                if (!isName(line) || /live/i.test(line)) continue;
                if (!home) home = line;
                else if (line !== home && !away) away = line;
                if (home && away) break;
            }

            if (!home || !away || odds.length < 2) continue;

            const key = `${home}|${away}|${window.location.href}`;
            if (seen.has(key)) continue;
            seen.add(key);
            results.push({ home, away, odds: odds.slice(0, 3), sourceUrl: window.location.href });
        } catch (_) {}
    }

    return results;
})()"#;

#[derive(Debug)]
pub struct BetMParser {
    client: Arc<Client>,
}

impl BetMParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::DiagnosticOnly,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "legacy_dom_path_ported".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Legacy Playwright DOM extraction is ported to the Rust headless helper.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "legacy_and_current_domains_probed".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "The Rust path probes legacy bet-m.net routes and current betm.ru aliases for line/live pages.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "public_feed_not_confirmed".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "Current public routes resolve to 404/marketing shells or reset the connection, so scan enablement remains off.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "product_target_unverified".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "Live 150+ and prematch 3000+ coverage is not yet verifiable from the available public surface.".to_string(),
                },
            ],
        }
    }

    fn is_unavailable_page_text(text: &str) -> bool {
        let normalized = text.to_lowercase();
        normalized.contains("404 not found")
            || normalized.contains("букмекерская контора")
            || normalized.contains("официальный сайт")
    }

    fn is_plausible_team(value: &str) -> bool {
        if !is_valid_team_name(value) {
            return false;
        }

        let normalized = value.to_lowercase();
        !normalized.contains("404")
            && !normalized.contains("not found")
            && !normalized.contains("bet-m")
    }

    fn build_event_id(home: &str, away: &str, is_live: bool) -> String {
        let normalize = |value: &str| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_lowercase()
                    } else if ch.is_alphanumeric() {
                        ch
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .split('-')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        };

        format!(
            "betm-{}-{}-{}",
            if is_live { "live" } else { "prematch" },
            normalize(home),
            normalize(away)
        )
    }

    fn parse_headless_item(
        item: &serde_json::Value,
        fallback_live: bool,
    ) -> Option<(Event, Vec<Odd>)> {
        let home_team = item.get("home").and_then(|value| value.as_str())?.trim();
        let away_team = item.get("away").and_then(|value| value.as_str())?.trim();
        if !Self::is_plausible_team(home_team)
            || !Self::is_plausible_team(away_team)
            || home_team == away_team
        {
            return None;
        }

        let odds_values = item
            .get("odds")
            .and_then(|value| value.as_array())?
            .iter()
            .filter_map(|value| {
                value.as_f64().or_else(|| {
                    value
                        .as_str()
                        .and_then(|raw| raw.replace(',', ".").parse::<f64>().ok())
                })
            })
            .filter(|value| (1.01..=50.0).contains(value))
            .collect::<Vec<_>>();
        if odds_values.len() < 2 {
            return None;
        }

        let source_url = item
            .get("sourceUrl")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let is_live = if source_url.is_empty() {
            fallback_live
        } else {
            source_url.contains("/live")
        };
        let event_id = Self::build_event_id(home_team, away_team, is_live);
        let league = if is_live { "Live" } else { "Pre-match" }.to_string();

        let event = Event {
            id: event_id.clone(),
            sport: Sport::Football,
            league,
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: (!source_url.is_empty()).then(|| source_url.to_string()),
            extra: HashMap::new(),
        };

        let now = Utc::now();
        let mut odds = Vec::new();
        if odds_values.len() >= 3 {
            for (selection, odds_type, value) in [
                ("1", OddsType::Home, odds_values[0]),
                ("X", OddsType::Draw, odds_values[1]),
                ("2", OddsType::Away, odds_values[2]),
            ] {
                odds.push(Odd {
                    id: format!("{}-{}", event_id, selection),
                    event_id: event_id.clone(),
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    market: "1X2".into(),
                    selection: selection.into(),
                    odds: value,
                    odds_type,
                    line: None,
                    timestamp: now,
                });
            }
        } else {
            for (selection, odds_type, value) in [
                ("1", OddsType::Home, odds_values[0]),
                ("2", OddsType::Away, odds_values[1]),
            ] {
                odds.push(Odd {
                    id: format!("{}-{}", event_id, selection),
                    event_id: event_id.clone(),
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    market: "Moneyline".into(),
                    selection: selection.into(),
                    odds: value,
                    odds_type,
                    line: None,
                    timestamp: now,
                });
            }
        }

        Some((event, odds))
    }

    fn parse_headless_payload(
        payload: &[serde_json::Value],
        fallback_live: bool,
    ) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut seen = HashSet::new();

        for item in payload {
            if let Some((event, mut event_odds)) = Self::parse_headless_item(item, fallback_live) {
                if seen.insert(event.id.clone()) {
                    events.push(event);
                    odds.append(&mut event_odds);
                }
            }
        }

        (events, odds)
    }

    fn fetch_runtime_data_blocking(
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let helper = HeadlessChromeHelper::new()?;
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen_event_ids = HashSet::new();

        for probe in PROBES {
            let tab = match helper.navigate_and_wait(probe.url, HEADLESS_WAIT_MS) {
                Ok(tab) => tab,
                Err(error) => {
                    debug!(url = probe.url, error = %error, "BetM: navigation failed");
                    continue;
                }
            };

            let page_text = HeadlessChromeHelper::get_page_text(&tab).unwrap_or_default();
            if Self::is_unavailable_page_text(&page_text) {
                debug!(
                    url = probe.url,
                    "BetM: route returned unavailable placeholder page"
                );
                continue;
            }

            let mut payload = HeadlessChromeHelper::evaluate_json_with_retry(
                &tab,
                HEADLESS_EXTRACT_JS,
                HEADLESS_EVAL_ATTEMPTS,
                HEADLESS_RETRY_DELAY_MS,
            )
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();

            if payload.is_empty() {
                let _ = HeadlessChromeHelper::scroll_page(&tab);
                payload = HeadlessChromeHelper::evaluate_json_with_retry(
                    &tab,
                    HEADLESS_EXTRACT_JS,
                    HEADLESS_EVAL_ATTEMPTS,
                    HEADLESS_RETRY_DELAY_MS,
                )
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default();
            }

            let (events, odds) = Self::parse_headless_payload(&payload, probe.is_live);
            debug!(
                url = probe.url,
                items = payload.len(),
                events = events.len(),
                odds = odds.len(),
                "BetM: headless payload parsed"
            );

            for event in events {
                if seen_event_ids.insert(event.id.clone()) {
                    all_events.push(event);
                }
            }
            all_odds.extend(odds);
        }

        Ok((all_events, all_odds))
    }

    async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let _ = &self.client;
        tokio::task::spawn_blocking(Self::fetch_runtime_data_blocking)
            .await
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?
    }
}

#[async_trait]
impl BookmakerParser for BetMParser {
    fn name(&self) -> &str {
        "Bet-M"
    }

    fn slug(&self) -> &str {
        BOOKMAKER_SLUG
    }

    fn is_enabled(&self) -> bool {
        false
    }

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(Self::readiness_snapshot())
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let (events, _) = self.fetch_runtime_data().await?;
        info!(count = events.len(), "BetM: events fetched");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        info!(count = odds.len(), "BetM: odds fetched");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let (events, odds) = self.fetch_runtime_data().await?;
        let elapsed = started.elapsed().as_millis() as u64;

        if events.is_empty() {
            warn!("BetM: no public events detected on probed routes");
        }

        Ok(ParserResult::new(BOOKMAKER_SLUG, events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://bet-m.net"
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
    }
}

#[cfg(test)]
mod tests {
    use super::BetMParser;
    use shared::OddsType;

    #[test]
    fn parses_three_way_fixture_payload() {
        let payload: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/betm_headless_payload.json"))
                .expect("fixture should parse");

        let (events, odds) = BetMParser::parse_headless_payload(&payload, true);

        assert_eq!(events.len(), 2);
        assert_eq!(odds.len(), 5);

        let football = events
            .iter()
            .find(|event| event.home_team == "Спартак Москва")
            .expect("football event exists");
        assert!(football.is_live);
        assert_eq!(football.league, "Live");

        assert!(odds.iter().any(|odd| {
            odd.event_id == football.id
                && odd.selection == "X"
                && odd.market == "1X2"
                && odd.odds_type == OddsType::Draw
                && (odd.odds - 3.55).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.event_id.starts_with("betm-prematch")
                && odd.market == "Moneyline"
                && odd.selection == "2"
                && odd.odds_type == OddsType::Away
                && (odd.odds - 2.08).abs() < f64::EPSILON
        }));
    }

    #[test]
    fn treats_marketing_and_404_pages_as_unavailable() {
        assert!(BetMParser::is_unavailable_page_text(
            "404 Not Found\nHome\nBet-M"
        ));
        assert!(BetMParser::is_unavailable_page_text(
            "Bet-M букмекерская контора\nОфициальный сайт"
        ));
        assert!(!BetMParser::is_unavailable_page_text(
            "Лига чемпионов\nСпартак Москва\nЗенит"
        ));
    }
}
