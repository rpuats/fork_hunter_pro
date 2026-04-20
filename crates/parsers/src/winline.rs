use crate::base::{BookmakerParser, ParserResult};
use crate::headless_helper::{is_valid_team_name, HeadlessChromeHelper, SCROLL_PAGE_BUDGET_MS};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage, DiagnosticSeverity};
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
const HEADLESS_HYDRATION_RETRY_DELAY_MS: u64 = 400;
const HEADLESS_HYDRATION_RETRY_ATTEMPTS: usize = 8;
const HEADLESS_HYDRATION_EARLY_DIAGNOSTIC_ATTEMPT: usize = 3;
const HEADLESS_STABLE_EMPTY_ROUTE_REPEAT_LIMIT: usize = 2;
const HEADLESS_BLOCKER_ROUTE_STREAK_LIMIT: usize = 2;
const HEADLESS_MAX_PREMATCH_PAGES: usize = 18;
const HEADLESS_MAX_LIVE_SPORT_PAGES: usize = 8;
const HEADLESS_PREMATCH_EMPTY_STREAK_LIMIT: usize = 6;
const HEADLESS_LIVE_EMPTY_STREAK_LIMIT: usize = 4;
const HEADLESS_LIVE_FANOUT_BUDGET_MS: u64 = 18_000;
const HEADLESS_PREMATCH_FANOUT_BUDGET_MS: u64 = 18_000;
const HEADLESS_RUNTIME_BUDGET_MS: u64 = 70_000;
const HEADLESS_OUTER_TIMEOUT_RESERVE_MS: u64 = 15_000;
const HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS: u64 =
    HEADLESS_RUNTIME_BUDGET_MS - HEADLESS_OUTER_TIMEOUT_RESERVE_MS;
const HEADLESS_ROUTE_GUARD_MS: u64 = HEADLESS_NAVIGATION_TIMEOUT_MS
    + HEADLESS_HYDRATION_RETRY_DELAY_MS * HEADLESS_HYDRATION_RETRY_ATTEMPTS as u64
    + SCROLL_PAGE_BUDGET_MS * HEADLESS_SCROLL_ROUNDS as u64;
const HEADLESS_NAVIGATION_TIMEOUT_MS: u64 = 6_000;
const HEADLESS_EXPENSIVE_ROUTE_MS: u64 = 12_000;
const HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT: usize = 2;
const TARGET_LIVE_EVENTS: usize = 150;
const TARGET_PREMATCH_EVENTS: usize = 3000;
const PLAYWRIGHT_WAIT_MS: u64 = 1_500;
const DISCOVERY_URL: &str = "https://winline.ru/stavki/sport/futbol/";
const DISCOVERY_FALLBACK_URL: &str = "https://winline.ru/football";
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

#[derive(Debug, Default)]
struct HeadlessTabExtraction {
    payload: Vec<serde_json::Value>,
    blocker_signal: Option<String>,
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
    const GENERIC_CARD_SELECTOR = [
        '.pinned-event',                // NEW - Primary selector for pinned/live events
        '.event-card',                  // NEW - Event card wrapper
        '.card',                        // Generic card (lower priority)
        '.ww-events-info',              // Alternative event container
        'ww-feature-block-event-dsk',   // Keep legacy fallback
        'ww-feature-event-mini-card-dsk', // Keep legacy fallback
        '.main-event',                  // Legacy fallback
        '[data-test*="event"]',
        '[data-testid*="event"]',
        '[class*="event-card"]',
        '[class*="match-card"]'
    ].join(', ');
    const GENERIC_ODDS_SELECTOR = [
        '.coefficient-button',          // PRIMARY - New class structure
        '.coefficient-button_fill',     // Fill variant
        '.button__coef-title',
        '.main-event__coeff',
        '[data-test*="coef"]',
        '[data-testid*="coef"]',
        '[class*="coef"]',
        '[class*="odd"]',
        'button[title]',
        'button span',
        '[role="button"]'
    ].join(', ');
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
    const extractNamesFromHydratedText = (scope) => {
        if (!scope) return [];

        const text = normalizeText(scope.innerText || scope.textContent || '');
        if (!text) return [];

        const lines = String(scope.innerText || scope.textContent || '')
            .split(/\n+/)
            .map((value) => normalizeText(value))
            .filter(Boolean)
            .slice(0, 18);
        for (const line of lines) {
            const split = splitMatchName(line);
            if (split && split.every(isValidName)) return split;
        }

        const filteredLines = lines.filter((line) => {
            if (!isValidName(line)) return false;
            if (parseOdds(line) !== null) return false;
            if (/^(?:live|1x2|match|game|set|period|тайм|матч|тотал|больше|меньше)$/i.test(line)) {
                return false;
            }
            if (/^\d+(?:[.,]\d+)?(?:\s*[-:+]\s*\d+(?:[.,]\d+)?)?$/.test(line)) return false;
            return true;
        });
        if (filteredLines.length >= 2) {
            return filteredLines.slice(0, 2);
        }

        const compactSplit = splitMatchName(text);
        return compactSplit && compactSplit.every(isValidName) ? compactSplit : [];
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
            '.pinned-event__team',              // NEW - Primary team selector
            '.pinned-event__match',             // NEW - Match info
            '.half__names .name',
            '.body-left__names .name',
            '.card__competitors .name',
            '.competitor__name',
            '[class*="competitor"] [class*="name"]',
            '[class*="team"] [class*="name"]',
            '[data-test*="team"]',
            '[data-testid*="team"]',
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

        return extractNamesFromHydratedText(scope);
    };
    const findHydratedEventRoot = (node) => {
        let current = node instanceof Element ? node : null;
        for (let depth = 0; current && depth < 8; depth += 1, current = current.parentElement) {
            const text = normalizeText(current.innerText || current.textContent || '');
            if (text.length < 12 || text.length > 800) continue;
            const names = extractNames(current);
            const marketSelection = pickOdds(collectMarkets(current));
            if (names.length >= 2 && marketSelection) return current;
        }

        return null;
    };
    const collectGenericMarkets = (root) => {
        const genericButtons = Array.from(root.querySelectorAll(GENERIC_ODDS_SELECTOR))
            .filter((node) => !node.querySelector?.(GENERIC_ODDS_SELECTOR))
            .map((button) => parseOdds(button.textContent || button.getAttribute?.('title') || ''))
            .filter((value) => value !== null)
            .slice(0, 3);
        if (genericButtons.length < 2) return [];
        return [{
            buttons: genericButtons,
            middle: textFrom(root.querySelector('.coefficient-middle, .coefficient-middle__selector, [class*="middle"]')),
            text: normalizeText(root.textContent || ''),
        }];
    };
    const collectMarkets = (root) => {
        // Try to find markets in WW-FEATURE-EVENT-MARKET-DSK web components (primary)
        const featureMarkets = Array.from(root.querySelectorAll('ww-feature-event-market-dsk')).map((market) => {
            const buttons = Array.from(market.querySelectorAll('.coefficient-button, .coefficient-button_fill, .button__coef-title, .main-event__coeff'))
                .map((button) => parseOdds(button.textContent || ''))
                .filter((value) => value !== null);
            return {
                buttons,
                middle: textFrom(market.querySelector('.coefficient-middle, .coefficient-middle__selector')),
                text: normalizeText(market.textContent || ''),
            };
        }).filter((market) => market.buttons.length >= 2);
        
        if (featureMarkets.length > 0) return featureMarkets;
        
        // Fallback: Try coefficient containers
        const coeffContainers = Array.from(root.querySelectorAll('.coeffs-wrapper, .card__coeffs, [class*="coefficients"]')).map((container) => {
            const buttons = Array.from(container.querySelectorAll('.coefficient-button, .coefficient-button_fill'))
                .map((button) => parseOdds(button.textContent || ''))
                .filter((value) => value !== null);
            return {
                buttons,
                middle: textFrom(container.querySelector('.coefficient-middle, .coefficient-middle__selector')),
                text: normalizeText(container.textContent || ''),
            };
        }).filter((market) => market.buttons.length >= 2);
        
        return coeffContainers.length > 0 ? coeffContainers : collectGenericMarkets(root);
    };
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

    // NEW: Search for pinned/live events first (most relevant)
    Array.from(document.querySelectorAll('.pinned-event')).forEach((card) => {
        try {
            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const eventId = hrefMatch ? hrefMatch[1] : '';
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;
            const cardText = normalizeText(card.textContent || '');

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card) || textFrom(card.querySelector('[class*="tournament"]')),
                eventId,
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: true,  // Pinned events are typically live
                sportName: sportName(card),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    // Legacy: Search for old ww-feature-block-event-dsk (fallback)
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

    // NEW: Search for .event-card (new event card structure)
    Array.from(document.querySelectorAll('.event-card:not(.pinned-event)')).forEach((card) => {
        try {
            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const eventId = hrefMatch ? hrefMatch[1] : '';
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card),
                eventId,
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

    // Legacy: Search for old ww-feature-event-mini-card-dsk and .main-event (fallback)
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
            const card = link.closest(GENERIC_CARD_SELECTOR) || link.parentElement;
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

    Array.from(document.querySelectorAll(GENERIC_CARD_SELECTOR)).slice(0, 250).forEach((card) => {
        try {
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;

            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const cardIdMatch = normalizeText(card.id || '').match(/eventId-(\d+)/);
            const eventId = hrefMatch ? hrefMatch[1] : (cardIdMatch ? cardIdMatch[1] : '');
            const cardText = normalizeText(card.textContent || '');

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card) || textFrom(card.querySelector('.main-event__tournament, [class*="tournament"], [class*="league"]')),
                eventId,
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: inferLive(card, cardText),
                sportName: sportName(card) || textFrom(card.querySelector('[class*="sport"]')),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    const hydratedRoots = new Set();
    Array.from(document.querySelectorAll(GENERIC_ODDS_SELECTOR)).slice(0, 400).forEach((button) => {
        try {
            const root = findHydratedEventRoot(button);
            if (root) hydratedRoots.add(root);
        } catch (_) {}
    });

    Array.from(hydratedRoots).forEach((card) => {
        try {
            const marketSelection = pickOdds(collectMarkets(card));
            if (!marketSelection) return;

            const nameEls = extractNames(card);
            if (nameEls.length < 2) return;

            const link = card.querySelector('a[href*="/stavki/event/"]');
            const href = link ? normalizeText(link.getAttribute('href') || '') : '';
            const hrefMatch = href.match(/\/stavki\/event\/(\d+)/);
            const cardIdMatch = normalizeText(card.id || '').match(/eventId-(\d+)/);
            const eventId = hrefMatch ? hrefMatch[1] : (cardIdMatch ? cardIdMatch[1] : '');
            const cardText = normalizeText(card.innerText || card.textContent || '');

            pushEvent({
                home: nameEls[0],
                away: nameEls[1],
                tournament: tournamentName(card) || textFrom(card.querySelector('.main-event__tournament, [class*="tournament"], [class*="league"], [class*="championship"]')),
                eventId,
                href,
                odds: marketSelection.odds,
                totalLine: marketSelection.totalLine,
                sourceUrl: window.location.href,
                isLive: inferLive(card, cardText),
                sportName: sportName(card) || textFrom(card.querySelector('[class*="sport"]')),
                sportSlug: pageSportSlug,
            });
        } catch (_) {}
    });

    return results;
})()"#;

const HEADLESS_DOM_DIAGNOSTICS_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    const GENERIC_EVENT_SELECTOR = 'ww-feature-block-event-dsk, ww-feature-event-mini-card-dsk, .main-event, .card, .event-card, [id^="eventId-"], [data-test*="event"], [data-testid*="event"], [class*="event-card"], [class*="match-card"]';
    const GENERIC_ODDS_SELECTOR = '.coefficient-button_fill, .button__coef-title, .main-event__coeff, [data-test*="coef"], [data-testid*="coef"], [class*="coef"], [class*="odd"], button[title], button span, [role="button"]';
    const count = (selector) => {
        try {
            return document.querySelectorAll(selector).length;
        } catch (_) {
            return 0;
        }
    };
    const firstCard = document.querySelector(GENERIC_EVENT_SELECTOR);
    const firstButton = document.querySelector('button, [role="button"]');
    const hydratedRoots = new Set();
    Array.from(document.querySelectorAll(GENERIC_ODDS_SELECTOR)).slice(0, 400).forEach((button) => {
        try {
            const root = button.closest(GENERIC_EVENT_SELECTOR);
            if (root) hydratedRoots.add(root);
        } catch (_) {}
    });
    const navigationEntry = (() => {
        try {
            const entries = performance.getEntriesByType('navigation') || [];
            const last = entries[entries.length - 1];
            if (!last) return null;
            return {
                type: String(last.type || ''),
                domContentLoadedMs: Math.round(Number(last.domContentLoadedEventEnd || 0)),
                loadMs: Math.round(Number(last.loadEventEnd || 0))
            };
        } catch (_) {
            return null;
        }
    })();
    return {
        url: window.location.href,
        readyState: document.readyState || '',
        route: {
            pathname: window.location.pathname || '',
            search: window.location.search || '',
            hash: window.location.hash || '',
            title: document.title || '',
            historyLength: Number(window.history?.length || 0),
            navigationEntry,
        },
        counts: {
            wwBlockEventCards: count('ww-feature-block-event-dsk'),
            wwMiniEventCards: count('ww-feature-event-mini-card-dsk'),
            wwEventMarkets: count('ww-feature-event-market-dsk'),
            mainEventCards: count('.main-event'),
            genericEventCards: count('.card, .event-card, [id^="eventId-"], [data-test*="event"], [data-testid*="event"], [class*="event-card"], [class*="match-card"]'),
            hydratedRoots: hydratedRoots.size,
            eventLinks: count('a[href*="/stavki/event/"]'),
            halfNameNodes: count('.half__names .name'),
            bodyLeftNameNodes: count('.body-left__names .name'),
            genericNameNodes: count('.card__competitors .name, .competitor__name, .name'),
            coefficientButtons: count('.coefficient-button_fill, .button__coef-title, .main-event__coeff'),
            genericCoefficientButtons: count('[data-test*="coef"], [data-testid*="coef"], [class*="coef"], [class*="odd"], [role="button"]'),
            buttonNodes: count('button, [role="button"]'),
            routeLinkNodes: count('a[href^="/live"], a[href^="/stavki/"]'),
            liveCards: count('.card--live'),
            genericLiveMarkers: count('[class*="live"], [data-test*="live"], [data-testid*="live"]'),
            shellNodes: count('ww-app-dsk, ww-feature-header-dsk, ww-feature-nav-bar-dsk, ww-feature-sport-menu-dsk, ww-feature-coupon-dsk'),
            couponNodes: count('ww-feature-coupon-dsk'),
            sportMenuNodes: count('ww-feature-sport-menu-dsk')
        },
        bodyTextLength: normalizeText(document.body?.innerText || document.body?.textContent || '').length,
        bodyTextSample: normalizeText(document.body?.innerText || document.body?.textContent || '').slice(0, 280),
        firstCardText: normalizeText(firstCard?.textContent || '').slice(0, 280),
        firstCardHtml: String(firstCard?.outerHTML || '').slice(0, 800),
        firstButtonText: normalizeText(firstButton?.textContent || '').slice(0, 160)
    };
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
    fn live_headless_variants(path: &str) -> Vec<String> {
        let normalized = normalize_winline_path(path);
        let slug = normalized
            .trim_end_matches('/')
            .strip_prefix("/stavki/sport/")
            .or_else(|| {
                normalized
                    .trim_end_matches('/')
                    .strip_prefix("/live/sport/")
            })
            .or_else(|| normalized.trim_end_matches('/').strip_prefix("/live/"));

        let mut variants = Vec::new();
        let mut seen = HashSet::new();
        let candidates = if let Some(slug) = slug {
            vec![format!("/live/{slug}"), format!("/live/sport/{slug}")]
        } else {
            vec![normalized]
        };

        for candidate in candidates {
            let normalized_candidate = normalize_winline_path(&candidate);
            if seen.insert(normalized_candidate.clone()) {
                variants.push(normalized_candidate);
            }
        }

        variants
    }

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

    fn extract_headless_dom_diagnostics(tab: &headless_chrome::Tab) -> Option<serde_json::Value> {
        let mut diagnostics = HeadlessChromeHelper::evaluate_json_with_retry(
            tab,
            HEADLESS_DOM_DIAGNOSTICS_JS,
            HEADLESS_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )?;

        if let Some(runtime_state) = HeadlessChromeHelper::capture_runtime_state(tab) {
            if let Some(object) = diagnostics.as_object_mut() {
                object.insert("runtime".to_string(), runtime_state);
            }
        }

        Some(diagnostics)
    }

    fn dom_diagnostics_count(value: &serde_json::Value, key: &str) -> usize {
        value
            .get("counts")
            .and_then(|counts| counts.get(key))
            .and_then(|count| count.as_u64())
            .unwrap_or_default() as usize
    }

    fn dom_diagnostics_nested_str<'a>(
        value: &'a serde_json::Value,
        parent: &str,
        key: &str,
    ) -> &'a str {
        value
            .get(parent)
            .and_then(|item| item.get(key))
            .and_then(|item| item.as_str())
            .unwrap_or_default()
    }

    fn dom_diagnostics_nested_u64(value: &serde_json::Value, parent: &str, key: &str) -> u64 {
        value
            .get(parent)
            .and_then(|item| item.get(key))
            .and_then(|item| item.as_u64())
            .unwrap_or_default()
    }

    fn classify_empty_payload_diagnostics(value: &serde_json::Value) -> &'static str {
        let event_cards = Self::dom_diagnostics_count(value, "wwBlockEventCards")
            + Self::dom_diagnostics_count(value, "wwMiniEventCards")
            + Self::dom_diagnostics_count(value, "mainEventCards")
            + Self::dom_diagnostics_count(value, "genericEventCards");
        let market_nodes = Self::dom_diagnostics_count(value, "wwEventMarkets");
        let coefficient_buttons = Self::dom_diagnostics_count(value, "coefficientButtons")
            + Self::dom_diagnostics_count(value, "genericCoefficientButtons");
        let hydrated_roots = Self::dom_diagnostics_count(value, "hydratedRoots");
        let shell_nodes = Self::dom_diagnostics_count(value, "shellNodes");
        let route_path = Self::dom_diagnostics_nested_str(value, "route", "pathname");

        if event_cards > 0 && market_nodes > 0 && coefficient_buttons > 0 {
            "cards_present_extract_empty"
        } else if hydrated_roots > 0 && coefficient_buttons > 0 {
            "hydrated_roots_extract_empty"
        } else if event_cards > 0 {
            "cards_present_incomplete_markets"
        } else if shell_nodes > 0
            && (route_path.starts_with("/live") || route_path.starts_with("/stavki/"))
        {
            "route_ready_shell_only"
        } else if shell_nodes > 0 {
            "shell_only_no_event_cards"
        } else {
            "no_known_winline_dom_nodes"
        }
    }

    fn dom_diagnostics_runtime_blocker_signal(value: &serde_json::Value) -> Option<String> {
        let runtime = value.get("runtime")?;
        let blocker = runtime.get("blocker")?;
        let kind = blocker
            .get("kind")
            .and_then(|item| item.as_str())
            .unwrap_or_default();
        if kind.is_empty() {
            return None;
        }

        let source = blocker
            .get("source")
            .and_then(|item| item.as_str())
            .filter(|item| !item.is_empty())
            .unwrap_or("runtime");
        let matched_text = blocker
            .get("matchedText")
            .and_then(|item| item.as_str())
            .filter(|item| !item.is_empty())
            .unwrap_or("-");
        Some(format!(
            "blocker={}@{}:{}",
            kind,
            source,
            matched_text.replace('|', "/")
        ))
    }

    fn format_empty_payload_diagnostic(value: &serde_json::Value) -> String {
        let status = Self::classify_empty_payload_diagnostics(value);
        let route = Self::dom_diagnostics_nested_str(value, "route", "pathname");
        let ready_state = value
            .get("readyState")
            .and_then(|item| item.as_str())
            .unwrap_or_default();
        let shell_nodes = Self::dom_diagnostics_count(value, "shellNodes");
        let event_cards = Self::dom_diagnostics_count(value, "wwBlockEventCards")
            + Self::dom_diagnostics_count(value, "wwMiniEventCards")
            + Self::dom_diagnostics_count(value, "mainEventCards")
            + Self::dom_diagnostics_count(value, "genericEventCards");
        let markets = Self::dom_diagnostics_count(value, "wwEventMarkets");
        let buttons = Self::dom_diagnostics_count(value, "coefficientButtons")
            + Self::dom_diagnostics_count(value, "genericCoefficientButtons");
        let mut summary = format!(
            "status={status},route={route},ready={ready_state},shell={shell_nodes},cards={event_cards},markets={markets},buttons={buttons}"
        );
        if let Some(blocker) = Self::dom_diagnostics_runtime_blocker_signal(value) {
            summary.push(',');
            summary.push_str(&blocker);
        }
        summary
    }

    fn should_abort_empty_route_early(value: &serde_json::Value) -> bool {
        if Self::dom_diagnostics_runtime_blocker_signal(value).is_some() {
            return true;
        }

        let status = Self::classify_empty_payload_diagnostics(value);
        let runtime_body_text_length =
            Self::dom_diagnostics_nested_u64(value, "runtime", "bodyTextLength");
        let runtime_button_count =
            Self::dom_diagnostics_nested_u64(value, "runtime", "buttonCount");
        let hydrated_roots = Self::dom_diagnostics_count(value, "hydratedRoots");
        let event_links = Self::dom_diagnostics_count(value, "eventLinks");
        let route_link_nodes = Self::dom_diagnostics_count(value, "routeLinkNodes");

        matches!(
            status,
            "route_ready_shell_only" | "shell_only_no_event_cards" | "no_known_winline_dom_nodes"
        ) && runtime_button_count == 0
            && hydrated_roots == 0
            && event_links == 0
            && route_link_nodes <= 24
            && runtime_body_text_length <= 600
    }

    fn should_abort_hydration_retry_after_diagnostics(
        value: &serde_json::Value,
        hydration_attempt: usize,
    ) -> bool {
        if Self::dom_diagnostics_runtime_blocker_signal(value).is_some() {
            return true;
        }

        hydration_attempt >= HEADLESS_HYDRATION_EARLY_DIAGNOSTIC_ATTEMPT
            && Self::should_abort_empty_route_early(value)
    }

    fn is_shell_only_empty_route_status(status: &str) -> bool {
        matches!(
            status,
            "route_ready_shell_only" | "shell_only_no_event_cards" | "no_known_winline_dom_nodes"
        )
    }

    fn stable_empty_route_signature(value: &serde_json::Value) -> Option<String> {
        let status = Self::classify_empty_payload_diagnostics(value);
        if !Self::is_shell_only_empty_route_status(status) {
            return None;
        }

        let route_path = Self::dom_diagnostics_nested_str(value, "route", "pathname");
        let shell_nodes = Self::dom_diagnostics_count(value, "shellNodes");
        let hydrated_roots = Self::dom_diagnostics_count(value, "hydratedRoots");
        let event_links = Self::dom_diagnostics_count(value, "eventLinks");
        let route_link_nodes = Self::dom_diagnostics_count(value, "routeLinkNodes");
        let runtime_button_count =
            Self::dom_diagnostics_nested_u64(value, "runtime", "buttonCount");
        let runtime_interactive_count =
            Self::dom_diagnostics_nested_u64(value, "runtime", "interactiveNodeCount");
        let runtime_link_count = Self::dom_diagnostics_nested_u64(value, "runtime", "linkCount");
        let runtime_body_text_bucket =
            Self::dom_diagnostics_nested_u64(value, "runtime", "bodyTextLength") / 200;

        Some(format!(
            "{status}|{route_path}|shell={shell_nodes}|hydrated={hydrated_roots}|events={event_links}|route_links={route_link_nodes}|buttons={runtime_button_count}|interactive={runtime_interactive_count}|links={runtime_link_count}|body_bucket={runtime_body_text_bucket}"
        ))
    }

    fn update_stable_empty_route_cycle_state(
        value: &serde_json::Value,
        previous_signature: &mut Option<String>,
        repeated_count: &mut usize,
    ) -> bool {
        let Some(signature) = Self::stable_empty_route_signature(value) else {
            *previous_signature = None;
            *repeated_count = 0;
            return false;
        };

        if previous_signature.as_deref() == Some(signature.as_str()) {
            *repeated_count += 1;
        } else {
            *previous_signature = Some(signature);
            *repeated_count = 1;
        }

        *repeated_count >= HEADLESS_STABLE_EMPTY_ROUTE_REPEAT_LIMIT
    }

    fn push_blocker_signal(signals: &mut Vec<String>, signal: Option<String>) {
        if let Some(signal) = signal.filter(|item| !item.is_empty()) {
            if !signals.iter().any(|existing| existing == &signal) {
                signals.push(signal);
            }
        }
    }

    fn is_skippable_live_bootstrap_navigation_error(url: &str, error: &str) -> bool {
        url == LIVE_URL && error.contains("headless navigation readiness timeout")
    }

    fn is_skippable_discovery_bootstrap_navigation_error(url: &str, error: &str) -> bool {
        url == DISCOVERY_URL && error.contains("headless navigation readiness timeout")
    }

    fn wait_for_hydrated_payload(
        tab: &headless_chrome::Tab,
        source_url: &str,
        payload: &mut Vec<serde_json::Value>,
        deadline: Option<Instant>,
    ) -> Option<String> {
        if !payload.is_empty() {
            return None;
        }

        let mut previous_empty_route_signature = None;
        let mut repeated_empty_route_count = 0;

        for attempt in 0..HEADLESS_HYDRATION_RETRY_ATTEMPTS {
            let hydration_wait_ms =
                Self::cap_wait_to_deadline_ms(HEADLESS_HYDRATION_RETRY_DELAY_MS, deadline);
            if hydration_wait_ms == 0 {
                warn!(
                    url = source_url,
                    hydration_attempt = attempt + 1,
                    remaining_budget_ms = 0,
                    "Winline: stopping hydration retry after runtime budget deadline"
                );
                return Some("budget=runtime_deadline".to_string());
            }
            std::thread::sleep(Duration::from_millis(hydration_wait_ms));
            let next_payload = Self::extract_headless_payload(tab);
            if !next_payload.is_empty() {
                debug!(
                    url = source_url,
                    hydration_attempt = attempt + 1,
                    hydration_wait_ms = hydration_wait_ms,
                    items = next_payload.len(),
                    "Winline: payload appeared after hydration retry"
                );
                *payload = next_payload;
                return None;
            }

            if attempt == 0 || attempt + 1 >= HEADLESS_HYDRATION_EARLY_DIAGNOSTIC_ATTEMPT {
                if let Some(diagnostics) = Self::extract_headless_dom_diagnostics(tab) {
                    let repeated_empty_route = Self::update_stable_empty_route_cycle_state(
                        &diagnostics,
                        &mut previous_empty_route_signature,
                        &mut repeated_empty_route_count,
                    );
                    if Self::should_abort_hydration_retry_after_diagnostics(
                        &diagnostics,
                        attempt + 1,
                    ) || repeated_empty_route
                    {
                        let mut diagnostic = Self::format_empty_payload_diagnostic(&diagnostics);
                        if repeated_empty_route {
                            diagnostic.push_str(&format!(
                                ",cycle=stable_empty_route_x{}",
                                repeated_empty_route_count
                            ));
                        }
                        warn!(
                            url = source_url,
                            hydration_attempt = attempt + 1,
                            hydration_wait_ms = hydration_wait_ms,
                            status = Self::classify_empty_payload_diagnostics(&diagnostics),
                            repeated_empty_route_count = repeated_empty_route_count,
                            diagnostic = %diagnostic,
                            "Winline: aborting hydration retry early after empty-route diagnostics"
                        );
                        return Some(diagnostic);
                    }
                }
            }
        }

        if let Some(diagnostics) = Self::extract_headless_dom_diagnostics(tab) {
            let diagnostic = Self::format_empty_payload_diagnostic(&diagnostics);
            warn!(
                url = source_url,
                status = Self::classify_empty_payload_diagnostics(&diagnostics),
                diagnostic = %diagnostic,
                ready_state = diagnostics
                    .get("readyState")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                ww_block_cards = Self::dom_diagnostics_count(&diagnostics, "wwBlockEventCards"),
                ww_mini_cards = Self::dom_diagnostics_count(&diagnostics, "wwMiniEventCards"),
                ww_event_markets = Self::dom_diagnostics_count(&diagnostics, "wwEventMarkets"),
                generic_event_cards =
                    Self::dom_diagnostics_count(&diagnostics, "genericEventCards"),
                hydrated_roots = Self::dom_diagnostics_count(&diagnostics, "hydratedRoots"),
                event_links = Self::dom_diagnostics_count(&diagnostics, "eventLinks"),
                coefficient_buttons =
                    Self::dom_diagnostics_count(&diagnostics, "coefficientButtons"),
                generic_coefficient_buttons =
                    Self::dom_diagnostics_count(&diagnostics, "genericCoefficientButtons"),
                button_nodes = Self::dom_diagnostics_count(&diagnostics, "buttonNodes"),
                route_link_nodes = Self::dom_diagnostics_count(&diagnostics, "routeLinkNodes"),
                shell_nodes = Self::dom_diagnostics_count(&diagnostics, "shellNodes"),
                route_path = Self::dom_diagnostics_nested_str(&diagnostics, "route", "pathname"),
                route_title = Self::dom_diagnostics_nested_str(&diagnostics, "route", "title"),
                route_history_length =
                    Self::dom_diagnostics_nested_u64(&diagnostics, "route", "historyLength"),
                navigation_type = diagnostics
                    .get("route")
                    .and_then(|route| route.get("navigationEntry"))
                    .and_then(|entry| entry.get("type"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                runtime_ready_state = Self::dom_diagnostics_nested_str(&diagnostics, "runtime", "readyState"),
                runtime_body_children =
                    Self::dom_diagnostics_nested_u64(&diagnostics, "runtime", "bodyChildCount"),
                runtime_body_text_length =
                    Self::dom_diagnostics_nested_u64(&diagnostics, "runtime", "bodyTextLength"),
                runtime_custom_elements =
                    Self::dom_diagnostics_nested_u64(&diagnostics, "runtime", "customElementCount"),
                runtime_button_count =
                    Self::dom_diagnostics_nested_u64(&diagnostics, "runtime", "buttonCount"),
                runtime_first_button_text = diagnostics
                    .get("runtime")
                    .and_then(|runtime| runtime.get("firstButtonText"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                body_text_sample = diagnostics
                    .get("bodyTextSample")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                first_card_text = diagnostics
                    .get("firstCardText")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                first_button_text = diagnostics
                    .get("firstButtonText")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
                "Winline: empty payload after hydration retry"
            );
            return Some(diagnostic);
        }

        None
    }

    fn cap_wait_to_deadline_ms(wait_ms: u64, deadline: Option<Instant>) -> u64 {
        match deadline {
            Some(deadline) => wait_ms.min(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64,
            ),
            None => wait_ms,
        }
    }

    fn should_skip_scroll_after_empty_extraction(
        payload: &[serde_json::Value],
        blocker_signal: Option<&str>,
    ) -> bool {
        payload.is_empty() && blocker_signal.is_some()
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
            .flat_map(|path| Self::live_headless_variants(path))
            .collect();

        HeadlessSeedPaths { prematch, live }
    }

    fn extract_from_tab(tab: &headless_chrome::Tab, source_url: &str) -> HeadlessTabExtraction {
        Self::extract_from_tab_with_deadline(tab, source_url, None)
    }

    fn extract_from_tab_with_deadline(
        tab: &headless_chrome::Tab,
        source_url: &str,
        deadline: Option<Instant>,
    ) -> HeadlessTabExtraction {
        let mut payload = Self::extract_headless_payload(tab);
        debug!(
            url = source_url,
            items = payload.len(),
            "Winline: initial tab extraction"
        );

        let blocker_signal =
            Self::wait_for_hydrated_payload(tab, source_url, &mut payload, deadline);

        if Self::should_skip_scroll_after_empty_extraction(&payload, blocker_signal.as_deref()) {
            debug!(
                url = source_url,
                blocker_signal = blocker_signal.as_deref().unwrap_or_default(),
                "Winline: skipping scroll retries after early-empty route diagnostic"
            );
            return HeadlessTabExtraction {
                payload,
                blocker_signal,
            };
        }

        for round in 0..HEADLESS_SCROLL_ROUNDS {
            let scroll_completed =
                HeadlessChromeHelper::scroll_page_with_deadline(tab, deadline).unwrap_or(false);
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

            if !scroll_completed {
                debug!(
                    url = source_url,
                    round = round + 1,
                    "Winline: stopping scroll loop after runtime budget deadline"
                );
                break;
            }
        }

        HeadlessTabExtraction {
            blocker_signal: if payload.is_empty() {
                blocker_signal
            } else {
                None
            },
            payload,
        }
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
            if is_live {
                for candidate in Self::live_headless_variants(&normalized) {
                    if seen.insert(candidate.clone()) {
                        prioritized.push(candidate);
                    }
                }
            } else if seen.insert(normalized.clone()) {
                prioritized.push(normalized);
            }
        }

        for path in paths {
            let normalized = normalize_winline_path(&path);
            if is_live {
                for candidate in Self::live_headless_variants(&normalized) {
                    if seen.insert(candidate.clone()) {
                        prioritized.push(candidate);
                    }
                }
            } else if seen.insert(normalized.clone()) {
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

    fn is_internal_runtime_budget_error(error: &str) -> bool {
        error.contains("headless runtime budget exceeded")
            || error.contains("headless navigation start budget exhausted")
            || error.contains("headless runtime empty before prematch bootstrap under tight budget")
    }

    fn is_useful_empty_route_blocker_signal(signal: &str) -> bool {
        !signal.trim().is_empty()
            && (signal.contains("blocker=")
                || signal.contains("status=")
                || signal.contains("budget="))
    }

    fn should_stop_phase_after_blocker_streak(consecutive_blocker_routes: usize) -> bool {
        consecutive_blocker_routes >= HEADLESS_BLOCKER_ROUTE_STREAK_LIMIT
    }

    fn live_route_family_key(path: &str) -> Option<String> {
        let normalized = normalize_winline_path(path);
        normalized
            .strip_prefix("/live/sport/")
            .or_else(|| normalized.strip_prefix("/live/"))
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(|slug| slug.trim_matches('/').to_string())
    }

    fn should_skip_live_route_family_after_empty_shell(signal: Option<&str>) -> bool {
        signal.is_some_and(|signal| {
            signal.contains("status=route_ready_shell_only")
                || signal.contains("status=shell_only_no_event_cards")
                || signal.contains("status=no_known_winline_dom_nodes")
        })
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
        HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS.saturating_sub(Self::runtime_elapsed_ms(started))
    }

    fn runtime_deadline(started: Instant) -> Instant {
        started + Duration::from_millis(HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS)
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

    fn should_return_partial_before_phase(
        total_events: &[Event],
        remaining_budget_ms: u64,
        bootstrap_total_ms: u64,
        previous_phase_metrics: &[HeadlessRouteMetric],
    ) -> bool {
        if total_events.is_empty() {
            return false;
        }

        let next_phase_floor_ms =
            Self::next_phase_start_floor_ms(bootstrap_total_ms, previous_phase_metrics);
        let slowest_route_ms = previous_phase_metrics
            .iter()
            .map(|metric| metric.total_ms)
            .max()
            .unwrap_or(0);
        let expensive_empty_routes = previous_phase_metrics
            .iter()
            .filter(|metric| Self::is_expensive_empty_route(metric))
            .count();

        remaining_budget_ms < next_phase_floor_ms
            && (previous_phase_metrics.is_empty()
                || slowest_route_ms >= HEADLESS_EXPENSIVE_ROUTE_MS
                || expensive_empty_routes >= HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT)
    }

    fn should_abort_empty_before_phase(
        remaining_budget_ms: u64,
        bootstrap_total_ms: u64,
        previous_phase_metrics: &[HeadlessRouteMetric],
    ) -> bool {
        let next_phase_floor_ms =
            Self::next_phase_start_floor_ms(bootstrap_total_ms, previous_phase_metrics);
        let expensive_empty_routes = previous_phase_metrics
            .iter()
            .filter(|metric| Self::is_expensive_empty_route(metric))
            .count();

        remaining_budget_ms < next_phase_floor_ms
            && expensive_empty_routes >= HEADLESS_EXPENSIVE_EMPTY_STREAK_LIMIT
    }

    fn next_phase_start_floor_ms(
        bootstrap_total_ms: u64,
        previous_phase_metrics: &[HeadlessRouteMetric],
    ) -> u64 {
        let slowest_route_ms = previous_phase_metrics
            .iter()
            .map(|metric| metric.total_ms)
            .max()
            .unwrap_or(0);

        bootstrap_total_ms
            .max(slowest_route_ms)
            .max(HEADLESS_NAVIGATION_TIMEOUT_MS)
            .saturating_add(HEADLESS_ROUTE_GUARD_MS)
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
        let runtime_started = Instant::now();
        let helper = HeadlessChromeHelper::new()?;
        let runtime_deadline = Self::runtime_deadline(runtime_started);
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen = HashSet::new();
        let mut live_metrics = Vec::new();
        let mut prematch_metrics = Vec::new();
        let mut blocker_signals = Vec::new();

        let live_bootstrap_started = Instant::now();
        let live_navigation_started = Instant::now();
        match helper.navigate_and_wait_with_timeout_and_deadline(
            LIVE_URL,
            HEADLESS_WAIT_MS,
            HEADLESS_NAVIGATION_TIMEOUT_MS,
            runtime_deadline,
        ) {
            Ok(live_tab) => {
                let live_navigation_ms = live_navigation_started.elapsed().as_millis() as u64;
                let live_extract_started = Instant::now();
                let live_payload = Self::extract_from_tab_with_deadline(
                    &live_tab,
                    LIVE_URL,
                    Some(runtime_deadline),
                );
                let live_extraction_ms = live_extract_started.elapsed().as_millis() as u64;
                Self::push_blocker_signal(&mut blocker_signals, live_payload.blocker_signal);
                debug!(
                    url = LIVE_URL,
                    items = live_payload.payload.len(),
                    "Winline: headless live payload extracted"
                );
                let live_payload_items = live_payload.payload.len();
                let live_collect_started = Instant::now();
                let live_added_events = Self::collect_headless_page(
                    &mut all_events,
                    &mut all_odds,
                    &mut seen,
                    live_payload.payload,
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
            }
            Err(error) => {
                let live_navigation_ms = live_navigation_started.elapsed().as_millis() as u64;
                let error_text = error.to_string();
                if Self::is_skippable_live_bootstrap_navigation_error(LIVE_URL, &error_text) {
                    warn!(
                        url = LIVE_URL,
                        error = %error_text,
                        navigation_ms = live_navigation_ms,
                        remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                        "Winline: skipping live bootstrap navigation failure and continuing with live fanout"
                    );
                } else {
                    return Err(error);
                }
            }
        }
        let live_bootstrap_total_ms = live_bootstrap_started.elapsed().as_millis() as u64;

        let live_paths = Self::prioritized_headless_paths(seed_paths.live, true);
        let live_fanout_started = Instant::now();
        let mut visited_live_paths = HashSet::new();
        let mut skipped_live_route_families = HashSet::new();
        visited_live_paths.insert(normalize_winline_path(LIVE_URL));
        let mut live_empty_streak = 0;
        let mut live_expensive_empty_streak = 0;
        let mut live_blocker_streak = 0;
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
            if let Some(route_family) = Self::live_route_family_key(&normalized_path) {
                if skipped_live_route_families.contains(&route_family) {
                    debug!(
                        path = normalized_path.as_str(),
                        route_family = route_family.as_str(),
                        "Winline: skipping duplicate live route family after shell-only empty route"
                    );
                    continue;
                }
            }

            let url = format!("{}{}", BASE_URL, normalized_path);
            let fallback_sport = sport_from_winline_hint(&normalized_path, Sport::Other);
            let route_started = Instant::now();
            let navigation_started = Instant::now();
            let tab = match helper.navigate_and_wait_with_timeout_and_deadline(
                &url,
                HEADLESS_WAIT_MS,
                HEADLESS_NAVIGATION_TIMEOUT_MS,
                runtime_deadline,
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
            let payload = Self::extract_from_tab_with_deadline(&tab, &url, Some(runtime_deadline));
            let extraction_ms = extract_started.elapsed().as_millis() as u64;
            let route_blocker_signal = payload.blocker_signal.clone();
            Self::push_blocker_signal(&mut blocker_signals, route_blocker_signal.clone());
            let payload_items = payload.payload.len();
            let collect_started = Instant::now();
            let added_events = Self::collect_headless_page(
                &mut all_events,
                &mut all_odds,
                &mut seen,
                payload.payload,
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
            live_blocker_streak = if added_events == 0
                && route_blocker_signal
                    .as_deref()
                    .is_some_and(Self::is_useful_empty_route_blocker_signal)
            {
                live_blocker_streak + 1
            } else {
                0
            };
            if added_events == 0
                && Self::should_skip_live_route_family_after_empty_shell(
                    route_blocker_signal.as_deref(),
                )
            {
                if let Some(route_family) = Self::live_route_family_key(&normalized_path) {
                    skipped_live_route_families.insert(route_family.clone());
                    debug!(
                        path = normalized_path.as_str(),
                        route_family = route_family.as_str(),
                        blocker_signal = route_blocker_signal.as_deref().unwrap_or_default(),
                        "Winline: suppressing sibling live route variant after shell-only empty route"
                    );
                }
            }

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

            if Self::should_stop_phase_after_blocker_streak(live_blocker_streak) {
                warn!(
                    phase = "live",
                    blocker_streak = live_blocker_streak,
                    blocker_signal = route_blocker_signal.as_deref().unwrap_or_default(),
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after consecutive blocker routes"
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

        let remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started);
        if Self::should_return_partial_before_phase(
            &all_events,
            remaining_budget_ms,
            live_bootstrap_total_ms,
            &live_metrics,
        ) {
            let live = all_events.iter().filter(|event| event.is_live).count();
            let prematch = all_events.len().saturating_sub(live);
            warn!(
                phase = "live",
                next_phase = "prematch-bootstrap",
                remaining_budget_ms = remaining_budget_ms,
                bootstrap_total_ms = live_bootstrap_total_ms,
                slowest_route_ms = live_metrics.iter().map(|metric| metric.total_ms).max().unwrap_or(0),
                expensive_empty_routes = live_metrics
                    .iter()
                    .filter(|metric| Self::is_expensive_empty_route(metric))
                    .count(),
                total = all_events.len(),
                live = live,
                prematch = prematch,
                odds = all_odds.len(),
                "Winline: returning partial headless result before prematch bootstrap under tight runtime budget"
            );
            return Ok((all_events, all_odds));
        }

        if all_events.is_empty()
            && Self::should_abort_empty_before_phase(
                remaining_budget_ms,
                live_bootstrap_total_ms,
                &live_metrics,
            )
        {
            let blocker = if blocker_signals.is_empty() {
                format!(
                    "budget=live_phase_empty_tight_budget,remaining_ms={},bootstrap_ms={},slowest_route_ms={}",
                    remaining_budget_ms,
                    live_bootstrap_total_ms,
                    live_metrics.iter().map(|metric| metric.total_ms).max().unwrap_or(0)
                )
            } else {
                blocker_signals.join(" || ")
            };
            return Err(format!(
                "headless runtime empty before prematch bootstrap under tight budget: {}",
                blocker
            )
            .into());
        }

        let discovery_started = Instant::now();
        let mut discovered_paths = Vec::new();
        let discovery_navigation_started = Instant::now();
        let mut discovery_source_url = DISCOVERY_URL;
        let discovery_tab = match helper.navigate_and_wait_with_timeout_and_deadline(
            DISCOVERY_URL,
            HEADLESS_WAIT_MS,
            HEADLESS_NAVIGATION_TIMEOUT_MS,
            runtime_deadline,
        ) {
            Ok(tab) => Some(tab),
            Err(error) => {
                let navigation_ms = discovery_navigation_started.elapsed().as_millis() as u64;
                let error_text = error.to_string();
                warn!(
                    phase = "prematch-bootstrap",
                    url = DISCOVERY_URL,
                    status = "navigation_failed",
                    navigation_ms = navigation_ms,
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    total = all_events.len(),
                    odds = all_odds.len(),
                    error = %error_text,
                    "Winline: prematch bootstrap navigation failed"
                );
                if Self::is_skippable_discovery_bootstrap_navigation_error(DISCOVERY_URL, &error_text)
                {
                    let fallback_navigation_started = Instant::now();
                    match helper.navigate_and_wait_with_timeout_and_deadline(
                        DISCOVERY_FALLBACK_URL,
                        HEADLESS_WAIT_MS,
                        HEADLESS_NAVIGATION_TIMEOUT_MS,
                        runtime_deadline,
                    ) {
                        Ok(tab) => {
                            discovery_source_url = DISCOVERY_FALLBACK_URL;
                            warn!(
                                phase = "prematch-bootstrap",
                                url = DISCOVERY_URL,
                                fallback_url = DISCOVERY_FALLBACK_URL,
                                navigation_ms = navigation_ms,
                                fallback_navigation_ms = fallback_navigation_started.elapsed().as_millis() as u64,
                                remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                                "Winline: retrying prematch bootstrap via alternate football route after discovery readiness timeout"
                            );
                            Some(tab)
                        }
                        Err(fallback_error) => {
                            warn!(
                                phase = "prematch-bootstrap",
                                url = DISCOVERY_URL,
                                fallback_url = DISCOVERY_FALLBACK_URL,
                                navigation_ms = navigation_ms,
                                fallback_navigation_ms = fallback_navigation_started.elapsed().as_millis() as u64,
                                remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                                error = %fallback_error,
                                "Winline: alternate prematch bootstrap route failed; continuing with seeded prematch fanout"
                            );
                            None
                        }
                    }
                } else if !all_events.is_empty() {
                    return Ok((all_events, all_odds));
                } else {
                    return Err(error);
                }
            }
        };

        if let Some(discovery_tab) = discovery_tab {
            let discovery_navigation_ms = discovery_navigation_started.elapsed().as_millis() as u64;
            let discovery_extract_started = Instant::now();
            discovered_paths = Self::extract_discovered_sport_links(&discovery_tab);
            let discovery_payload = Self::extract_from_tab_with_deadline(
                &discovery_tab,
                discovery_source_url,
                Some(runtime_deadline),
            );
            let discovery_extraction_ms = discovery_extract_started.elapsed().as_millis() as u64;
            Self::push_blocker_signal(&mut blocker_signals, discovery_payload.blocker_signal);
            debug!(
                url = discovery_source_url,
                items = discovery_payload.payload.len(),
                discovered = discovered_paths.len(),
                "Winline: headless sport discovery extracted"
            );
            let discovery_payload_items = discovery_payload.payload.len();
            let discovery_collect_started = Instant::now();
            let discovery_added_events = Self::collect_headless_page(
                &mut all_events,
                &mut all_odds,
                &mut seen,
                discovery_payload.payload,
                Sport::Football,
                false,
                discovery_source_url,
            );
            let discovery_collect_ms = discovery_collect_started.elapsed().as_millis() as u64;
            Self::log_phase_result(
                "prematch-bootstrap",
                discovery_source_url,
                discovery_payload_items,
                discovery_added_events,
                discovery_started,
                discovery_navigation_ms,
                discovery_extraction_ms,
                discovery_collect_ms,
                &all_events,
                all_odds.len(),
            );
        }

        let mut visited_paths = HashSet::new();
        visited_paths.insert(normalize_winline_path(DISCOVERY_URL));
        if discovery_source_url != DISCOVERY_URL {
            visited_paths.insert(normalize_winline_path(discovery_source_url));
        }

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
        let mut prematch_blocker_streak = 0;

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
            let tab = match helper.navigate_and_wait_with_timeout_and_deadline(
                &url,
                HEADLESS_WAIT_MS,
                HEADLESS_NAVIGATION_TIMEOUT_MS,
                runtime_deadline,
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
            let payload = Self::extract_from_tab_with_deadline(&tab, &url, Some(runtime_deadline));
            let extraction_ms = extract_started.elapsed().as_millis() as u64;
            let route_blocker_signal = payload.blocker_signal.clone();
            Self::push_blocker_signal(&mut blocker_signals, route_blocker_signal.clone());
            let payload_items = payload.payload.len();
            let collect_started = Instant::now();
            let added_events = Self::collect_headless_page(
                &mut all_events,
                &mut all_odds,
                &mut seen,
                payload.payload,
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
            prematch_blocker_streak = if added_events == 0
                && route_blocker_signal
                    .as_deref()
                    .is_some_and(Self::is_useful_empty_route_blocker_signal)
            {
                prematch_blocker_streak + 1
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

            if Self::should_stop_phase_after_blocker_streak(prematch_blocker_streak) {
                warn!(
                    phase = "prematch",
                    blocker_streak = prematch_blocker_streak,
                    blocker_signal = route_blocker_signal.as_deref().unwrap_or_default(),
                    remaining_budget_ms = Self::runtime_remaining_budget_ms(runtime_started),
                    "Winline: stopping headless fanout after consecutive blocker routes"
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

        if all_events.is_empty() && !blocker_signals.is_empty() {
            return Err(format!(
                "headless runtime empty after DOM extraction: {}",
                blocker_signals.join(" || ")
            )
            .into());
        }

        Ok((all_events, all_odds))
    }

    async fn fetch_via_headless(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let seed_paths = self.fetch_seed_paths().await;
        match tokio::time::timeout(
            Duration::from_millis(HEADLESS_RUNTIME_BUDGET_MS + HEADLESS_OUTER_TIMEOUT_RESERVE_MS),
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
                HEADLESS_RUNTIME_BUDGET_MS + HEADLESS_OUTER_TIMEOUT_RESERVE_MS
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
        let mut runtime_failure_contexts = Vec::new();

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
                runtime_failure_contexts.push("headless_dom_empty".to_string());
            }
            Err(error) => {
                let error_text = error.to_string();
                skip_playwright_fallback = Self::is_internal_runtime_budget_error(&error_text);
                runtime_failure_contexts.push(format!("headless={error_text}"));
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
                    runtime_failure_contexts.push("playwright_dom_empty".to_string());
                }
                Err(error) => {
                    runtime_failure_contexts.push(format!("playwright={error}"));
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
            if !runtime_failure_contexts.is_empty() {
                return Err(runtime_failure_contexts.join(" | ").into());
            }
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
          HeadlessRouteMetric, WinlineParser, BOOTSTRAP_WEBSCRIPT_PATH, DISCOVERY_FALLBACK_URL,
          DISCOVERY_URL, HEADLESS_DOM_DIAGNOSTICS_JS,
           HEADLESS_BLOCKER_ROUTE_STREAK_LIMIT,
           HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS, HEADLESS_EXPENSIVE_ROUTE_MS, HEADLESS_EXTRACT_JS,
          HEADLESS_HYDRATION_RETRY_ATTEMPTS, HEADLESS_HYDRATION_RETRY_DELAY_MS,
         HEADLESS_LIVE_FANOUT_BUDGET_MS, HEADLESS_NAVIGATION_TIMEOUT_MS,
        HEADLESS_OUTER_TIMEOUT_RESERVE_MS, HEADLESS_PREMATCH_FANOUT_BUDGET_MS,
        HEADLESS_ROUTE_GUARD_MS, HEADLESS_RUNTIME_BUDGET_MS, HEADLESS_SCROLL_ROUNDS,
        HEADLESS_STABLE_EMPTY_ROUTE_REPEAT_LIMIT, HEADLESS_WAIT_MS, LIVE_URL,
    };
    use crate::headless_helper::SCROLL_PAGE_BUDGET_MS;
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

        assert_eq!(ordered[0], "/live/nastolijnyj_tennis");
        assert_eq!(ordered[1], "/live/sport/nastolijnyj_tennis");
        assert!(ordered.iter().any(|path| path == "/live/futbol"));
        assert!(ordered.iter().any(|path| path == "/live/sport/futbol"));
        assert_eq!(
            ordered
                .iter()
                .filter(|path| path.as_str() == "/live/futbol")
                .count(),
            1
        );
    }

    #[test]
    fn expands_live_headless_variants_from_prematch_path() {
        assert_eq!(
            WinlineParser::live_headless_variants("/stavki/sport/tennis"),
            vec!["/live/tennis".to_string(), "/live/sport/tennis".to_string()]
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
        let started = Instant::now()
            - Duration::from_millis(
                HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS.saturating_sub(HEADLESS_ROUTE_GUARD_MS) + 500,
            );
        assert!(!WinlineParser::runtime_budget_allows_next_route(started));

        let started = Instant::now()
            - Duration::from_millis(
                HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS
                    .saturating_sub(HEADLESS_ROUTE_GUARD_MS)
                    .saturating_sub(500),
            );
        assert!(WinlineParser::runtime_budget_allows_next_route(started));
    }

    #[test]
    fn headless_navigation_timeout_stays_inside_route_guard() {
        assert!(HEADLESS_NAVIGATION_TIMEOUT_MS < HEADLESS_ROUTE_GUARD_MS);
    }

    #[test]
    fn route_guard_covers_navigation_hydration_and_scroll_budget() {
        assert_eq!(
            HEADLESS_ROUTE_GUARD_MS,
            HEADLESS_NAVIGATION_TIMEOUT_MS
                + HEADLESS_HYDRATION_RETRY_DELAY_MS * HEADLESS_HYDRATION_RETRY_ATTEMPTS as u64
                + SCROLL_PAGE_BUDGET_MS * HEADLESS_SCROLL_ROUNDS as u64
        );
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

    #[test]
    fn returns_partial_before_next_phase_when_budget_is_tighter_than_measured_live_cost() {
        let events = vec![shared::Event {
            id: "winline-live-1".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: true,
            bookmaker_slug: "winline".into(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        }];
        let metrics = vec![
            HeadlessRouteMetric {
                phase: "live",
                path: "/live/sport/darts".into(),
                sport: Sport::Darts,
                status: "ok",
                payload_items: 0,
                added_events: 0,
                navigation_ms: 4_000,
                extraction_ms: 3_000,
                collect_ms: 0,
                total_ms: 12_500,
                expensive: true,
            },
            HeadlessRouteMetric {
                phase: "live",
                path: "/live/sport/snuker".into(),
                sport: Sport::Snooker,
                status: "ok",
                payload_items: 0,
                added_events: 0,
                navigation_ms: 4_200,
                extraction_ms: 3_100,
                collect_ms: 0,
                total_ms: 12_700,
                expensive: true,
            },
        ];

        assert!(WinlineParser::should_return_partial_before_phase(
            &events, 18_000, 11_500, &metrics,
        ));
    }

    #[test]
    fn keeps_next_phase_when_no_partial_data_or_budget_is_healthy() {
        let metrics = vec![HeadlessRouteMetric {
            phase: "live",
            path: "/live/sport/futbol".into(),
            sport: Sport::Football,
            status: "ok",
            payload_items: 24,
            added_events: 6,
            navigation_ms: 2_000,
            extraction_ms: 1_500,
            collect_ms: 5,
            total_ms: 3_505,
            expensive: false,
        }];
        let events = vec![shared::Event {
            id: "winline-live-2".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: true,
            bookmaker_slug: "winline".into(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        }];

        assert!(!WinlineParser::should_return_partial_before_phase(
            &[],
            5_000,
            12_000,
            &metrics,
        ));
        assert!(!WinlineParser::should_return_partial_before_phase(
            &events, 25_000, 12_000, &metrics,
        ));
    }

    #[test]
    fn returns_partial_before_next_phase_after_single_slow_route_when_budget_is_tight() {
        let metrics = vec![HeadlessRouteMetric {
            phase: "live",
            path: "/live/sport/futbol".into(),
            sport: Sport::Football,
            status: "ok",
            payload_items: 12,
            added_events: 4,
            navigation_ms: 6_200,
            extraction_ms: 5_100,
            collect_ms: 5,
            total_ms: 12_300,
            expensive: true,
        }];
        let events = vec![shared::Event {
            id: "winline-live-tight-budget".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: true,
            bookmaker_slug: "winline".into(),
            raw_url: None,
            extra: std::collections::HashMap::new(),
        }];

        assert!(WinlineParser::should_return_partial_before_phase(
            &events, 18_000, 11_500, &metrics,
        ));
    }

    #[test]
    fn aborts_empty_next_phase_startup_when_budget_is_already_too_tight() {
        let metrics = vec![
            HeadlessRouteMetric {
                phase: "live",
                path: "/live/sport/darts".into(),
                sport: Sport::Darts,
                status: "ok",
                payload_items: 0,
                added_events: 0,
                navigation_ms: 4_000,
                extraction_ms: 3_000,
                collect_ms: 0,
                total_ms: 12_500,
                expensive: true,
            },
            HeadlessRouteMetric {
                phase: "live",
                path: "/live/sport/snuker".into(),
                sport: Sport::Snooker,
                status: "ok",
                payload_items: 0,
                added_events: 0,
                navigation_ms: 4_200,
                extraction_ms: 3_100,
                collect_ms: 0,
                total_ms: 12_700,
                expensive: true,
            },
        ];

        assert!(WinlineParser::should_abort_empty_before_phase(
            18_000, 11_500, &metrics,
        ));
        assert!(!WinlineParser::should_abort_empty_before_phase(
            26_000, 11_500, &metrics,
        ));
    }

    #[test]
    fn extraction_path_uses_scroll_aware_helper_for_live_and_prematch_fanout() {
        let source = include_str!("winline.rs");
        let scroll_aware_calls = source
            .lines()
            .filter(|line| line.trim() == "let payload = Self::extract_from_tab(&tab, &url);")
            .count();
        let budget_aware_calls = source
            .lines()
            .filter(|line| {
                line.trim()
                    == "let payload = Self::extract_from_tab_with_deadline(&tab, &url, Some(runtime_deadline));"
            })
            .count();
        let direct_payload_calls = source
            .lines()
            .filter(|line| line.trim() == "let payload = Self::extract_headless_payload(&tab);")
            .count();

        assert!(source.contains(
            "let payload = Self::extract_from_tab_with_deadline(&tab, &url, Some(runtime_deadline));"
        ));
        assert_eq!(scroll_aware_calls, 0);
        assert_eq!(budget_aware_calls, 2);
        assert_eq!(direct_payload_calls, 0);
    }

    #[test]
    fn live_phase_navigation_uses_deadline_capped_helper() {
        let source = include_str!("winline.rs");
        assert!(source.contains(
            "match helper.navigate_and_wait_with_timeout_and_deadline(\n            LIVE_URL,"
        ));
        assert!(source.contains("let tab = match helper.navigate_and_wait_with_timeout_and_deadline(\n                &url,"));
    }

    #[test]
    fn deadline_capped_wait_drops_to_zero_after_budget_expires() {
        let deadline = Instant::now() - Duration::from_millis(1);
        assert_eq!(
            WinlineParser::cap_wait_to_deadline_ms(400, Some(deadline)),
            0
        );
        assert_eq!(WinlineParser::cap_wait_to_deadline_ms(400, None), 400);
    }

    #[test]
    fn runtime_deadline_stays_inside_outer_timeout_reserve() {
        let started = Instant::now() - Duration::from_millis(250);
        let remaining = WinlineParser::runtime_deadline(started)
            .saturating_duration_since(Instant::now())
            .as_millis() as u64;
        assert!(remaining <= HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS);
        assert!(remaining >= HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS.saturating_sub(500));
    }

    #[test]
    fn outer_timeout_reserve_is_positive() {
        assert!(HEADLESS_RUNTIME_BUDGET_MS > HEADLESS_OUTER_TIMEOUT_RESERVE_MS);
        assert_eq!(
            HEADLESS_EFFECTIVE_RUNTIME_BUDGET_MS,
            HEADLESS_RUNTIME_BUDGET_MS - HEADLESS_OUTER_TIMEOUT_RESERVE_MS
        );
    }

    #[test]
    fn hydration_retry_window_covers_observed_winline_spa_boot_time() {
        assert!(
            HEADLESS_WAIT_MS
                + HEADLESS_HYDRATION_RETRY_DELAY_MS * HEADLESS_HYDRATION_RETRY_ATTEMPTS as u64
                >= 5_000
        );
    }

    #[test]
    fn early_empty_route_abort_triggers_for_shell_only_runtime_with_no_interactive_nodes() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/live/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 8,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 220,
                "buttonCount": 0
            }
        });

        assert!(WinlineParser::should_abort_empty_route_early(&diagnostics));
    }

    #[test]
    fn early_empty_route_abort_stays_off_for_richer_shell_routes_without_blocker() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 48,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1400,
                "buttonCount": 0
            }
        });

        assert!(!WinlineParser::should_abort_empty_route_early(&diagnostics));
    }

    #[test]
    fn early_empty_route_abort_triggers_for_runtime_blocker_even_with_richer_dom() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 52,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1800,
                "buttonCount": 4,
                "blocker": {
                    "kind": "captcha",
                    "source": "body",
                    "matchedText": "Please complete captcha"
                }
            }
        });

        assert!(WinlineParser::should_abort_empty_route_early(&diagnostics));
    }

    #[test]
    fn hydration_retry_aborts_on_first_diagnostic_attempt_when_runtime_blocker_is_present() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 52,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1800,
                "buttonCount": 4,
                "blocker": {
                    "kind": "captcha",
                    "source": "body",
                    "matchedText": "Please complete captcha"
                }
            }
        });

        assert!(WinlineParser::should_abort_hydration_retry_after_diagnostics(
            &diagnostics,
            1,
        ));
    }

    #[test]
    fn hydration_retry_keeps_shell_only_abort_gated_until_early_diagnostic_threshold() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/live/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 8,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 220,
                "buttonCount": 0
            }
        });

        assert!(!WinlineParser::should_abort_hydration_retry_after_diagnostics(
            &diagnostics,
            1,
        ));
        assert!(WinlineParser::should_abort_hydration_retry_after_diagnostics(
            &diagnostics,
            super::HEADLESS_HYDRATION_EARLY_DIAGNOSTIC_ATTEMPT,
        ));
    }

    #[test]
    fn stable_empty_route_cycle_triggers_abort_for_repeated_richer_shell_routes() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 48,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1400,
                "buttonCount": 0,
                "linkCount": 52
            }
        });
        let mut previous_signature = None;
        let mut repeated_count = 0;

        assert!(!WinlineParser::update_stable_empty_route_cycle_state(
            &diagnostics,
            &mut previous_signature,
            &mut repeated_count,
        ));
        assert_eq!(repeated_count, 1);
        assert!(WinlineParser::update_stable_empty_route_cycle_state(
            &diagnostics,
            &mut previous_signature,
            &mut repeated_count,
        ));
        assert_eq!(repeated_count, HEADLESS_STABLE_EMPTY_ROUTE_REPEAT_LIMIT);
    }

    #[test]
    fn stable_empty_route_cycle_state_resets_after_dom_progress() {
        let shell_only = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "hydratedRoots": 0,
                "eventLinks": 0,
                "routeLinkNodes": 48,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1400,
                "buttonCount": 0,
                "linkCount": 52
            }
        });
        let progressed = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "wwBlockEventCards": 4,
                "wwEventMarkets": 8,
                "coefficientButtons": 12,
                "hydratedRoots": 2,
                "eventLinks": 4,
                "routeLinkNodes": 48,
                "shellNodes": 5
            },
            "runtime": {
                "bodyTextLength": 1800,
                "buttonCount": 12,
                "linkCount": 56
            }
        });
        let mut previous_signature = None;
        let mut repeated_count = 0;

        assert!(!WinlineParser::update_stable_empty_route_cycle_state(
            &shell_only,
            &mut previous_signature,
            &mut repeated_count,
        ));
        assert!(!WinlineParser::update_stable_empty_route_cycle_state(
            &progressed,
            &mut previous_signature,
            &mut repeated_count,
        ));
        assert_eq!(repeated_count, 0);
        assert!(previous_signature.is_none());
    }

    #[test]
    fn skips_scroll_retries_after_early_empty_route_signal() {
        assert!(WinlineParser::should_skip_scroll_after_empty_extraction(
            &[],
            Some("status=route_ready_shell_only")
        ));
        assert!(!WinlineParser::should_skip_scroll_after_empty_extraction(
            &[],
            None
        ));
        assert!(!WinlineParser::should_skip_scroll_after_empty_extraction(
            &[serde_json::json!({"eventId": "1"})],
            Some("blocker=captcha@body:test")
        ));
    }

    #[test]
    fn classifies_shell_only_empty_payload_diagnostics() {
        let diagnostics = serde_json::json!({
            "counts": {
                "wwBlockEventCards": 0,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 0,
                "wwEventMarkets": 0,
                "coefficientButtons": 0,
                "genericCoefficientButtons": 0,
                "shellNodes": 5
            }
        });

        assert_eq!(
            WinlineParser::classify_empty_payload_diagnostics(&diagnostics),
            "shell_only_no_event_cards"
        );
    }

    #[test]
    fn classifies_cards_present_empty_payload_diagnostics() {
        let diagnostics = serde_json::json!({
            "counts": {
                "wwBlockEventCards": 27,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 0,
                "wwEventMarkets": 153,
                "coefficientButtons": 189,
                "genericCoefficientButtons": 0,
                "shellNodes": 5
            }
        });

        assert_eq!(
            WinlineParser::classify_empty_payload_diagnostics(&diagnostics),
            "cards_present_extract_empty"
        );
    }

    #[test]
    fn classifies_generic_hydrated_cards_as_incomplete_markets() {
        let diagnostics = serde_json::json!({
            "counts": {
                "wwBlockEventCards": 0,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 12,
                "wwEventMarkets": 0,
                "coefficientButtons": 0,
                "genericCoefficientButtons": 24,
                "shellNodes": 2
            }
        });

        assert_eq!(
            WinlineParser::classify_empty_payload_diagnostics(&diagnostics),
            "cards_present_incomplete_markets"
        );
    }

    #[test]
    fn classifies_hydrated_roots_empty_payload_diagnostics() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/stavki/sport/futbol"
            },
            "counts": {
                "wwBlockEventCards": 0,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 0,
                "wwEventMarkets": 0,
                "coefficientButtons": 0,
                "genericCoefficientButtons": 32,
                "hydratedRoots": 11,
                "shellNodes": 4
            }
        });

        assert_eq!(
            WinlineParser::classify_empty_payload_diagnostics(&diagnostics),
            "hydrated_roots_extract_empty"
        );
    }

    #[test]
    fn classifies_route_ready_shell_only_payload_diagnostics() {
        let diagnostics = serde_json::json!({
            "route": {
                "pathname": "/live/futbol"
            },
            "counts": {
                "wwBlockEventCards": 0,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 0,
                "wwEventMarkets": 0,
                "coefficientButtons": 0,
                "genericCoefficientButtons": 0,
                "hydratedRoots": 0,
                "shellNodes": 5
            }
        });

        assert_eq!(
            WinlineParser::classify_empty_payload_diagnostics(&diagnostics),
            "route_ready_shell_only"
        );
    }

    #[test]
    fn extractor_source_contains_generic_dom_fallback() {
        assert!(HEADLESS_EXTRACT_JS.contains("GENERIC_CARD_SELECTOR"));
        assert!(HEADLESS_EXTRACT_JS.contains("collectGenericMarkets"));
        assert!(HEADLESS_EXTRACT_JS.contains("[data-testid*=\"event\"]"));
        assert!(HEADLESS_EXTRACT_JS.contains("[data-testid*=\"coef\"]"));
    }

    #[test]
    fn extractor_source_contains_hydrated_text_fallback() {
        assert!(HEADLESS_EXTRACT_JS.contains("extractNamesFromHydratedText"));
        assert!(HEADLESS_EXTRACT_JS.contains("findHydratedEventRoot"));
        assert!(HEADLESS_EXTRACT_JS.contains("Array.from(hydratedRoots).forEach"));
    }

    #[test]
    fn dom_diagnostics_source_contains_route_and_hydration_probes() {
        assert!(HEADLESS_DOM_DIAGNOSTICS_JS.contains("hydratedRoots"));
        assert!(HEADLESS_DOM_DIAGNOSTICS_JS.contains("navigationEntry"));
        assert!(HEADLESS_DOM_DIAGNOSTICS_JS.contains("routeLinkNodes"));
        assert!(HEADLESS_DOM_DIAGNOSTICS_JS.contains("firstButtonText"));
    }

    #[test]
    fn formats_runtime_blocker_signal_from_dom_diagnostics() {
        let diagnostics = serde_json::json!({
            "runtime": {
                "blocker": {
                    "kind": "access_denied",
                    "source": "title",
                    "matchedText": "Access denied"
                }
            }
        });

        assert_eq!(
            WinlineParser::dom_diagnostics_runtime_blocker_signal(&diagnostics).as_deref(),
            Some("blocker=access_denied@title:Access denied")
        );
    }

    #[test]
    fn includes_blocker_signal_in_empty_payload_diagnostic_summary() {
        let diagnostics = serde_json::json!({
            "readyState": "complete",
            "route": {
                "pathname": "/live/futbol"
            },
            "counts": {
                "wwBlockEventCards": 0,
                "wwMiniEventCards": 0,
                "mainEventCards": 0,
                "genericEventCards": 0,
                "wwEventMarkets": 0,
                "coefficientButtons": 0,
                "genericCoefficientButtons": 0,
                "shellNodes": 5
            },
            "runtime": {
                "blocker": {
                    "kind": "captcha",
                    "source": "body",
                    "matchedText": "Please complete captcha"
                }
            }
        });

        let summary = WinlineParser::format_empty_payload_diagnostic(&diagnostics);
        assert!(summary.contains("status=route_ready_shell_only"));
        assert!(summary.contains("route=/live/futbol"));
        assert!(summary.contains("blocker=captcha@body:Please complete captcha"));
    }

    #[test]
    fn keeps_unique_blocker_signals_only() {
        let mut signals = Vec::new();
        WinlineParser::push_blocker_signal(&mut signals, Some("blocker=a".into()));
        WinlineParser::push_blocker_signal(&mut signals, Some("blocker=a".into()));
        WinlineParser::push_blocker_signal(&mut signals, Some("blocker=b".into()));

        assert_eq!(
            signals,
            vec!["blocker=a".to_string(), "blocker=b".to_string()]
        );
    }

    #[test]
    fn skips_base_live_navigation_readiness_timeout_only() {
        assert!(WinlineParser::is_skippable_live_bootstrap_navigation_error(
            LIVE_URL,
            "headless navigation readiness timeout after 6000ms for https://winline.ru/live"
        ));
        assert!(!WinlineParser::is_skippable_live_bootstrap_navigation_error(
            "https://winline.ru/live/futbol",
            "headless navigation readiness timeout after 6000ms for https://winline.ru/live/futbol"
        ));
        assert!(
            !WinlineParser::is_skippable_live_bootstrap_navigation_error(
                LIVE_URL,
                "cloudflare challenge detected"
            )
        );
    }

    #[test]
    fn skips_only_discovery_route_readiness_timeout_for_prematch_bootstrap() {
        assert!(WinlineParser::is_skippable_discovery_bootstrap_navigation_error(
            DISCOVERY_URL,
            "headless navigation readiness timeout after 6000ms for https://winline.ru/stavki/sport/futbol/"
        ));
        assert!(!WinlineParser::is_skippable_discovery_bootstrap_navigation_error(
            DISCOVERY_FALLBACK_URL,
            "headless navigation readiness timeout after 6000ms for https://winline.ru/football"
        ));
        assert!(!WinlineParser::is_skippable_discovery_bootstrap_navigation_error(
            DISCOVERY_URL,
            "cloudflare challenge detected"
        ));
    }

    #[test]
    fn classifies_new_start_budget_errors_as_internal_runtime_budget() {
        assert!(WinlineParser::is_internal_runtime_budget_error(
            "headless runtime budget exceeded after 75000ms"
        ));
        assert!(WinlineParser::is_internal_runtime_budget_error(
            "headless navigation start budget exhausted before navigate for https://winline.ru/live (remaining=850ms, required=1000ms)"
        ));
        assert!(WinlineParser::is_internal_runtime_budget_error(
            "headless runtime empty before prematch bootstrap under tight budget: budget=live_phase_empty_tight_budget"
        ));
        assert!(!WinlineParser::is_internal_runtime_budget_error(
            "cloudflare challenge detected"
        ));
    }

    #[test]
    fn recognizes_useful_empty_route_blocker_signals() {
        assert!(WinlineParser::is_useful_empty_route_blocker_signal(
            "blocker=captcha@body:test"
        ));
        assert!(WinlineParser::is_useful_empty_route_blocker_signal(
            "status=route_ready_shell_only,route=/live/futbol"
        ));
        assert!(WinlineParser::is_useful_empty_route_blocker_signal(
            "budget=runtime_deadline"
        ));
        assert!(!WinlineParser::is_useful_empty_route_blocker_signal("route=/live/futbol"));
        assert!(!WinlineParser::is_useful_empty_route_blocker_signal("   "));
    }

    #[test]
    fn blocker_route_streak_stops_phase_at_limit() {
        assert!(!WinlineParser::should_stop_phase_after_blocker_streak(
            HEADLESS_BLOCKER_ROUTE_STREAK_LIMIT.saturating_sub(1)
        ));
        assert!(WinlineParser::should_stop_phase_after_blocker_streak(
            HEADLESS_BLOCKER_ROUTE_STREAK_LIMIT
        ));
    }

    #[test]
    fn derives_same_live_route_family_for_both_live_variants() {
        assert_eq!(
            WinlineParser::live_route_family_key("/live/futbol").as_deref(),
            Some("futbol")
        );
        assert_eq!(
            WinlineParser::live_route_family_key("/live/sport/futbol").as_deref(),
            Some("futbol")
        );
        assert_eq!(
            WinlineParser::live_route_family_key("/stavki/sport/futbol"),
            None
        );
    }

    #[test]
    fn only_shell_only_empty_status_skips_sibling_live_variant() {
        assert!(WinlineParser::should_skip_live_route_family_after_empty_shell(
            Some("status=route_ready_shell_only,route=/live/futbol")
        ));
        assert!(WinlineParser::should_skip_live_route_family_after_empty_shell(
            Some("status=shell_only_no_event_cards,route=/live/futbol")
        ));
        assert!(!WinlineParser::should_skip_live_route_family_after_empty_shell(
            Some("blocker=captcha@body:test")
        ));
        assert!(!WinlineParser::should_skip_live_route_family_after_empty_shell(
            Some("budget=runtime_deadline")
        ));
    }

    #[test]
    fn runtime_budget_starts_before_headless_helper_initialization() {
        let source = include_str!("winline.rs");
        let runtime_started_pos = source
            .find("let runtime_started = Instant::now();")
            .expect("runtime_started marker");
        let helper_new_pos = source
            .find("let helper = HeadlessChromeHelper::new()?;")
            .expect("helper init marker");

        assert!(runtime_started_pos < helper_new_pos);
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

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(ParserReadiness {
            stage: ParserReadinessStage::Production,
            production_enabled: true,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "headless_runtime_path_enabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Primary path uses headless Chrome with DOM hydration and Playwright fallback.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "headless_runtime_timeout_configured".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!("Runtime budget: {}ms, route guard: {}ms.", HEADLESS_RUNTIME_BUDGET_MS, HEADLESS_ROUTE_GUARD_MS),
                },
                ParserDiagnosticCheck {
                    code: "headless_hydration_retry_enabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!("Hydration retries: {} attempts with {}ms delay.", HEADLESS_HYDRATION_RETRY_ATTEMPTS, HEADLESS_HYDRATION_RETRY_DELAY_MS),
                },
            ],
        })
    }
}
