use crate::base::{BookmakerParser, ParserResult};
use crate::headless_helper::{is_valid_team_name, HeadlessChromeHelper};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Winline parser.
///
/// The practical public runtime path is a headless SPA extraction flow modeled after the legacy
/// Playwright parser from `scanner/parsers/winline_playwright.py`.
///
/// Public HTTP endpoints in repository research still do not provide a confirmed event feed, so
/// the Rust parser now prefers rendered DOM extraction and keeps lightweight HTML bootstrap
/// probing only as a fallback when headless collection fails.
#[derive(Debug)]
pub struct WinlineParser {
    client: Arc<Client>,
}

const BASE_URL: &str = "https://winline.ru";
const HEADLESS_WAIT_MS: u64 = 1_800;
const HEADLESS_RETRY_DELAY_MS: u64 = 500;
const HEADLESS_EVAL_ATTEMPTS: usize = 3;
const HEADLESS_SCROLL_ROUNDS: usize = 2;
const HEADLESS_MAX_PREMATCH_PAGES: usize = 18;
const HEADLESS_MAX_LIVE_SPORT_PAGES: usize = 8;
const HEADLESS_PREMATCH_EMPTY_STREAK_LIMIT: usize = 6;
const HEADLESS_LIVE_EMPTY_STREAK_LIMIT: usize = 4;
const HEADLESS_LIVE_FANOUT_BUDGET_MS: u64 = 18_000;
const HEADLESS_PREMATCH_FANOUT_BUDGET_MS: u64 = 18_000;
const HEADLESS_RUNTIME_BUDGET_MS: u64 = 70_000;
const HEADLESS_ROUTE_GUARD_MS: u64 = 7_000;
const HEADLESS_NAVIGATION_TIMEOUT_MS: u64 = 6_000;
const HEADLESS_EXPENSIVE_ROUTE_MS: u64 = 12_000;
const HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT: usize = 2;
const TARGET_LIVE_EVENTS: usize = 150;
const TARGET_PREMATCH_EVENTS: usize = 3000;
const PLAYWRIGHT_WAIT_MS: u64 = 1_500;
const DISCOVERY_URL: &str = "https://winline.ru/stavki/sport/futbol/";
const LIVE_URL: &str = "https://winline.ru/live";
const BOOTSTRAP_WEBSCRIPT_PATH: &str = "/api/v2/webscript.js";
const DISCOVERED_WS_URL: &str = "wss://wss.winline.ru/data_ng?client=newsite&nb=true";
const DISCOVERED_WS_INIT_COMMANDS: &[&str] = &["lang", "ru", "data", "WINLINE", "getdate"];
const DISCOVERED_EVENT_FILTER_HINTS: &[&str] = &[
    "Events.filter({isLive:1})",
    "Events.filter({isLive:0,category:[1,2,3]})",
    "Events.filter({idSport:<id>,isLive:0})",
    "Events.filter({idChampionship:<id>,isLive:1})",
];
const DISCOVERED_LINE_COMMAND_HINTS: &[&str] = &["event.plus", "SM.PREDLINE", "PREDLINELIVE"];
const PLAYWRIGHT_PRIORITY_PATHS: &[&str] = &[
    "/stavki/sport/nastolijnyj_tennis",
    "/stavki/sport/bejsbol",
    "/stavki/sport/tennis",
    "/stavki/sport/gandbol",
    "/stavki/sport/regbi",
    "/stavki/sport/futbol",
    "/stavki/sport/basketbol",
    "/stavki/sport/volejbol",
    "/stavki/sport/darts",
    "/stavki/sport/snuker",
];

#[derive(Clone, Copy, Debug)]
struct HtmlProbe {
    path: &'static str,
    sport: Sport,
    is_live: bool,
}

#[derive(Debug, Default)]
struct BootstrapHints {
    script_sources: Vec<String>,
    has_webscript: bool,
    has_main_bundle: bool,
    has_runtime_bundle: bool,
}

#[derive(Clone, Debug, Default)]
struct HeadlessSeedPaths {
    prematch: Vec<String>,
    live: Vec<String>,
}

#[derive(Debug)]
struct HeadlessRouteMetric {
    phase: &'static str,
    path: String,
    sport: Sport,
    status: &'static str,
    payload_items: usize,
    added_events: usize,
    navigation_ms: u64,
    extraction_ms: u64,
    collect_ms: u64,
    total_ms: u64,
    expensive: bool,
}

const HTML_PROBES: &[HtmlProbe] = &[
    HtmlProbe {
        path: "/football",
        sport: Sport::Football,
        is_live: false,
    },
    HtmlProbe {
        path: "/live/football",
        sport: Sport::Football,
        is_live: true,
    },
    HtmlProbe {
        path: "/stavki/sport/futbol/",
        sport: Sport::Football,
        is_live: false,
    },
    HtmlProbe {
        path: "/now/",
        sport: Sport::Football,
        is_live: true,
    },
];

const HEADLESS_EXTRACT_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    const textFrom = (node) => {
        if (!node) return '';
        const title = typeof node.getAttribute === 'function' ? node.getAttribute('title') : null;
        return normalizeText(title || node.textContent || '');
    };
    const splitMatchName = (value) => {
        const normalized = normalizeText(value);
        if (!normalized) return null;
        for (const separator of [' - ', ' -', '- ', ' – ', ' — ', ' vs ', ' VS ']) {
            const index = normalized.indexOf(separator);
            if (index === -1) continue;
            const home = normalizeText(normalized.slice(0, index));
            const away = normalizeText(normalized.slice(index + separator.length));
            if (home && away) return [home, away];
        }
        return null;
    };
    const isValidName = (name) => {
        if (!name || name.length < 2 || name.length > 80) return false;
        if (name === '-' || /^[-\s]+$/.test(name)) return false;
        if (/^(event|match|game|live|pre)/i.test(name)) return false;
        return true;
    };
    const parseOdds = (value) => {
        const normalized = normalizeText(value).replace(',', '.');
        if (!normalized) return null;
        const parsed = Number.parseFloat(normalized);
        return Number.isFinite(parsed) && parsed >= 1.01 && parsed <= 100 ? parsed : null;
    };
    const parseLine = (value) => {
        const match = normalizeText(value).match(/(\d+(?:[.,]\d+)?)/);
        return match ? match[1].replace(',', '.') : null;
    };
    const results = [];
    const seen = new Set();
    const pushEvent = (event) => {
        if (!isValidName(event.home) || !isValidName(event.away)) return;
        if (!event.odds || event.odds.length < 2) return;
        const key = [event.eventId || '', event.home, event.away, event.href || ''].join('|');
        if (seen.has(key)) return;
        seen.add(key);
        results.push(event);
    };

    const pathMatch = window.location.pathname.match(/\/stavki\/sport\/([^/?#]+)/);
    const pageSportSlug = pathMatch ? normalizeText(pathMatch[1]) : '';
    const extractNames = (scope) => {
        if (!scope) return [];

        const selectors = [
            '.half__names .name',
            '.body-left__names .name',
            '.card__competitors .name',
            '.competitor__name',
            '.name',
        ];
        for (const selector of selectors) {
            const names = Array.from(scope.querySelectorAll(selector))
                .filter((node) => node.children.length === 0)
                .map((node) => textFrom(node))
                .filter(isValidName);
            if (names.length >= 2) {
                return names.slice(0, 2);
            }
            if (names.length === 1) {
                const split = splitMatchName(names[0]);
                if (split && split.every(isValidName)) return split;
            }
        }

        const titleCandidates = [
            textFrom(scope.querySelector('.main-event__title')),
            textFrom(scope.querySelector('.card__title')),
            textFrom(scope.querySelector('[title*=" - "]')),
            textFrom(scope),
        ];
        for (const candidate of titleCandidates) {
            const split = splitMatchName(candidate);
            if (split && split.every(isValidName)) return split;
        }

        return [];
    };
    const collectMarkets = (root) => Array.from(root.querySelectorAll('ww-feature-event-market-dsk')).map((market) => {
        const buttons = Array.from(market.querySelectorAll('.coefficient-button_fill, .button__coef-title, .main-event__coeff'))
            .map((button) => parseOdds(button.textContent || ''))
            .filter((value) => value !== null);
        return {
            buttons,
            middle: textFrom(market.querySelector('.coefficient-middle, .coefficient-middle__selector')),
            text: normalizeText(market.textContent || ''),
        };
    }).filter((market) => market.buttons.length >= 2);
    const inferLive = (scope, fallbackText) => {
        const cardText = normalizeText(fallbackText || scope?.textContent || '');
        return Boolean(
            scope?.querySelector('.header-left__live-logo') ||
            scope?.querySelector('.card--live') ||
            /\/live(?:$|[/?#])/.test(window.location.pathname) ||
            /(?:^|\s)(?:1[ТT]|2[ТT]|3[ТT]|4[ТT]|1P|2P|3P|OT|SO|SET|СЕТ|ИГРАЮТ|LIVE)\b/.test(cardText) ||
            /\d+['’]/.test(cardText)
        );
    };
    const pickOdds = (markets) => {
        const selected = markets.find((market) => market.buttons.length >= 3) || markets[0];
        if (!selected) return null;
        const odds = selected.buttons.slice(0, selected.buttons.length >= 3 ? 3 : 2);
        if (odds.length < 2) return null;
        return {
            odds,
            totalLine: odds.length === 2 ? parseLine(selected.middle || selected.text) : null,
        };
    };
    const tournamentName = (card) => textFrom(
        card.closest('ww-feature-block-tournament-dsk')?.querySelector('.block-tournament-header__info, .block-tournament-header')
    );
    const sportName = (card) => textFrom(
        card.closest('ww-feature-block-sport-dsk')?.querySelector('.block-sport-header__info, .block-sport-header')
    );

    Array.from(document.querySelectorAll('ww-feature-block-event-dsk')).forEach((card) => {
        try {
            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const cardIdMatch = normalizeText(card.id || '').match(/eventId-(\d+)/);
            const eventId = hrefMatch ? hrefMatch[1] : (cardIdMatch ? cardIdMatch[1] : '');
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;
            const cardText = normalizeText(card.textContent || '');

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card),
                eventId,
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: inferLive(card, cardText),
                sportName: sportName(card),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    Array.from(document.querySelectorAll('ww-feature-event-mini-card-dsk, .main-event')).forEach((card) => {
        try {
            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card) || textFrom(card.querySelector('.main-event__tournament')),
                eventId: hrefMatch ? hrefMatch[1] : '',
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: inferLive(card),
                sportName: sportName(card),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    Array.from(document.querySelectorAll('a[href*="/stavki/event/"]')).forEach((link) => {
        try {
            const href = normalizeText(link.getAttribute('href') || '');
            const eventIdMatch = href.match(/\/stavki\/event\/(\d+)/);
            const eventId = eventIdMatch ? eventIdMatch[1] : '';
            const card = link.closest('ww-feature-block-event-dsk, ww-feature-event-mini-card-dsk, .card, .event-card, [id^="eventId-"]') || link.parentElement;
            if (!card) return;

            const nameEls = extractNames(link).length >= 2 ? extractNames(link) : extractNames(card);
            if (nameEls.length < 2) return;

            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;
            const cardText = normalizeText(card.textContent || '');

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card),
                eventId,
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: inferLive(card, cardText),
                sportName: sportName(card),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    return results;
})()"#;

const HEADLESS_DISCOVER_SPORT_LINKS_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    const seen = new Set();
    const results = [];

    Array.from(document.querySelectorAll('a[href^="/stavki/sport/"]')).forEach((link) => {
        const href = normalizeText(link.getAttribute('href') || '');
        if (!href || seen.has(href)) return;
        seen.add(href);
        results.push({
            href,
            text: normalizeText(link.textContent || ''),
        });
    });

    return results;
})()"#;

fn is_valid_name(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 2
        && value.len() <= 80
        && !value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
        && !value.eq_ignore_ascii_case("n/a")
        && !value.eq_ignore_ascii_case("tbd")
}

fn split_event_name(name: &str) -> Option<(String, String)> {
    for separator in [" - ", " -", "- ", " – ", " — ", " vs ", " VS "] {
        if let Some(position) = name.find(separator) {
            let home = name[..position].trim().to_string();
            let away = name[position + separator.len()..].trim().to_string();
            if is_valid_name(&home) && is_valid_name(&away) {
                return Some((home, away));
            }
        }
    }

    None
}

fn parse_odds_value(value: &serde_json::Value) -> Option<f64> {
    let parsed = value
        .as_f64()
        .or_else(|| value.as_i64().map(|raw| raw as f64))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.replace(',', ".").parse::<f64>().ok())
        })?;

    (1.01..=200.0).contains(&parsed).then_some(parsed)
}

fn parse_line_value(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|raw| raw as f64))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.replace(',', ".").parse::<f64>().ok())
        })
}

fn parse_truthy_bool(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(value) => value.as_i64().map(|raw| raw != 0),
        serde_json::Value::String(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "y" | "live" => Some(true),
                "0" | "false" | "no" | "n" | "prematch" | "pre" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn normalize_winline_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_origin = trimmed
        .strip_prefix(BASE_URL)
        .or_else(|| trimmed.strip_prefix("https://www.winline.ru"))
        .unwrap_or(trimmed);
    let without_query = without_origin
        .split(['?', '#'])
        .next()
        .unwrap_or(without_origin)
        .trim();
    if without_query.is_empty() {
        return "/".to_string();
    }

    let mut normalized = if without_query.starts_with('/') {
        without_query.to_string()
    } else {
        format!("/{}", without_query)
    };

    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    normalized
}

fn sport_from_winline_hint(value: &str, fallback: Sport) -> Sport {
    let lower = value.trim().to_lowercase();
    if lower.is_empty() {
        return fallback;
    }

    if lower.contains("nastol") || lower.contains("table-tennis") || lower.contains("table_tennis")
    {
        return Sport::TableTennis;
    }
    if lower.contains("kiber") || lower.contains("cyber") || lower.contains("esport") {
        return Sport::Esports;
    }
    if lower.contains("futbol") || lower.contains("football") || lower.contains("soccer") {
        return Sport::Football;
    }
    if lower.contains("basket") {
        return Sport::Basketball;
    }
    if lower.contains("xok") || lower.contains("hockey") {
        return Sport::Hockey;
    }
    if lower.contains("tennis") {
        return Sport::Tennis;
    }
    if lower.contains("volej") || lower.contains("volleyball") {
        return Sport::Volleyball;
    }
    if lower.contains("gand") || lower.contains("handball") {
        return Sport::Handball;
    }
    if lower.contains("bejs") || lower.contains("baseball") {
        return Sport::Baseball;
    }
    if lower.contains("regbi") || lower.contains("rugby") {
        return Sport::Rugby;
    }
    if lower.contains("futz") || lower.contains("futsal") {
        return Sport::Futsal;
    }
    if lower.contains("vodnoe_polo") || lower.contains("water polo") {
        return Sport::WaterPolo;
    }
    if lower.contains("darts") {
        return Sport::Darts;
    }
    if lower.contains("snuker") || lower.contains("snooker") {
        return Sport::Snooker;
    }
    if lower.contains("florbol") || lower.contains("floorball") {
        return Sport::Floorball;
    }
    if lower.contains("badminton") {
        return Sport::Badminton;
    }
    if lower.contains("motorsport") || lower.contains("avtosport") {
        return Sport::Motorsport;
    }
    if lower.contains("golijf") || lower.contains("golf") {
        return Sport::Golf;
    }
    if lower.contains("plyazhnyj_volejbol") || lower.contains("beach volleyball") {
        return Sport::BeachVolleyball;
    }

    let parsed = Sport::from_str(&lower);
    if parsed == Sport::Other {
        fallback
    } else {
        parsed
    }
}

impl WinlineParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn merge_runtime_results(
        target_events: &mut Vec<Event>,
        target_odds: &mut Vec<Odd>,
        seen_event_ids: &mut HashSet<String>,
        events: Vec<Event>,
        odds: Vec<Odd>,
    ) {
        for event in events {
            if seen_event_ids.insert(event.id.clone()) {
                target_events.push(event);
            }
        }
        target_odds.extend(odds);
    }

    fn extract_odds_field(item: &serde_json::Value, keys: &[&str]) -> Option<f64> {
        for key in keys {
            if let Some(value) = item.get(*key).and_then(parse_odds_value) {
                return Some(value);
            }
        }

        if let Some(odds_object) = item.get("odds") {
            for key in keys {
                if let Some(value) = odds_object.get(*key).and_then(parse_odds_value) {
                    return Some(value);
                }
            }
        }

        None
    }

    fn parse_headless_item(
        item: &serde_json::Value,
        fallback_sport: Sport,
        fallback_live: bool,
        source_url: &str,
    ) -> Option<(Event, Vec<Odd>)> {
        let home_team = item.get("home").and_then(|value| value.as_str())?.trim();
        let away_team = item.get("away").and_then(|value| value.as_str())?.trim();
        if !is_valid_team_name(home_team) || !is_valid_team_name(away_team) {
            return None;
        }

        let odds_values = item
            .get("odds")
            .and_then(|value| value.as_array())?
            .iter()
            .filter_map(parse_odds_value)
            .collect::<Vec<_>>();
        if odds_values.len() < 2 {
            return None;
        }

        let raw_id = item
            .get("eventId")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| {
                format!(
                    "{}-{}-{}",
                    fallback_sport,
                    home_team.replace(' ', "_"),
                    away_team.replace(' ', "_")
                )
            });
        let event_id = format!("winline-{}", raw_id);
        let league = item
            .get("league")
            .or_else(|| item.get("tournament"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let sport = item
            .get("sportName")
            .or_else(|| item.get("sportSlug"))
            .and_then(|value| value.as_str())
            .map(|value| sport_from_winline_hint(value, fallback_sport))
            .unwrap_or(fallback_sport);
        let href = item
            .get("href")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("{}{}", BASE_URL, value))
            .unwrap_or_else(|| source_url.to_string());
        let is_live = item
            .get("isLive")
            .or_else(|| item.get("live"))
            .or_else(|| item.get("is_live"))
            .and_then(parse_truthy_bool)
            .unwrap_or(fallback_live);

        let mut extra = HashMap::new();
        extra.insert(
            "source_url".to_string(),
            serde_json::Value::String(source_url.to_string()),
        );

        let event = Event {
            id: event_id.clone(),
            sport,
            league,
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: "winline".to_string(),
            raw_url: Some(href),
            extra,
        };

        let now = Utc::now();
        let mut odds = Vec::new();
        if odds_values.len() >= 3 {
            let selections = [
                ("1", OddsType::Home, odds_values[0]),
                ("X", OddsType::Draw, odds_values[1]),
                ("2", OddsType::Away, odds_values[2]),
            ];
            for (selection, odds_type, value) in selections {
                odds.push(Odd {
                    id: format!("{}-{}", event_id, selection),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "1X2".into(),
                    selection: selection.into(),
                    odds: value,
                    odds_type,
                    line: None,
                    timestamp: now,
                });
            }
        } else {
            let line = item.get("totalLine").and_then(parse_line_value);
            odds.push(Odd {
                id: format!("{}-over", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: odds_values[0],
                odds_type: OddsType::Over,
                line,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-under", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: odds_values[1],
                odds_type: OddsType::Under,
                line,
                timestamp: now,
            });
        }

        Some((event, odds))
    }

    fn extract_headless_payload(tab: &headless_chrome::Tab) -> Vec<serde_json::Value> {
        HeadlessChromeHelper::evaluate_json_with_retry(
            tab,
            HEADLESS_EXTRACT_JS,
            HEADLESS_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
    }

    fn extract_discovered_sport_links(tab: &headless_chrome::Tab) -> Vec<String> {
        HeadlessChromeHelper::evaluate_json_with_retry(
            tab,
            HEADLESS_DISCOVER_SPORT_LINKS_JS,
            HEADLESS_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            entry
                .get("href")
                .and_then(|value| value.as_str())
                .map(normalize_winline_path)
        })
        .filter(|path| path.starts_with("/stavki/sport/"))
        .collect()
    }

    fn extract_sport_links_from_html(html: &str) -> Vec<String> {
        let mut links = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = 0;

        while let Some(anchor_start) = html[cursor..].find("<a") {
            let absolute_start = cursor + anchor_start;
            let Some(tag_end_rel) = html[absolute_start..].find('>') else {
                break;
            };
            let tag = &html[absolute_start..absolute_start + tag_end_rel];
            if let Some(href) = Self::extract_attr_value(tag, "href") {
                let normalized = normalize_winline_path(&href);
                if normalized.starts_with("/stavki/sport/") && seen.insert(normalized.clone()) {
                    links.push(normalized);
                }
            }
            cursor = absolute_start + tag_end_rel + 1;
        }

        links
    }

    async fn fetch_seed_paths(&self) -> HeadlessSeedPaths {
        let html = match self
            .client
            .get(DISCOVERY_URL)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await
        {
            Ok(response) => match response.text().await {
                Ok(html) => html,
                Err(error) => {
                    debug!(error = %error, "Winline: failed reading seed HTML response");
                    String::new()
                }
            },
            Err(error) => {
                debug!(error = %error, "Winline: failed requesting seed HTML response");
                String::new()
            }
        };

        let mut prematch = Self::extract_sport_links_from_html(&html);
        if !prematch.iter().any(|path| path == "/stavki/sport/futbol") {
            prematch.insert(0, "/stavki/sport/futbol".to_string());
        }

        let live = prematch
            .iter()
            .map(|path| {
                path.replacen("/stavki/sport/", "/live/sport/", 1)
                    .trim_end_matches('/')
                    .to_string()
            })
            .collect();

        HeadlessSeedPaths { prematch, live }
    }

    fn extract_from_tab(tab: &headless_chrome::Tab, source_url: &str) -> Vec<serde_json::Value> {
        let mut payload = Self::extract_headless_payload(tab);
        debug!(
            url = source_url,
            items = payload.len(),
            "Winline: initial tab extraction"
        );

        for round in 0..HEADLESS_SCROLL_ROUNDS {
            let _ = HeadlessChromeHelper::scroll_page(tab);
            let next_payload = Self::extract_headless_payload(tab);
            debug!(
                url = source_url,
                round = round + 1,
                items = next_payload.len(),
                "Winline: payload after scroll"
            );
            if next_payload.len() > payload.len() {
                payload = next_payload;
            } else {
                debug!(
                    url = source_url,
                    round = round + 1,
                    items = payload.len(),
                    "Winline: stopping scroll loop after payload plateau"
                );
                break;
            }
        }

        payload
    }

    fn prioritized_playwright_paths() -> Vec<String> {
        let mut prioritized = Vec::new();
        let mut seen = HashSet::new();

        for path in PLAYWRIGHT_PRIORITY_PATHS {
            let normalized = normalize_winline_path(path);
            if seen.insert(normalized.clone()) {
                prioritized.push(normalized);
            }
        }

        prioritized
    }

    fn prioritized_headless_paths(paths: Vec<String>, is_live: bool) -> Vec<String> {
        let mut prioritized = Vec::new();
        let mut seen = HashSet::new();

        for path in PLAYWRIGHT_PRIORITY_PATHS {
            let normalized = normalize_winline_path(path);
            let candidate = if is_live {
                normalized.replacen("/stavki/sport/", "/live/sport/", 1)
            } else {
                normalized
            };
            if seen.insert(candidate.clone()) {
                prioritized.push(candidate);
            }
        }

        for path in paths {
            let normalized = normalize_winline_path(&path);
            if seen.insert(normalized.clone()) {
                prioritized.push(normalized);
            }
        }

        prioritized
    }

    fn playwright_extraction_script(paths: &[String]) -> String {
        let paths_json = serde_json::to_string(paths).unwrap_or_else(|_| "[]".to_string());
        format!(
            r#"import json
import sys
from playwright.sync_api import sync_playwright

PATHS = json.loads({paths_json:?})
WAIT_MS = {wait_ms}
EXTRACT_JS = r'''{extract_js}'''

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page(
        user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
        locale='ru-RU',
        viewport={{'width': 1440, 'height': 2200}},
    )
    results = []
    seen = set()
    errors = []
    for path in PATHS:
        url = 'https://winline.ru' + path
        try:
            page.goto(url, wait_until='domcontentloaded', timeout=60000)
            page.wait_for_timeout(WAIT_MS)
            payload = page.evaluate(EXTRACT_JS)
            for item in payload:
                key = '|'.join([
                    str(item.get('eventId') or ''),
                    str(item.get('home') or ''),
                    str(item.get('away') or ''),
                    str(item.get('href') or ''),
                ])
                if key in seen:
                    continue
                seen.add(key)
                results.append(item)
        except Exception as error:
            errors.append({{'url': url, 'error': str(error)}})
    browser.close()
    sys.stdout.write(json.dumps({{'items': results, 'errors': errors}}, ensure_ascii=False))
"#,
            paths_json = paths_json,
            wait_ms = PLAYWRIGHT_WAIT_MS,
            extract_js = HEADLESS_EXTRACT_JS,
        )
    }

    fn fetch_via_playwright_blocking(
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let paths = Self::prioritized_playwright_paths();
        let script = Self::playwright_extraction_script(&paths);
        let output = Command::new("python")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;

                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(script.as_bytes())?;
                }
                child.wait_with_output()
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                format!("python exited with status {}", output.status)
            } else {
                stderr
            };
            return Err(message.into());
        }

        let payload: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let items = payload
            .get("items")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        if let Some(errors) = payload.get("errors").and_then(|value| value.as_array()) {
            for error in errors {
                if let (Some(url), Some(message)) = (
                    error.get("url").and_then(|value| value.as_str()),
                    error.get("error").and_then(|value| value.as_str()),
                ) {
                    debug!(
                        url = url,
                        error = message,
                        "Winline: Playwright page extraction failed"
                    );
                }
            }
        }

        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut seen = HashSet::new();
        for item in items {
            if let Some((event, mut event_odds)) =
                Self::parse_headless_item(&item, Sport::Other, false, BASE_URL)
            {
                if seen.insert(event.id.clone()) {
                    events.push(event);
                }
                odds.append(&mut event_odds);
            }
        }

        Ok((events, odds))
    }

    async fn fetch_via_playwright(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let _ = &self.client;
        tokio::task::spawn_blocking(Self::fetch_via_playwright_blocking)
            .await
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?
    }

    fn target_counts_reached(events: &[Event]) -> bool {
        let live = events.iter().filter(|event| event.is_live).count();
        let prematch = events.len().saturating_sub(live);
        live >= TARGET_LIVE_EVENTS && prematch >= TARGET_PREMATCH_EVENTS
    }

    fn should_run_playwright_fallback(events: &[Event]) -> bool {
        events.is_empty()
    }

    fn collect_headless_page(
        all_events: &mut Vec<Event>,
        all_odds: &mut Vec<Odd>,
        seen: &mut HashSet<String>,
        payload: Vec<serde_json::Value>,
        fallback_sport: Sport,
        fallback_live: bool,
        source_url: &str,
    ) -> usize {
        let mut added_events = 0;
        for item in payload {
            if let Some((event, mut odds)) =
                Self::parse_headless_item(&item, fallback_sport, fallback_live, source_url)
            {
                if seen.insert(event.id.clone()) {
                    all_events.push(event);
                    added_events += 1;
                }
                all_odds.append(&mut odds);
            }
        }

        added_events
    }

    fn runtime_budget_exhausted(started: Instant) -> bool {
        started.elapsed().as_millis() as u64 >= HEADLESS_RUNTIME_BUDGET_MS
    }

    fn runtime_elapsed_ms(started: Instant) -> u64 {
        started.elapsed().as_millis() as u64
    }

    fn runtime_remaining_budget_ms(started: Instant) -> u64 {
        HEADLESS_RUNTIME_BUDGET_MS.saturating_sub(Self::runtime_elapsed_ms(started))
    }

    fn runtime_budget_allows_next_route(started: Instant) -> bool {
        Self::runtime_remaining_budget_ms(started) >= HEADLESS_ROUTE_GUARD_MS
    }

    fn fanout_budget_exhausted(started: Instant, budget_ms: u64) -> bool {
        started.elapsed().as_millis() as u64 >= budget_ms
    }

    fn log_route_budget_break(
        phase: &str,
        next_path: &str,
        started: Instant,
        events: &[Event],
        odds: usize,
        route_guard_ms: u64,
        reason: &str,
    ) {
        let live = events.iter().filter(|event| event.is_live).count();
        let prematch = events.len().saturating_sub(live);
        warn!(
            phase = phase,
            next_path = next_path,
            elapsed_ms = Self::runtime_elapsed_ms(started),
            remaining_budget_ms = Self::runtime_remaining_budget_ms(started),
            budget_ms = HEADLESS_RUNTIME_BUDGET_MS,
            route_guard_ms = route_guard_ms,
            reason = reason,
            total = events.len(),
            live = live,
            prematch = prematch,
            odds = odds,
            "Winline: stopping headless route walk after runtime budget exhaustion"
        );
    }

    fn log_phase_result(
        phase: &str,
        source_url: &str,
        payload_items: usize,
        added_events: usize,
        route_started: Instant,
        navigation_ms: u64,
        extraction_ms: u64,
        collect_ms: u64,
        total_events: &[Event],
        odds: usize,
    ) {
        let live = total_events.iter().filter(|event| event.is_live).count();
        let prematch = total_events.len().saturating_sub(live);
        info!(
            phase = phase,
            url = source_url,
            payload_items = payload_items,
            added_events = added_events,
            navigation_ms = navigation_ms,
            extraction_ms = extraction_ms,
            collect_ms = collect_ms,
            route_elapsed_ms = route_started.elapsed().as_millis() as u64,
            total = total_events.len(),
            live = live,
            prematch = prematch,
            odds = odds,
            "Winline: headless bootstrap route processed"
        );
    }

    fn log_route_result(
        phase: &str,
        url: &str,
        fallback_sport: Sport,
        payload_items: usize,
        added_events: usize,
        route_started: Instant,
        navigation_ms: u64,
        extraction_ms: u64,
        collect_ms: u64,
        total_events: &[Event],
        odds: usize,
    ) {
        let live = total_events.iter().filter(|event| event.is_live).count();
        let prematch = total_events.len().saturating_sub(live);
        info!(
            phase = phase,
            url = url,
            sport = %fallback_sport,
            payload_items = payload_items,
            added_events = added_events,
            navigation_ms = navigation_ms,
            extraction_ms = extraction_ms,
            collect_ms = collect_ms,
            route_elapsed_ms = route_started.elapsed().as_millis() as u64,
            total = total_events.len(),
            live = live,
            prematch = prematch,
            odds = odds,
            "Winline: headless route processed"
        );
    }

    fn is_expensive_empty_route(metric: &HeadlessRouteMetric) -> bool {
        metric.expensive && metric.added_events == 0 && metric.status == "ok"
    }

    fn log_route_summary(
        phase: &'static str,
        metrics: &[HeadlessRouteMetric],
        started: Instant,
        total_events: &[Event],
        odds: usize,
    ) {
        if metrics.is_empty() {
            return;
        }

        let live = total_events.iter().filter(|event| event.is_live).count();
        let prematch = total_events.len().saturating_sub(live);
        let expensive_routes = metrics.iter().filter(|metric| metric.expensive).count();
        let empty_routes = metrics
            .iter()
            .filter(|metric| metric.added_events == 0 && metric.status == "ok")
            .count();
        let failed_routes = metrics
            .iter()
            .filter(|metric| metric.status != "ok")
            .count();

        if let Some(slowest) = metrics.iter().max_by_key(|metric| metric.total_ms) {
            info!(
                phase = phase,
                routes = metrics.len(),
                expensive_routes = expensive_routes,
                empty_routes = empty_routes,
                failed_routes = failed_routes,
                elapsed_ms = started.elapsed().as_millis() as u64,
                slowest_path = slowest.path.as_str(),
                slowest_metric_phase = slowest.phase,
                slowest_sport = %slowest.sport,
                slowest_status = slowest.status,
                slowest_total_ms = slowest.total_ms,
                slowest_navigation_ms = slowest.navigation_ms,
                slowest_extraction_ms = slowest.extraction_ms,
                slowest_collect_ms = slowest.collect_ms,
                slowest_payload_items = slowest.payload_items,
                slowest_added_events = slowest.added_events,
                total = total_events.len(),
                live = live,
                prematch = prematch,
                odds = odds,
                "Winline: headless phase summary"
            );
        }
    }

    fn fetch_headless_runtime_data_blocking(
        seed_paths: HeadlessSeedPaths,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let helper = HeadlessChromeHelper::new()?;
        let runtime_started = Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen = HashSet::new();
        let mut live_metrics = Vec::new();
        let mut prematch_metrics = Vec::new();

        let live_bootstrap_started = Instant::now();
        let live_navigation_started = Instant::now();
        let live_tab = helper.navigate_and_wait_with_timeout(
            LIVE_URL,
            HEADLESS_WAIT_MS,
            HEADLESS_NAVIGATION_TIMEOUT_MS,
        )?;
        let live_navigation_ms = live_navigation_started.elapsed().as_millis() as u64;
        let live_extract_started = Instant::now();
        let live_payload = Self::extract_from_tab(&live_tab, LIVE_URL);
        let live_extraction_ms = live_extract_started.elapsed().as_millis() as u64;
        debug!(
            url = LIVE_URL,
            items = live_payload.len(),
            "Winline: headless live payload extracted"
        );
        let live_payload_items = live_payload.len();
        let live_collect_started = Instant::now();
        let live_added_events = Self::collect_headless_page(
            &mut all_events,
            &mut all_odds,
            &mut seen,
            live_payload,
            Sport::Other,
            true,
            LIVE_URL,
        );
        let live_collect_ms = live_collect_started.elapsed().as_millis() as u64;
        Self::log_phase_result(
            "live-bootstrap",
            LIVE_URL,
            live_payload_items,
            live_added_events,
            live_bootstrap_started,
            live_navigation_ms,
            live_extraction_ms,
            live_collect_ms,
            &all_events,
            all_odds.len(),
        );

        let live_paths = Self::prioritized_headless_paths(seed_paths.live, true);
        let live_fanout_started = Instant::now();
        let mut visited_live_paths = HashSet::new();
        visited_live_paths.insert(normalize_winline_path(LIVE_URL));
        let mut live_empty_streak = 0;
        let mut live_expensive_empty_streak = 0;
        for path in live_paths.into_iter().take(HEADLESS_MAX_LIVE_SPORT_PAGES) {
            if Self::target_counts_reached(&all_events) {
                break;
            }

            if live_empty_streak >= HEADLESS_LIVE_EMPTY_STREAK_LIMIT {
                debug!(
                    empty_streak = live_empty_streak,
                    limit = HEADLESS_LIVE_EMPTY_STREAK_LIMIT,
                    "Winline: stopping live sport navigation after empty streak"
                );
                break;
            }

            if Self::fanout_budget_exhausted(live_fanout_started, HEADLESS_LIVE_FANOUT_BUDGET_MS) {
                warn!(
                    phase = "live",
                    fanout_budget_ms = HEADLESS_LIVE_FANOUT_BUDGET_MS,
                    fanout_elapsed_ms = live_fanout_started.elapsed().as_millis() as u64,
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after live phase budget exhaustion"
                );
                break;
            }

            if Self::runtime_budget_exhausted(runtime_started) {
                Self::log_route_budget_break(
                    "live",
                    &path,
                    runtime_started,
                    &all_events,
                    all_odds.len(),
                    HEADLESS_ROUTE_GUARD_MS,
                    "budget_exhausted",
                );
                break;
            }

            if !Self::runtime_budget_allows_next_route(runtime_started) {
                Self::log_route_budget_break(
                    "live",
                    &path,
                    runtime_started,
                    &all_events,
                    all_odds.len(),
                    HEADLESS_ROUTE_GUARD_MS,
                    "route_guard",
                );
                break;
            }

            let normalized_path = normalize_winline_path(&path);
            if !visited_live_paths.insert(normalized_path.clone()) {
                continue;
            }

            let url = format!("{}{}", BASE_URL, normalized_path);
            let fallback_sport = sport_from_winline_hint(&normalized_path, Sport::Other);
            let route_started = Instant::now();
            let navigation_started = Instant::now();
            let tab = match helper.navigate_and_wait_with_timeout(
                &url,
                HEADLESS_WAIT_MS,
                HEADLESS_NAVIGATION_TIMEOUT_MS,
            ) {
                Ok(tab) => tab,
                Err(error) => {
                    let navigation_ms = navigation_started.elapsed().as_millis() as u64;
                    live_metrics.push(HeadlessRouteMetric {
                        phase: "live",
                        path: normalized_path.clone(),
                        sport: fallback_sport,
                        status: "navigation_failed",
                        payload_items: 0,
                        added_events: 0,
                        navigation_ms,
                        extraction_ms: 0,
                        collect_ms: 0,
                        total_ms: route_started.elapsed().as_millis() as u64,
                        expensive: navigation_ms >= HEADLESS_EXPENSIVE_ROUTE_MS,
                    });
                    debug!(
                        url = url.as_str(),
                        error = %error,
                        route_elapsed_ms = route_started.elapsed().as_millis() as u64,
                        "Winline: live sport navigation failed"
                    );
                    continue;
                }
            };
            let navigation_ms = navigation_started.elapsed().as_millis() as u64;

            let extract_started = Instant::now();
            let payload = Self::extract_from_tab(&tab, &url);
            let extraction_ms = extract_started.elapsed().as_millis() as u64;
            let payload_items = payload.len();
            let collect_started = Instant::now();
            let added_events = Self::collect_headless_page(
                &mut all_events,
                &mut all_odds,
                &mut seen,
                payload,
                fallback_sport,
                true,
                &url,
            );
            let collect_ms = collect_started.elapsed().as_millis() as u64;
            let total_ms = route_started.elapsed().as_millis() as u64;
            let metric = HeadlessRouteMetric {
                phase: "live",
                path: normalized_path.clone(),
                sport: fallback_sport,
                status: "ok",
                payload_items,
                added_events,
                navigation_ms,
                extraction_ms,
                collect_ms,
                total_ms,
                expensive: total_ms >= HEADLESS_EXPENSIVE_ROUTE_MS,
            };
            Self::log_route_result(
                "live",
                &url,
                fallback_sport,
                payload_items,
                added_events,
                route_started,
                navigation_ms,
                extraction_ms,
                collect_ms,
                &all_events,
                all_odds.len(),
            );
            live_expensive_empty_streak = if Self::is_expensive_empty_route(&metric) {
                live_expensive_empty_streak + 1
            } else {
                0
            };
            live_metrics.push(metric);
            live_empty_streak = if added_events == 0 {
                live_empty_streak + 1
            } else {
                0
            };

            if live_expensive_empty_streak >= HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT {
                warn!(
                    phase = "live",
                    expensive_empty_streak = live_expensive_empty_streak,
                    expensive_route_ms = HEADLESS_EXPENSIVE_ROUTE_MS,
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after consecutive expensive empty routes"
                );
                break;
            }
        }

        Self::log_route_summary(
            "live",
            &live_metrics,
            runtime_started,
            &all_events,
            all_odds.len(),
        );

        let discovery_started = Instant::now();
        let discovery_navigation_started = Instant::now();
        let discovery_tab = helper.navigate_and_wait_with_timeout(
            DISCOVERY_URL,
            HEADLESS_WAIT_MS,
            HEADLESS_NAVIGATION_TIMEOUT_MS,
        )?;
        let discovery_navigation_ms = discovery_navigation_started.elapsed().as_millis() as u64;
        let discovery_extract_started = Instant::now();
        let discovered_paths = Self::extract_discovered_sport_links(&discovery_tab);
        let discovery_payload = Self::extract_from_tab(&discovery_tab, DISCOVERY_URL);
        let discovery_extraction_ms = discovery_extract_started.elapsed().as_millis() as u64;
        debug!(
            url = DISCOVERY_URL,
            items = discovery_payload.len(),
            discovered = discovered_paths.len(),
            "Winline: headless sport discovery extracted"
        );
        let discovery_payload_items = discovery_payload.len();
        let discovery_collect_started = Instant::now();
        let discovery_added_events = Self::collect_headless_page(
            &mut all_events,
            &mut all_odds,
            &mut seen,
            discovery_payload,
            Sport::Football,
            false,
            DISCOVERY_URL,
        );
        let discovery_collect_ms = discovery_collect_started.elapsed().as_millis() as u64;
        Self::log_phase_result(
            "prematch-bootstrap",
            DISCOVERY_URL,
            discovery_payload_items,
            discovery_added_events,
            discovery_started,
            discovery_navigation_ms,
            discovery_extraction_ms,
            discovery_collect_ms,
            &all_events,
            all_odds.len(),
        );

        let mut visited_paths = HashSet::new();
        visited_paths.insert(normalize_winline_path(DISCOVERY_URL));

        let mut prematch_paths = seed_paths.prematch;
        for path in discovered_paths {
            let normalized = normalize_winline_path(&path);
            if !prematch_paths.iter().any(|item| item == &normalized) {
                prematch_paths.push(normalized);
            }
        }

        let prematch_paths = Self::prioritized_headless_paths(prematch_paths, false);
        let prematch_fanout_started = Instant::now();
        let mut prematch_empty_streak = 0;
        let mut prematch_expensive_empty_streak = 0;

        for path in prematch_paths.into_iter().take(HEADLESS_MAX_PREMATCH_PAGES) {
            if Self::target_counts_reached(&all_events) {
                break;
            }

            if prematch_empty_streak >= HEADLESS_PREMATCH_EMPTY_STREAK_LIMIT {
                debug!(
                    empty_streak = prematch_empty_streak,
                    limit = HEADLESS_PREMATCH_EMPTY_STREAK_LIMIT,
                    "Winline: stopping prematch sport navigation after empty streak"
                );
                break;
            }

            if Self::fanout_budget_exhausted(
                prematch_fanout_started,
                HEADLESS_PREMATCH_FANOUT_BUDGET_MS,
            ) {
                warn!(
                    phase = "prematch",
                    fanout_budget_ms = HEADLESS_PREMATCH_FANOUT_BUDGET_MS,
                    fanout_elapsed_ms = prematch_fanout_started.elapsed().as_millis() as u64,
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after prematch phase budget exhaustion"
                );
                break;
            }

            if Self::runtime_budget_exhausted(runtime_started) {
                Self::log_route_budget_break(
                    "prematch",
                    &path,
                    runtime_started,
                    &all_events,
                    all_odds.len(),
                    HEADLESS_ROUTE_GUARD_MS,
                    "budget_exhausted",
                );
                break;
            }

            if !Self::runtime_budget_allows_next_route(runtime_started) {
                Self::log_route_budget_break(
                    "prematch",
                    &path,
                    runtime_started,
                    &all_events,
                    all_odds.len(),
                    HEADLESS_ROUTE_GUARD_MS,
                    "route_guard",
                );
                break;
            }

            let normalized_path = normalize_winline_path(&path);
            if !visited_paths.insert(normalized_path.clone()) {
                continue;
            }

            let url = format!("{}{}", BASE_URL, normalized_path);
            let fallback_sport = sport_from_winline_hint(&normalized_path, Sport::Other);
            let route_started = Instant::now();
            let navigation_started = Instant::now();
            let tab = match helper.navigate_and_wait_with_timeout(
                &url,
                HEADLESS_WAIT_MS,
                HEADLESS_NAVIGATION_TIMEOUT_MS,
            ) {
                Ok(tab) => tab,
                Err(error) => {
                    let navigation_ms = navigation_started.elapsed().as_millis() as u64;
                    prematch_metrics.push(HeadlessRouteMetric {
                        phase: "prematch",
                        path: normalized_path.clone(),
                        sport: fallback_sport,
                        status: "navigation_failed",
                        payload_items: 0,
                        added_events: 0,
                        navigation_ms,
                        extraction_ms: 0,
                        collect_ms: 0,
                        total_ms: route_started.elapsed().as_millis() as u64,
                        expensive: navigation_ms >= HEADLESS_EXPENSIVE_ROUTE_MS,
                    });
                    debug!(
                        url = url.as_str(),
                        error = %error,
                        route_elapsed_ms = route_started.elapsed().as_millis() as u64,
                        "Winline: discovered sport navigation failed"
                    );
                    continue;
                }
            };
            let navigation_ms = navigation_started.elapsed().as_millis() as u64;

            let extract_started = Instant::now();
            let payload = Self::extract_headless_payload(&tab);
            let extraction_ms = extract_started.elapsed().as_millis() as u64;
            let payload_items = payload.len();
            let collect_started = Instant::now();
            let added_events = Self::collect_headless_page(
                &mut all_events,
                &mut all_odds,
                &mut seen,
                payload,
                fallback_sport,
                false,
                &url,
            );
            let collect_ms = collect_started.elapsed().as_millis() as u64;
            let total_ms = route_started.elapsed().as_millis() as u64;
            let metric = HeadlessRouteMetric {
                phase: "prematch",
                path: normalized_path.clone(),
                sport: fallback_sport,
                status: "ok",
                payload_items,
                added_events,
                navigation_ms,
                extraction_ms,
                collect_ms,
                total_ms,
                expensive: total_ms >= HEADLESS_EXPENSIVE_ROUTE_MS,
            };
            Self::log_route_result(
                "prematch",
                &url,
                fallback_sport,
                payload_items,
                added_events,
                route_started,
                navigation_ms,
                extraction_ms,
                collect_ms,
                &all_events,
                all_odds.len(),
            );
            prematch_expensive_empty_streak = if Self::is_expensive_empty_route(&metric) {
                prematch_expensive_empty_streak + 1
            } else {
                0
            };
            prematch_metrics.push(metric);
            prematch_empty_streak = if added_events == 0 {
                prematch_empty_streak + 1
            } else {
                0
            };

            if prematch_expensive_empty_streak >= HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT {
                warn!(
                    phase = "prematch",
                    expensive_empty_streak = prematch_expensive_empty_streak,
                    expensive_route_ms = HEADLESS_EXPENSIVE_ROUTE_MS,
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after consecutive expensive empty routes"
                );
                break;
            }
        }

        Self::log_route_summary(
            "prematch",
            &prematch_metrics,
            runtime_started,
            &all_events,
            all_odds.len(),
        );

        Ok((all_events, all_odds))
    }

    async fn fetch_via_headless(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let seed_paths = self.fetch_seed_paths().await;
        match tokio::time::timeout(
            Duration::from_millis(HEADLESS_RUNTIME_BUDGET_MS + 5_000),
            tokio::task::spawn_blocking(move || {
                Self::fetch_headless_runtime_data_blocking(seed_paths)
            }),
        )
        .await
        {
            Ok(joined) => joined
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?,
            Err(_) => Err(format!(
                "headless runtime budget exceeded after {}ms",
                HEADLESS_RUNTIME_BUDGET_MS + 5_000
            )
            .into()),
        }
    }

    fn parse_item_as_event(
        item: &serde_json::Value,
        fallback_sport: Sport,
        fallback_live: bool,
        probe_path: &str,
    ) -> Option<(Event, Vec<Odd>)> {
        let name = item
            .get("name")
            .or_else(|| item.get("title"))
            .or_else(|| item.get("eventName"))
            .and_then(|value| value.as_str())
            .unwrap_or("");

        let (home_team, away_team) = split_event_name(name).or_else(|| {
            let home = item
                .get("home")
                .or_else(|| item.get("team1"))
                .or_else(|| item.get("homeTeam"))
                .and_then(|value| value.as_str())?;
            let away = item
                .get("away")
                .or_else(|| item.get("team2"))
                .or_else(|| item.get("awayTeam"))
                .and_then(|value| value.as_str())?;

            (is_valid_name(home) && is_valid_name(away))
                .then(|| (home.to_string(), away.to_string()))
        })?;

        let raw_id = item
            .get("id")
            .and_then(|value| value.as_u64())
            .map(|value| value.to_string())
            .unwrap_or_else(|| {
                format!(
                    "{}-{}",
                    home_team.replace(' ', "_"),
                    away_team.replace(' ', "_")
                )
            });
        let event_id = format!("winline-{}", raw_id);
        let league = item
            .get("champ")
            .or_else(|| item.get("league"))
            .or_else(|| item.get("tournament"))
            .or_else(|| item.get("tournamentName"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let is_live = item
            .get("isLive")
            .or_else(|| item.get("live"))
            .or_else(|| item.get("is_live"))
            .and_then(parse_truthy_bool)
            .unwrap_or(fallback_live);
        let start_time = item
            .get("startTime")
            .or_else(|| item.get("start_time"))
            .or_else(|| item.get("date"))
            .and_then(|value| {
                value
                    .as_i64()
                    .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
                    .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
            });

        let event = Event {
            id: event_id.clone(),
            sport: fallback_sport,
            league,
            home_team,
            away_team,
            start_time,
            is_live,
            bookmaker_slug: "winline".to_string(),
            raw_url: Some(format!("{}{}", BASE_URL, probe_path)),
            extra: HashMap::new(),
        };

        let now = Utc::now();
        let mut odds = Vec::new();
        let k1 = Self::extract_odds_field(item, &["k1", "w1", "odds1", "home_odds"]);
        let kx = Self::extract_odds_field(item, &["kx", "wx", "oddsx", "draw_odds"]);
        let k2 = Self::extract_odds_field(item, &["k2", "w2", "odds2", "away_odds"]);

        if let Some(value) = k1 {
            odds.push(Odd {
                id: format!("{}-1", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: value,
                odds_type: OddsType::Home,
                line: None,
                timestamp: now,
            });
        }
        if let Some(value) = kx {
            odds.push(Odd {
                id: format!("{}-X", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "X".into(),
                odds: value,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: now,
            });
        }
        if let Some(value) = k2 {
            odds.push(Odd {
                id: format!("{}-2", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: value,
                odds_type: OddsType::Away,
                line: None,
                timestamp: now,
            });
        }

        Some((event, odds))
    }

    fn parse_json_blob(
        value: &serde_json::Value,
        fallback_sport: Sport,
        fallback_live: bool,
        probe_path: &str,
    ) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut seen = HashSet::new();

        let items = if let Some(array) = value.as_array() {
            array.iter().collect::<Vec<_>>()
        } else if let Some(array) = value
            .get("events")
            .or_else(|| value.get("matches"))
            .or_else(|| value.get("items"))
            .or_else(|| value.get("data"))
            .and_then(|nested| nested.as_array())
        {
            array.iter().collect::<Vec<_>>()
        } else {
            return (Vec::new(), Vec::new());
        };

        for item in items {
            if let Some((event, mut event_odds)) =
                Self::parse_item_as_event(item, fallback_sport, fallback_live, probe_path)
            {
                if seen.insert(event.id.clone()) {
                    events.push(event);
                    odds.append(&mut event_odds);
                }
            }
        }

        (events, odds)
    }

    async fn fetch_from_probe(
        &self,
        probe: HtmlProbe,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}{}", BASE_URL, probe.path);
        debug!(url = url.as_str(), "Winline: probing public page");

        let response = self
            .client
            .get(&url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .send()
            .await?;

        if !response.status().is_success() {
            debug!(status = %response.status(), url = url.as_str(), "Winline: page probe failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let html = response.text().await?;
        let bootstrap_hints = Self::extract_bootstrap_hints_from_html(&html);
        let json_candidates = Self::extract_json_from_html(&html);
        debug!(
            url = url.as_str(),
            candidates = json_candidates.len(),
            bootstrap_scripts = bootstrap_hints.script_sources.len(),
            has_webscript = bootstrap_hints.has_webscript,
            has_main_bundle = bootstrap_hints.has_main_bundle,
            has_runtime_bundle = bootstrap_hints.has_runtime_bundle,
            "Winline: extracted HTML JSON candidates"
        );

        for candidate in json_candidates {
            let parsed = match serde_json::from_str::<serde_json::Value>(&candidate) {
                Ok(parsed) => parsed,
                Err(_) => continue,
            };

            let (events, odds) =
                Self::parse_json_blob(&parsed, probe.sport, probe.is_live, probe.path);
            if !events.is_empty() {
                info!(
                    url = url.as_str(),
                    events = events.len(),
                    odds = odds.len(),
                    "Winline: found structured events in HTML payload"
                );
                return Ok((events, odds));
            }
        }

        Ok((Vec::new(), Vec::new()))
    }

    fn extract_bootstrap_hints_from_html(html: &str) -> BootstrapHints {
        let mut hints = BootstrapHints::default();
        let mut cursor = 0;

        while let Some(script_start) = html[cursor..].find("<script") {
            let absolute_start = cursor + script_start;
            let Some(tag_end_rel) = html[absolute_start..].find('>') else {
                break;
            };
            let tag = &html[absolute_start..absolute_start + tag_end_rel];
            if let Some(src) = Self::extract_attr_value(tag, "src") {
                let is_new = !hints.script_sources.iter().any(|item| item == &src);
                if is_new {
                    hints.has_webscript |= src.contains(BOOTSTRAP_WEBSCRIPT_PATH);
                    hints.has_main_bundle |= src.contains("main.") && src.ends_with(".js");
                    hints.has_runtime_bundle |= src.contains("runtime.") && src.ends_with(".js");
                    hints.script_sources.push(src);
                }
            }
            cursor = absolute_start + tag_end_rel + 1;
        }

        hints
    }

    fn extract_attr_value(tag: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr_name);
        let start = tag.find(&pattern)? + pattern.len();
        let rest = &tag[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn extract_json_from_html(html: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        let prefixes = [
            "window.__INITIAL_STATE__=",
            "window.__INITIAL_STATE__ =",
            "window.__DATA__=",
            "window.__DATA__ =",
            "window.__STATE__=",
            "window.__STATE__ =",
            "window.__PRELOADED_STATE__=",
            "window.__PRELOADED_STATE__ =",
        ];

        for prefix in prefixes {
            if let Some(start) = html.find(prefix) {
                let after = &html[start + prefix.len()..];
                if let Some(json) = Self::extract_balanced_json(after) {
                    candidates.push(json);
                }
            }
        }

        let mut offset = 0;
        while let Some(tag_start) = html[offset..].find("<script type=\"application/json\"") {
            let absolute_start = offset + tag_start;
            let Some(content_start) = html[absolute_start..].find('>') else {
                break;
            };
            let content_offset = absolute_start + content_start + 1;
            let Some(tag_end) = html[content_offset..].find("</script>") else {
                break;
            };

            let json = html[content_offset..content_offset + tag_end].trim();
            if json.starts_with('{') || json.starts_with('[') {
                candidates.push(json.to_string());
            }

            offset = content_offset + tag_end + 9;
        }

        candidates
    }

    fn extract_balanced_json(source: &str) -> Option<String> {
        let source = source.trim_start();
        let (open, close) = if source.starts_with('{') {
            ('{', '}')
        } else if source.starts_with('[') {
            ('[', ']')
        } else {
            return None;
        };

        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for (index, ch) in source.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                current if !in_string && current == open => depth += 1,
                current if !in_string && current == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(source[..index + 1].to_string());
                    }
                }
                _ => {}
            }
        }

        None
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            headless_live_url = LIVE_URL,
            headless_discovery_url = DISCOVERY_URL,
            probes = HTML_PROBES.len(),
            "Winline: collecting runtime data with headless DOM extraction and HTML fallback"
        );
        info!(
            ws_url = DISCOVERED_WS_URL,
            ws_init = ?DISCOVERED_WS_INIT_COMMANDS,
            event_filters = ?DISCOVERED_EVENT_FILTER_HINTS,
            line_commands = ?DISCOVERED_LINE_COMMAND_HINTS,
            "Winline: bootstrap investigation found websocket transport clues, but protocol remains unvalidated"
        );

        let mut runtime_events = Vec::new();
        let mut runtime_odds = Vec::new();
        let mut seen_runtime_event_ids = HashSet::new();
        let mut skip_playwright_fallback = false;

        match self.fetch_via_headless().await {
            Ok((events, odds)) if !events.is_empty() => {
                let live_count = events.iter().filter(|event| event.is_live).count();
                let prematch_count = events.len().saturating_sub(live_count);
                info!(
                    total = events.len(),
                    live = live_count,
                    prematch = prematch_count,
                    odds = odds.len(),
                    "Winline: runtime data collected from headless DOM extraction"
                );
                Self::merge_runtime_results(
                    &mut runtime_events,
                    &mut runtime_odds,
                    &mut seen_runtime_event_ids,
                    events,
                    odds,
                );
                if Self::target_counts_reached(&runtime_events) {
                    return Ok((runtime_events, runtime_odds));
                }
                warn!(
                    live = live_count,
                    prematch = prematch_count,
                    target_live = TARGET_LIVE_EVENTS,
                    target_prematch = TARGET_PREMATCH_EVENTS,
                    "Winline: headless DOM extraction stayed below KPI, but returning early to avoid Playwright timeout risk"
                );
                return Ok((runtime_events, runtime_odds));
            }
            Ok((_, _)) => {
                warn!("Winline: headless DOM extraction returned no events, falling back to Playwright DOM extraction");
            }
            Err(error) => {
                let error_text = error.to_string();
                skip_playwright_fallback = error_text.contains("headless runtime budget exceeded");
                warn!(error = %error, "Winline: headless DOM extraction failed, falling back to Playwright DOM extraction");
            }
        }

        if Self::should_run_playwright_fallback(&runtime_events) && !skip_playwright_fallback {
            match self.fetch_via_playwright().await {
                Ok((events, odds)) if !events.is_empty() => {
                    let live_count = events.iter().filter(|event| event.is_live).count();
                    let prematch_count = events.len().saturating_sub(live_count);
                    info!(
                        total = events.len(),
                        live = live_count,
                        prematch = prematch_count,
                        odds = odds.len(),
                        "Winline: runtime data collected from Playwright DOM extraction"
                    );
                    Self::merge_runtime_results(
                        &mut runtime_events,
                        &mut runtime_odds,
                        &mut seen_runtime_event_ids,
                        events,
                        odds,
                    );
                    if !runtime_events.is_empty() {
                        return Ok((runtime_events, runtime_odds));
                    }
                }
                Ok((_, _)) => {
                    warn!("Winline: Playwright DOM extraction returned no events, falling back to HTML probes");
                }
                Err(error) => {
                    warn!(error = %error, "Winline: Playwright DOM extraction failed, falling back to HTML probes");
                }
            }
        } else if skip_playwright_fallback {
            warn!(
                budget_ms = HEADLESS_RUNTIME_BUDGET_MS + 5_000,
                "Winline: skipping Playwright fallback because headless stalled past the internal runtime budget"
            );
        }

        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen = HashSet::new();

        for probe in HTML_PROBES {
            match self.fetch_from_probe(*probe).await {
                Ok((events, odds)) => {
                    for event in events {
                        if seen.insert(event.id.clone()) {
                            all_events.push(event);
                        }
                    }
                    all_odds.extend(odds);
                }
                Err(error) => {
                    debug!(path = probe.path, error = %error, "Winline: HTML probe failed");
                }
            }
        }

        let live_count = all_events.iter().filter(|event| event.is_live).count();
        let prematch_count = all_events.len().saturating_sub(live_count);

        if all_events.is_empty() {
            warn!(
                probes = HTML_PROBES.len(),
                "Winline: no enumerable public event feed found; returning empty runtime result"
            );
        } else {
            info!(
                total = all_events.len(),
                live = live_count,
                prematch = prematch_count,
                odds = all_odds.len(),
                "Winline: runtime data collected from HTML-exposed payloads"
            );
        }

        Ok((all_events, all_odds))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HeadlessRouteMetric, WinlineParser, BOOTSTRAP_WEBSCRIPT_PATH,
        HEADLESS_EXPENSIVE_ROUTE_MS, HEADLESS_LIVE_FANOUT_BUDGET_MS,
        HEADLESS_NAVIGATION_TIMEOUT_MS, HEADLESS_PREMATCH_FANOUT_BUDGET_MS,
        HEADLESS_ROUTE_GUARD_MS, HEADLESS_RUNTIME_BUDGET_MS, LIVE_URL,
    };
    use shared::Sport;
    use std::time::{Duration, Instant};

    #[test]
    fn extracts_bootstrap_script_hints_from_html() {
        let html = r#"
            <html>
                <body>
                    <script src="https://winline.ru/api/v2/webscript.js" async></script>
                    <script src="runtime.5e737b5dc3d71c9c.js" type="module"></script>
                    <script src="main.4c7c8cc49b07bee6.js" type="module"></script>
                </body>
            </html>
        "#;

        let hints = WinlineParser::extract_bootstrap_hints_from_html(html);
        assert_eq!(hints.script_sources.len(), 3);
        assert!(hints
            .script_sources
            .iter()
            .any(|src| src.contains(BOOTSTRAP_WEBSCRIPT_PATH)));
        assert!(hints.has_webscript);
        assert!(hints.has_main_bundle);
        assert!(hints.has_runtime_bundle);
    }

    #[test]
    fn extracts_sport_seed_links_from_html() {
        let html = r#"
            <html>
                <body>
                    <a href="/stavki/sport/futbol/">Football</a>
                    <a href="https://winline.ru/stavki/sport/tennis/">Tennis</a>
                    <a href="/live/sport/futbol">Ignored live</a>
                    <a href="/stavki/sport/futbol/">Duplicate</a>
                </body>
            </html>
        "#;

        let paths = WinlineParser::extract_sport_links_from_html(html);
        assert_eq!(paths, vec!["/stavki/sport/futbol", "/stavki/sport/tennis"]);
    }

    #[test]
    fn extracts_balanced_json_payload() {
        let source = r#"   {"events":[{"id":1}],"ok":true};window.next=1;"#;
        let json = WinlineParser::extract_balanced_json(source).expect("balanced json");
        assert_eq!(json, r#"{"events":[{"id":1}],"ok":true}"#);
    }

    #[test]
    fn parses_headless_three_way_payload() {
        let payload = serde_json::json!({
            "home": "Spartak Moscow",
            "away": "Zenit",
            "tournament": "Premier League",
            "eventId": "12345",
            "href": "/stavki/event/12345",
            "odds": [2.15, 3.4, 3.05]
        });

        let (event, odds) = WinlineParser::parse_headless_item(
            &payload,
            Sport::Football,
            true,
            "https://winline.ru/live/football",
        )
        .expect("headless item");

        assert_eq!(event.id, "winline-12345");
        assert_eq!(event.sport, Sport::Football);
        assert!(event.is_live);
        assert_eq!(
            event.raw_url.as_deref(),
            Some("https://winline.ru/stavki/event/12345")
        );
        assert_eq!(odds.len(), 3);
        assert_eq!(odds[0].selection, "1");
        assert_eq!(odds[1].selection, "X");
        assert_eq!(odds[2].selection, "2");
    }

    #[test]
    fn parses_headless_two_way_total_payload() {
        let payload = serde_json::json!({
            "home": "Djokovic",
            "away": "Medvedev",
            "tournament": "ATP Test",
            "odds": [1.87, 1.93],
            "totalLine": "22.5"
        });

        let (event, odds) = WinlineParser::parse_headless_item(
            &payload,
            Sport::Tennis,
            false,
            "https://winline.ru/tennis",
        )
        .expect("headless total item");

        assert_eq!(event.sport, Sport::Tennis);
        assert!(!event.is_live);
        assert_eq!(odds.len(), 2);
        assert_eq!(odds[0].market, "Total");
        assert_eq!(odds[0].line, Some(22.5));
        assert_eq!(odds[1].selection, "Under");
    }

    #[test]
    fn maps_winline_sport_hints_from_slugs() {
        assert_eq!(
            super::sport_from_winline_hint("/stavki/sport/nastolijnyj_tennis", Sport::Other),
            Sport::TableTennis
        );
        assert_eq!(
            super::sport_from_winline_hint("/stavki/sport/xokkej", Sport::Other),
            Sport::Hockey
        );
        assert_eq!(
            super::sport_from_winline_hint("киберспорт", Sport::Other),
            Sport::Esports
        );
    }

    #[test]
    fn prioritizes_headless_prematch_paths_before_discovered_tail() {
        let ordered = WinlineParser::prioritized_headless_paths(
            vec![
                "/stavki/sport/snuker".into(),
                "/stavki/sport/unknown".into(),
                "/stavki/sport/futbol".into(),
            ],
            false,
        );

        assert_eq!(ordered[0], "/stavki/sport/nastolijnyj_tennis");
        assert_eq!(ordered[1], "/stavki/sport/bejsbol");
        assert!(ordered.iter().any(|path| path == "/stavki/sport/unknown"));
        assert_eq!(
            ordered
                .iter()
                .filter(|path| path.as_str() == "/stavki/sport/futbol")
                .count(),
            1
        );
    }

    #[test]
    fn prioritizes_headless_live_paths_with_live_prefix() {
        let ordered = WinlineParser::prioritized_headless_paths(
            vec!["/live/sport/tennis".into(), "/live/sport/futbol".into()],
            true,
        );

        assert_eq!(ordered[0], "/live/sport/nastolijnyj_tennis");
        assert!(ordered.iter().any(|path| path == "/live/sport/futbol"));
        assert_eq!(
            ordered
                .iter()
                .filter(|path| path.as_str() == "/live/sport/futbol")
                .count(),
            1
        );
    }

    #[test]
    fn prefers_item_specific_live_flag_and_sport_name() {
        let payload = serde_json::json!({
            "home": "Ak Bars",
            "away": "Dinamo Minsk",
            "league": "KHL",
            "eventId": "15553043",
            "href": "/stavki/event/15553043",
            "odds": [2.35, 4.5, 1.55],
            "isLive": true,
            "sportName": "хоккей"
        });

        let (event, _) =
            WinlineParser::parse_headless_item(&payload, Sport::Football, false, LIVE_URL)
                .expect("headless item");

        assert_eq!(event.sport, Sport::Hockey);
        assert!(event.is_live);
        assert_eq!(event.league, "KHL");
    }

    #[test]
    fn parses_truthy_live_flag_from_string_payload() {
        let payload = serde_json::json!({
            "home": "Rubin",
            "away": "Lokomotiv",
            "eventId": "42",
            "odds": [2.1, 3.2, 3.4],
            "isLive": "1"
        });

        let (event, _) =
            WinlineParser::parse_headless_item(&payload, Sport::Football, false, LIVE_URL)
                .expect("headless item");

        assert!(event.is_live);
    }

    #[test]
    fn parses_truthy_live_flag_from_numeric_payload() {
        let payload = serde_json::json!({
            "home": "Rubin",
            "away": "Lokomotiv",
            "eventId": "43",
            "odds": [2.1, 3.2, 3.4],
            "live": 0
        });

        let (event, _) =
            WinlineParser::parse_headless_item(&payload, Sport::Football, true, LIVE_URL)
                .expect("headless item");

        assert!(!event.is_live);
    }

    #[test]
    fn only_runs_playwright_fallback_when_headless_is_empty() {
        let populated = vec![shared::Event {
            id: "winline-1".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "winline".into(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        }];

        assert!(WinlineParser::should_run_playwright_fallback(&[]));
        assert!(!WinlineParser::should_run_playwright_fallback(&populated));
    }

    #[test]
    fn runtime_route_guard_requires_minimum_remaining_budget() {
        let started = Instant::now() - Duration::from_millis(HEADLESS_RUNTIME_BUDGET_MS - 6_500);
        assert!(!WinlineParser::runtime_budget_allows_next_route(started));

        let started = Instant::now() - Duration::from_millis(HEADLESS_RUNTIME_BUDGET_MS - 7_500);
        assert!(WinlineParser::runtime_budget_allows_next_route(started));
    }

    #[test]
    fn headless_navigation_timeout_stays_inside_route_guard() {
        assert!(HEADLESS_NAVIGATION_TIMEOUT_MS < HEADLESS_ROUTE_GUARD_MS);
    }

    #[test]
    fn fanout_phase_budgets_trip_before_global_runtime_budget() {
        let started = Instant::now() - Duration::from_millis(HEADLESS_LIVE_FANOUT_BUDGET_MS + 1);
        assert!(WinlineParser::fanout_budget_exhausted(
            started,
            HEADLESS_LIVE_FANOUT_BUDGET_MS
        ));

        let started =
            Instant::now() - Duration::from_millis(HEADLESS_PREMATCH_FANOUT_BUDGET_MS - 1);
        assert!(!WinlineParser::fanout_budget_exhausted(
            started,
            HEADLESS_PREMATCH_FANOUT_BUDGET_MS
        ));
    }

    #[test]
    fn flags_expensive_empty_routes_for_early_stop_logic() {
        let expensive_empty = HeadlessRouteMetric {
            phase: "live",
            path: "/live/sport/darts".into(),
            sport: Sport::Darts,
            status: "ok",
            payload_items: 0,
            added_events: 0,
            navigation_ms: 4_000,
            extraction_ms: 3_000,
            collect_ms: 0,
            total_ms: HEADLESS_EXPENSIVE_ROUTE_MS,
            expensive: true,
        };
        let expensive_non_empty = HeadlessRouteMetric {
            phase: "live",
            path: "/live/sport/futbol".into(),
            sport: Sport::Football,
            status: "ok",
            payload_items: 12,
            added_events: 3,
            navigation_ms: 4_000,
            extraction_ms: 3_000,
            collect_ms: 1,
            total_ms: HEADLESS_EXPENSIVE_ROUTE_MS,
            expensive: true,
        };

        assert!(WinlineParser::is_expensive_empty_route(&expensive_empty));
        assert!(!WinlineParser::is_expensive_empty_route(
            &expensive_non_empty
        ));
        assert_eq!(expensive_empty.phase, "live");
    }
}

#[async_trait]
impl BookmakerParser for WinlineParser {
    fn name(&self) -> &str {
        "Winline"
    }

    fn slug(&self) -> &str {
        "winline"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: fetching events from truthful public runtime path");
        let (events, _) = self.fetch_runtime_data().await?;
        info!(count = events.len(), "Winline: events fetched");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: fetching odds from truthful public runtime path");
        let (_, odds) = self.fetch_runtime_data().await?;
        info!(count = odds.len(), "Winline: odds fetched");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        info!("Winline: full fetch from truthful public runtime path");

        let (events, odds) = self.fetch_runtime_data().await?;
        let elapsed = started.elapsed().as_millis() as u64;

        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "Winline: fetch finished"
        );

        Ok(ParserResult::new("winline", events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
    }
}
