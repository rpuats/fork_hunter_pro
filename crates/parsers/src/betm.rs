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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteKind {
    Live,
    Prematch,
    Unknown,
}

impl RouteKind {
    fn from_source_url(source_url: &str) -> Self {
        let normalized = source_url.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Self::Unknown;
        }

        if normalized.contains("/live") {
            Self::Live
        } else if normalized.contains("/line") {
            Self::Prematch
        } else {
            Self::Unknown
        }
    }

    fn is_live(self, fallback_live: bool) -> bool {
        match self {
            Self::Live => true,
            Self::Prematch => false,
            Self::Unknown => fallback_live,
        }
    }

    fn league_label(self, fallback_live: bool) -> String {
        if self.is_live(fallback_live) {
            "Live".to_string()
        } else {
            "Pre-match".to_string()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MarketKind {
    ThreeWay,
    Moneyline,
}

impl MarketKind {
    fn from_odds_len(odds_len: usize) -> Option<Self> {
        if odds_len >= 3 {
            Some(Self::ThreeWay)
        } else if odds_len >= 2 {
            Some(Self::Moneyline)
        } else {
            None
        }
    }

    fn market_name(self) -> &'static str {
        match self {
            Self::ThreeWay => "1X2",
            Self::Moneyline => "Moneyline",
        }
    }

    fn selections(self, odds_values: &[f64]) -> [Option<(&'static str, OddsType, f64)>; 3] {
        match self {
            Self::ThreeWay => [
                Some(("1", OddsType::Home, odds_values[0])),
                Some(("X", OddsType::Draw, odds_values[1])),
                Some(("2", OddsType::Away, odds_values[2])),
            ],
            Self::Moneyline => [
                Some(("1", OddsType::Home, odds_values[0])),
                Some(("2", OddsType::Away, odds_values[1])),
                None,
            ],
        }
    }
}

#[derive(Debug, Default)]
struct RuntimeProbeStats {
    probes_attempted: usize,
    probes_with_payload: usize,
    navigation_failures: usize,
    unavailable_pages: usize,
    empty_payloads: usize,
    payload_items: usize,
    parsed_events: usize,
    parsed_odds: usize,
}

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
    const splitPair = (value) => {
        const normalized = normalizeText(value);
        for (const separator of [' - ', ' -', '- ', ' – ', ' — ', ' vs ', ' VS ', ' v ']) {
            const index = normalized.indexOf(separator);
            if (index <= 0) continue;
            const home = normalizeText(normalized.slice(0, index));
            const away = normalizeText(normalized.slice(index + separator.length));
            if (isName(home) && isName(away) && home !== away) return [home, away];
        }
        return null;
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
                const pair = splitPair(line);
                if (pair) {
                    [home, away] = pair;
                    break;
                }
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
                    code: "legacy_net_routes_return_placeholder_404".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Bounded live proof: the public bet-m.net /live and /line routes answer with the same 404/marketing shell instead of sportsbook content.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "betm_ru_alias_resets_transport".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "Bounded live proof: the current betm.ru /live and /line aliases terminate TLS/HTTP before a public payload is served, so the route cannot be promoted beyond diagnostics.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "public_feed_not_confirmed".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "Current public routes have one bounded blocker profile: bet-m.net resolves to a placeholder 404 shell and betm.ru resets transport before feed content is exposed, so scan enablement remains off.".to_string(),
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

    fn split_match_title(value: &str) -> Option<(String, String)> {
        for separator in [" - ", " -", "- ", " – ", " — ", " vs ", " VS ", " v "] {
            let Some(position) = value.find(separator) else {
                continue;
            };

            let home = value[..position].trim();
            let away = value[position + separator.len()..].trim();
            if Self::is_plausible_team(home) && Self::is_plausible_team(away) && home != away {
                return Some((home.to_string(), away.to_string()));
            }
        }

        None
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

    fn detect_live_state(source_url: &str, fallback_live: bool) -> bool {
        RouteKind::from_source_url(source_url).is_live(fallback_live)
    }

    fn parse_headless_item(
        item: &serde_json::Value,
        fallback_live: bool,
    ) -> Option<(Event, Vec<Odd>)> {
        let home_team = item
            .get("home")
            .and_then(|value| value.as_str())
            .map(str::trim);
        let away_team = item
            .get("away")
            .and_then(|value| value.as_str())
            .map(str::trim);
        let (home_team, away_team) = match (home_team, away_team) {
            (Some(home_team), Some(away_team))
                if Self::is_plausible_team(home_team)
                    && Self::is_plausible_team(away_team)
                    && home_team != away_team =>
            {
                (home_team.to_string(), away_team.to_string())
            }
            (Some(home_team), _) | (_, Some(home_team)) => Self::split_match_title(home_team)?,
            _ => return None,
        };

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
        let route_kind = RouteKind::from_source_url(source_url);
        let is_live = route_kind.is_live(fallback_live);
        let event_id = Self::build_event_id(&home_team, &away_team, is_live);
        let league = route_kind.league_label(fallback_live);
        let market_kind = MarketKind::from_odds_len(odds_values.len())?;

        let event = Event {
            id: event_id.clone(),
            sport: Sport::Football,
            league,
            home_team: home_team.clone(),
            away_team: away_team.clone(),
            start_time: None,
            is_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: (!source_url.is_empty()).then(|| source_url.to_string()),
            extra: HashMap::new(),
        };

        let now = Utc::now();
        let mut odds = Vec::new();
        for (selection, odds_type, value) in
            market_kind.selections(&odds_values).into_iter().flatten()
        {
            odds.push(Odd {
                id: format!("{}-{}", event_id, selection),
                event_id: event_id.clone(),
                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                market: market_kind.market_name().into(),
                selection: selection.into(),
                odds: value,
                odds_type,
                line: None,
                timestamp: now,
            });
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
        let mut seen_odd_ids = HashSet::new();
        let mut stats = RuntimeProbeStats::default();

        for probe in PROBES {
            stats.probes_attempted += 1;

            let tab = match helper.navigate_and_wait(probe.url, HEADLESS_WAIT_MS) {
                Ok(tab) => tab,
                Err(error) => {
                    stats.navigation_failures += 1;
                    debug!(url = probe.url, error = %error, "BetM: navigation failed");
                    continue;
                }
            };

            let page_text = HeadlessChromeHelper::get_page_text(&tab).unwrap_or_default();
            if Self::is_unavailable_page_text(&page_text) {
                stats.unavailable_pages += 1;
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

            if payload.is_empty() {
                stats.empty_payloads += 1;
                debug!(url = probe.url, "BetM: probe returned empty payload");
                continue;
            }

            let (events, odds) = Self::parse_headless_payload(&payload, probe.is_live);
            stats.probes_with_payload += 1;
            stats.payload_items += payload.len();
            stats.parsed_events += events.len();
            stats.parsed_odds += odds.len();
            let new_events = events
                .iter()
                .filter(|event| !seen_event_ids.contains(&event.id))
                .count();
            let new_odds = odds
                .iter()
                .filter(|odd| !seen_odd_ids.contains(&odd.id))
                .count();
            debug!(
                url = probe.url,
                items = payload.len(),
                events = events.len(),
                odds = odds.len(),
                new_events,
                new_odds,
                "BetM: headless payload parsed"
            );

            for event in events {
                if seen_event_ids.insert(event.id.clone()) {
                    all_events.push(event);
                }
            }
            for odd in odds {
                if seen_odd_ids.insert(odd.id.clone()) {
                    all_odds.push(odd);
                }
            }
        }

        info!(
            probes_attempted = stats.probes_attempted,
            probes_with_payload = stats.probes_with_payload,
            navigation_failures = stats.navigation_failures,
            unavailable_pages = stats.unavailable_pages,
            empty_payloads = stats.empty_payloads,
            payload_items = stats.payload_items,
            parsed_events = stats.parsed_events,
            parsed_odds = stats.parsed_odds,
            unique_events = all_events.len(),
            unique_odds = all_odds.len(),
            "BetM: runtime probe summary"
        );

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
    use super::{BetMParser, MarketKind, RouteKind};
    use shared::OddsType;

    #[test]
    fn parses_three_way_fixture_payload() {
        let payload: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/betm_headless_payload.json"))
                .expect("fixture should parse");

        let (events, odds) = BetMParser::parse_headless_payload(&payload, true);

        assert_eq!(events.len(), 3);
        assert_eq!(odds.len(), 8);

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
        assert!(events.iter().any(|event| {
            event.home_team == "Реал Мадрид" && event.away_team == "Барселона" && event.is_live
        }));
        assert_eq!(
            events
                .iter()
                .filter(|event| event.home_team == "Спартак Москва" && event.away_team == "Зенит")
                .count(),
            1
        );
    }

    #[test]
    fn detects_live_state_from_source_url_with_fallback_for_unknown_routes() {
        assert!(BetMParser::detect_live_state(
            "https://bet-m.net/live",
            false
        ));
        assert!(!BetMParser::detect_live_state("https://betm.ru/line", true));
        assert!(BetMParser::detect_live_state(
            "https://betm.ru/sports",
            true
        ));
        assert!(!BetMParser::detect_live_state("", false));
    }

    #[test]
    fn classifies_route_kind_explicitly() {
        assert_eq!(
            RouteKind::from_source_url("https://bet-m.net/live"),
            RouteKind::Live
        );
        assert_eq!(
            RouteKind::from_source_url("https://betm.ru/line"),
            RouteKind::Prematch
        );
        assert_eq!(
            RouteKind::from_source_url("https://betm.ru/sports"),
            RouteKind::Unknown
        );
        assert_eq!(RouteKind::from_source_url(""), RouteKind::Unknown);
    }

    #[test]
    fn infers_market_kind_from_odds_arity() {
        assert_eq!(MarketKind::from_odds_len(3), Some(MarketKind::ThreeWay));
        assert_eq!(MarketKind::from_odds_len(2), Some(MarketKind::Moneyline));
        assert_eq!(MarketKind::from_odds_len(1), None);
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

    #[test]
    fn readiness_snapshot_keeps_betm_diagnostic_only() {
        let readiness = BetMParser::readiness_snapshot();

        assert_eq!(
            readiness.stage,
            shared::ParserReadinessStage::DiagnosticOnly
        );
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "legacy_dom_path_ported"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "legacy_and_current_domains_probed"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "legacy_net_routes_return_placeholder_404"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "betm_ru_alias_resets_transport"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "public_feed_not_confirmed"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "product_target_unverified"));
    }
}
