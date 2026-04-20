use headless_chrome::{types::Bounds, Browser, LaunchOptionsBuilder};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

#[derive(Clone, Copy, Debug)]
pub struct HeadlessProfile {
    pub label: &'static str,
    pub user_agent: &'static str,
    pub accept_language: &'static str,
    pub platform: &'static str,
    pub viewport: (u32, u32),
    pub is_mobile: bool,
    pub app_marker: Option<&'static str>,
}

pub const DESKTOP_PROFILE: HeadlessProfile = HeadlessProfile {
    label: "desktop",
    user_agent:
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    accept_language: "ru-RU,ru;q=0.9",
    platform: "Win32",
    viewport: (1440, 2200),
    is_mobile: false,
    app_marker: None,
};

/// Common headless Chrome utilities for SPA bookmaker parsers
pub struct HeadlessChromeHelper {
    browser: Browser,
}

const SELECTOR_READY_POST_WAIT_CAP_MS: u64 = 500;
const SELECTOR_MISS_POST_WAIT_CAP_MS: u64 = 600;
const SCROLL_STEP_WAIT_MS: u64 = 350;
pub const SCROLL_PAGE_BUDGET_MS: u64 = SCROLL_STEP_WAIT_MS * 2;
const NAVIGATION_START_BUDGET_MS: u64 = 1_000;
const BOOTSTRAP_CAPTURE_ATTEMPTS: usize = 3;
const BOOTSTRAP_CAPTURE_RETRY_DELAY_MS: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NavigationReadinessPolicy {
    Default,
    AllowCommittedShell,
}

impl HeadlessChromeHelper {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut launch_options = LaunchOptionsBuilder::default();
        launch_options
            .headless(true)
            .sandbox(false)
            .window_size(Some((1440, 2200)));
        if let Some(path) = detect_browser_executable() {
            launch_options.path(Some(path));
        }

        let browser = Browser::new(launch_options.build()?)?;
        Ok(Self { browser })
    }

    /// Navigate to URL and wait for page load
    pub fn navigate_and_wait(
        &self,
        url: &str,
        wait_ms: u64,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_wait(url, wait_ms, DESKTOP_PROFILE)
    }

    /// Navigate to URL and cap navigation readiness wait.
    pub fn navigate_and_wait_with_timeout(
        &self,
        url: &str,
        wait_ms: u64,
        navigation_timeout_ms: u64,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(
            url,
            wait_ms,
            DESKTOP_PROFILE,
            None,
            Some(navigation_timeout_ms),
            None,
        )
    }

    /// Navigate to URL with capped waits that respect an outer deadline.
    pub fn navigate_and_wait_with_timeout_and_deadline(
        &self,
        url: &str,
        wait_ms: u64,
        navigation_timeout_ms: u64,
        deadline: Instant,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(
            url,
            wait_ms,
            DESKTOP_PROFILE,
            None,
            Some(navigation_timeout_ms),
            Some(deadline),
        )
    }

    /// Navigate to URL with an explicit emulation profile.
    pub fn navigate_with_profile_and_wait(
        &self,
        url: &str,
        wait_ms: u64,
        profile: HeadlessProfile,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(url, wait_ms, profile, None, None, None)
    }

    /// Navigate to URL with profile and optional Referer header.
    pub fn navigate_with_profile_and_referer(
        &self,
        url: &str,
        wait_ms: u64,
        profile: HeadlessProfile,
        referer: Option<&str>,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(
            url, wait_ms, profile, referer, None, None,
        )
    }

    /// Navigate to URL with profile, referer, and capped navigation readiness wait.
    pub fn navigate_with_profile_and_referer_with_timeout(
        &self,
        url: &str,
        wait_ms: u64,
        profile: HeadlessProfile,
        referer: Option<&str>,
        navigation_timeout_ms: u64,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(
            url,
            wait_ms,
            profile,
            referer,
            Some(navigation_timeout_ms),
            None,
        )
    }

    /// Navigate to URL with profile, referer, timeout, and outer deadline.
    pub fn navigate_with_profile_and_referer_with_timeout_and_deadline(
        &self,
        url: &str,
        wait_ms: u64,
        profile: HeadlessProfile,
        referer: Option<&str>,
        navigation_timeout_ms: u64,
        deadline: Instant,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        self.navigate_with_profile_and_referer_and_timeout(
            url,
            wait_ms,
            profile,
            referer,
            Some(navigation_timeout_ms),
            Some(deadline),
        )
    }

    fn navigate_with_profile_and_referer_and_timeout(
        &self,
        url: &str,
        wait_ms: u64,
        profile: HeadlessProfile,
        referer: Option<&str>,
        navigation_timeout_ms: Option<u64>,
        deadline: Option<Instant>,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        let started = Instant::now();
        Self::ensure_navigation_start_budget(url, "open_tab", deadline)?;
        let tab = self.browser.new_tab()?;
        let _ = tab.set_user_agent(
            profile.user_agent,
            Some(profile.accept_language),
            Some(profile.platform),
        );
        if let Some(referer) = referer.filter(|value| !value.trim().is_empty()) {
            let mut headers = HashMap::new();
            headers.insert("Referer", referer);
            let _ = tab.set_extra_http_headers(headers);
        }
        let _ = tab.evaluate(&Self::build_profile_bootstrap_js(profile), false);
        let _ = tab.set_bounds(Bounds::Normal {
            left: None,
            top: None,
            width: Some(profile.viewport.0 as f64),
            height: Some(profile.viewport.1 as f64),
        });

        Self::ensure_navigation_start_budget(url, "navigate", deadline)?;
        debug!(url = url, "HeadlessChrome: navigating");
        tab.navigate_to(url)?;
        if let Some(timeout_ms) = navigation_timeout_ms {
            let effective_timeout_ms = Self::cap_with_deadline(timeout_ms, deadline);
            if effective_timeout_ms == 0 {
                return Err(format!(
                    "headless navigation readiness deadline elapsed before wait for {}",
                    url
                )
                .into());
            }
            Self::wait_for_navigation_readiness(&tab, url, effective_timeout_ms)?;
        } else {
            tab.wait_until_navigated()?;
        }
        let navigated_ms = started.elapsed().as_millis() as u64;

        let selector_wait_started = Instant::now();
        let selector_budget_ms = Self::cap_with_deadline(wait_ms.min(8_000), deadline);
        let selector_ready = selector_budget_ms > 0
            && Self::wait_for_any_selector(
                &tab,
                &[
                    "ww-feature-block-event-dsk",
                    "ww-feature-event-market-dsk",
                    "ww-feature-event-mini-card-dsk",
                    ".main-event",
                    ".card",
                    ".event-card",
                    "[id^='eventId-']",
                    "[data-test*='event']",
                    "[data-testid*='event']",
                    "[class*='event-card']",
                    "[class*='match-card']",
                    "a[href*='/stavki/event/']",
                    ".half__names .name",
                    ".competitor__name",
                    "[class*='team'] [class*='name']",
                    "[data-test*='coef']",
                    "[data-testid*='coef']",
                    "[class*='coef']",
                    "button[title]",
                    "[role='button']",
                ],
                selector_budget_ms,
            );
        let selector_wait_ms = selector_wait_started.elapsed().as_millis() as u64;

        // Wait additional time for lazy loading
        let post_wait_started = Instant::now();
        let post_wait_budget_ms =
            Self::cap_with_deadline(Self::post_wait_budget_ms(wait_ms, selector_ready), deadline);
        if post_wait_budget_ms > 0 {
            std::thread::sleep(Duration::from_millis(post_wait_budget_ms));
        }
        let post_wait_ms = post_wait_started.elapsed().as_millis() as u64;

        debug!(
            url = url,
            profile = profile.label,
            selector_ready = selector_ready,
            selector_budget_ms = selector_budget_ms,
            requested_wait_ms = wait_ms,
            effective_post_wait_ms = post_wait_budget_ms,
            navigation_ms = navigated_ms,
            selector_wait_ms = selector_wait_ms,
            post_wait_ms = post_wait_ms,
            total_ms = started.elapsed().as_millis() as u64,
            "HeadlessChrome: page loaded"
        );
        if !selector_ready {
            debug!(
                url = url,
                profile = profile.label,
                selector_budget_ms = selector_budget_ms,
                navigation_ms = navigated_ms,
                post_wait_ms = post_wait_ms,
                "HeadlessChrome: selector probe timed out before extraction"
            );
        }
        Ok(tab)
    }

    fn post_wait_budget_ms(wait_ms: u64, selector_ready: bool) -> u64 {
        if selector_ready {
            wait_ms.min(SELECTOR_READY_POST_WAIT_CAP_MS)
        } else {
            wait_ms.min(SELECTOR_MISS_POST_WAIT_CAP_MS)
        }
    }

    fn cap_with_deadline(budget_ms: u64, deadline: Option<Instant>) -> u64 {
        match deadline {
            Some(deadline) => budget_ms.min(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64,
            ),
            None => budget_ms,
        }
    }

    fn remaining_deadline_ms(deadline: Option<Instant>) -> Option<u64> {
        deadline.map(|deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis() as u64
        })
    }

    fn ensure_navigation_start_budget(
        url: &str,
        stage: &str,
        deadline: Option<Instant>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(remaining_budget_ms) = Self::remaining_deadline_ms(deadline) {
            if remaining_budget_ms < NAVIGATION_START_BUDGET_MS {
                return Err(format!(
                    "headless navigation start budget exhausted before {} for {} (remaining={}ms, required={}ms)",
                    stage, url, remaining_budget_ms, NAVIGATION_START_BUDGET_MS
                )
                .into());
            }
        }

        Ok(())
    }

    fn wait_for_navigation_readiness(
        tab: &headless_chrome::Tab,
        url: &str,
        timeout_ms: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));
        let readiness_policy = Self::navigation_readiness_policy(url);
        let readiness_js = r#"(() => ({
            href: window.location.href || '',
            readyState: document.readyState || '',
            bodyChildCount: Number(document.body?.children?.length || 0),
            bodyTextLength: Number((document.body?.innerText || document.body?.textContent || '').trim().length)
        }))()"#;

        loop {
            if let Some(state) = Self::evaluate_json(tab, readiness_js) {
                let href = state
                    .get("href")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let ready_state = state
                    .get("readyState")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let body_child_count = state
                    .get("bodyChildCount")
                    .and_then(|value| value.as_u64())
                    .unwrap_or_default();
                let body_text_length = state
                    .get("bodyTextLength")
                    .and_then(|value| value.as_u64())
                    .unwrap_or_default();

                let navigation_ready = Self::is_navigation_ready_for_policy(
                    readiness_policy,
                    ready_state,
                    href,
                    body_child_count,
                    body_text_length,
                );
                if navigation_ready {
                    return Ok(());
                }
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "headless navigation readiness timeout after {}ms for {}",
                    timeout_ms, url
                )
                .into());
            }

            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn navigation_readiness_policy(url: &str) -> NavigationReadinessPolicy {
        let normalized = url.trim_end_matches('/');
        match normalized {
            "https://winline.ru/stavki/sport/futbol" => {
                NavigationReadinessPolicy::AllowCommittedShell
            }
            _ if Self::is_melbet_canonical_shell_route(normalized) => {
                NavigationReadinessPolicy::AllowCommittedShell
            }
            _ if Self::is_melbet_sportsbook_shell_route(normalized) => {
                NavigationReadinessPolicy::AllowCommittedShell
            }
            _ => NavigationReadinessPolicy::Default,
        }
    }

    fn is_melbet_canonical_shell_route(url: &str) -> bool {
        matches!(
            url.to_ascii_lowercase().as_str(),
            "https://melbet.ru/live"
                | "https://melbet.ru/line"
                | "https://melbet.ru/m/live"
                | "https://melbet.ru/m/line"
                | "https://m.melbet.com/live"
                | "https://m.melbet.com/line"
        )
    }

    fn is_melbet_sportsbook_shell_route(url: &str) -> bool {
        let normalized = url.to_ascii_lowercase();
        normalized.starts_with("https://sport.melbet.ru/")
            && (normalized.contains("/partner/sportsbook/home")
                || normalized.contains("/sportsbook/home"))
    }

    fn is_navigation_ready_for_policy(
        policy: NavigationReadinessPolicy,
        ready_state: &str,
        href: &str,
        body_child_count: u64,
        body_text_length: u64,
    ) -> bool {
        let committed = matches!(ready_state, "interactive" | "complete")
            && href != "about:blank"
            && (!href.is_empty() || body_child_count > 0 || body_text_length > 0);

        match policy {
            NavigationReadinessPolicy::Default => {
                committed && (body_child_count > 0 || body_text_length > 0)
            }
            NavigationReadinessPolicy::AllowCommittedShell => committed,
        }
    }

    /// Capture lightweight session bootstrap details without intercepting transport.
    pub fn capture_session_bootstrap(tab: &headless_chrome::Tab) -> Option<Value> {
        Self::evaluate_json_with_retry(
            tab,
            r#"(() => {
                const storageToObject = (storage) => {
                    const result = {};
                    try {
                        for (let index = 0; index < storage.length; index += 1) {
                            const key = storage.key(index);
                            if (!key) continue;
                            result[key] = storage.getItem(key);
                        }
                    } catch (_) {}
                    return result;
                };

                const allScripts = Array.from(document.scripts || []);
                const scripts = allScripts
                    .map((script) => script.src || '')
                    .filter(Boolean)
                    .slice(0, 20);
                const inlineScriptCount = allScripts.filter((script) => !script.src).length;
                const resourceEntries = (() => {
                    try {
                        return performance.getEntriesByType('resource') || [];
                    } catch (_) {
                        return [];
                    }
                })();
                const pushHint = (list, seen, kind, value, source) => {
                    const normalizedValue = String(value || '').trim();
                    if (!normalizedValue) return;
                    const key = [kind, normalizedValue, source].join('|');
                    if (seen.has(key)) return;
                    seen.add(key);
                    list.push({ kind, value: normalizedValue, source });
                };
                const transportHints = [];
                const transportHintKeys = new Set();
                const bootstrapMarkers = [];
                const bootstrapMarkerKeys = new Set();
                const partnerConfig = window.$globalSettings?.partner || window.$P || {};
                const locationHref = String(window.location.href || '');
                const locationPath = String(window.location.pathname || '');
                const locationSearch = String(window.location.search || '');
                const locationHash = String(window.location.hash || '');
                const historyState = (() => {
                    try {
                        return window.history?.state || null;
                    } catch (_) {
                        return null;
                    }
                })();
                const pushBootstrapMarker = (value) => {
                    const normalizedValue = String(value || '').trim();
                    if (!normalizedValue || bootstrapMarkerKeys.has(normalizedValue)) return;
                    bootstrapMarkerKeys.add(normalizedValue);
                    bootstrapMarkers.push(normalizedValue);
                };
                const markRouteHint = (value, source) => {
                    const normalizedValue = String(value || '').trim();
                    if (!normalizedValue) return;
                    pushBootstrapMarker(`route:${source}:${normalizedValue}`);
                    const lower = normalizedValue.toLowerCase();
                    if (lower.includes('/live')) pushBootstrapMarker(`route_family:${source}:live`);
                    if (lower.includes('/line')) pushBootstrapMarker(`route_family:${source}:line`);
                    if (lower.includes('/ru/sport') || lower.includes('sport.melbet.ru')) {
                        pushBootstrapMarker(`route_family:${source}:sportsbook`);
                    }
                };
                markRouteHint(locationHref, 'href');
                markRouteHint(locationPath, 'path');
                markRouteHint(locationSearch, 'search');
                markRouteHint(locationHash, 'hash');
                markRouteHint(document.referrer || '', 'referrer');
                markRouteHint(historyState?.as || historyState?.url || historyState?.path, 'history');
                markRouteHint(historyState?.initialRoute?.path, 'history_initial_route');
                allScripts
                    .filter((script) => !script.src)
                    .slice(0, 12)
                    .forEach((script) => {
                        const content = String(script.textContent || '').toLowerCase();
                        if (!content) return;
                        if (content.includes('$globalsettings')) {
                            pushBootstrapMarker('inline:$globalSettings');
                        }
                        if (content.includes('window.$p') || content.includes('$p=')) {
                            pushBootstrapMarker('inline:$P');
                        }
                        if (content.includes('$httpapi')) {
                            pushBootstrapMarker('inline:$httpApi');
                        }
                        if (content.includes('partnerid')) {
                            pushBootstrapMarker('inline:partnerId');
                        }
                        if (content.includes('countrycode')) {
                            pushBootstrapMarker('inline:countryCode');
                        }
                        if (content.includes('langid')) {
                            pushBootstrapMarker('inline:langId');
                        }
                        if (content.includes('/ru/sport') || content.includes('sport.melbet.ru')) {
                            pushBootstrapMarker('inline:sportsbook_route');
                        }
                        if (content.includes('initialroute')) {
                            pushBootstrapMarker('inline:initialRoute');
                        }
                    });
                Array.from(document.querySelectorAll('a[href], iframe[src]'))
                    .slice(0, 16)
                    .forEach((node) => {
                        markRouteHint(
                            node.getAttribute('href') || node.getAttribute('src') || '',
                            node.tagName.toLowerCase()
                        );
                    });
                const shellSelectors = [
                    'ww-app-dsk',
                    'ww-feature-header-dsk',
                    'ww-feature-nav-bar-dsk',
                    'ww-feature-sport-menu-dsk',
                    'ww-feature-coupon-dsk',
                    '#root',
                    '#app',
                    '[data-reactroot]',
                    '[id="__next"]',
                    '[id="__nuxt"]'
                ];
                shellSelectors.forEach((selector) => {
                    try {
                        if (document.querySelector(selector)) {
                            pushBootstrapMarker(`shell:${selector}`);
                        }
                    } catch (_) {}
                });
                const sportsbookLinkCount = (() => {
                    try {
                        return document.querySelectorAll('a[href*="/live"], a[href*="/line"], a[href*="/ru/sport"], a[href*="sport.melbet.ru"]').length;
                    } catch (_) {
                        return 0;
                    }
                })();
                if (sportsbookLinkCount > 0) {
                    pushBootstrapMarker(`shell:route_links:${sportsbookLinkCount}`);
                }
                scripts.forEach((script) => {
                    const lower = script.toLowerCase();
                    if (lower.includes('ws') || lower.includes('socket')) {
                        pushHint(transportHints, transportHintKeys, 'script_transport_marker', script, 'script');
                    }
                    if (lower.includes('feed') || lower.includes('stream') || lower.includes('live')) {
                        pushHint(transportHints, transportHintKeys, 'script_feed_marker', script, 'script');
                    }
                });

                const resourceTimeline = resourceEntries
                    .slice(-25)
                    .map((entry) => ({
                        name: String(entry.name || ''),
                        initiatorType: String(entry.initiatorType || ''),
                        nextHopProtocol: String(entry.nextHopProtocol || ''),
                        transferSize: Number(entry.transferSize || 0),
                        durationMs: Math.round(Number(entry.duration || 0)),
                        startTimeMs: Math.round(Number(entry.startTime || 0)),
                        responseEndMs: Math.round(Number(entry.responseEnd || 0))
                    }));
                resourceTimeline.forEach((entry) => {
                    const lowerName = entry.name.toLowerCase();
                    const lowerProtocol = entry.nextHopProtocol.toLowerCase();
                    if (lowerName.startsWith('wss://') || lowerName.startsWith('ws://')) {
                        pushHint(transportHints, transportHintKeys, 'websocket_endpoint', entry.name, 'resource');
                    }
                    if (
                        lowerName.includes('/ws')
                        || lowerName.includes('socket')
                        || lowerName.includes('sockjs')
                        || lowerName.includes('signalr')
                    ) {
                        pushHint(transportHints, transportHintKeys, 'websocket_candidate', entry.name, 'resource');
                    }
                    if (
                        entry.initiatorType === 'fetch'
                        || entry.initiatorType === 'xmlhttprequest'
                        || lowerName.includes('/api/')
                        || lowerName.includes('feed')
                        || lowerName.includes('stream')
                    ) {
                        pushHint(transportHints, transportHintKeys, 'data_endpoint', entry.name, 'resource');
                    }
                    if (lowerProtocol.includes('h2') || lowerProtocol.includes('http/2')) {
                        pushHint(transportHints, transportHintKeys, 'http2_transport', entry.nextHopProtocol, 'resource');
                    }
                });

                const bodyText = (document.body && (document.body.innerText || document.body.textContent) || '')
                    .replace(/\s+/g, ' ')
                    .trim();
                const bodyTextLower = bodyText.toLowerCase();
                if (bodyTextLower.includes('socket.io') || bodyTextLower.includes('websocket')) {
                    pushHint(transportHints, transportHintKeys, 'body_transport_marker', bodyText.slice(0, 160), 'body_text');
                }

                const localStorageData = storageToObject(window.localStorage);
                const sessionStorageData = storageToObject(window.sessionStorage);
                const storageEntries = [
                    ...Object.entries(localStorageData),
                    ...Object.entries(sessionStorageData)
                ];
                storageEntries.forEach(([key, value]) => {
                    const lowerKey = String(key || '').toLowerCase();
                    const lowerValue = String(value || '').toLowerCase();
                    if (
                        lowerKey.includes('socket')
                        || lowerKey.includes('stream')
                        || lowerKey.includes('transport')
                        || lowerValue.includes('wss://')
                        || lowerValue.includes('ws://')
                    ) {
                        pushHint(transportHints, transportHintKeys, 'storage_transport_marker', `${key}=${String(value || '').slice(0, 120)}`, 'storage');
                    }
                });
                [
                    ['websocket_endpoint', partnerConfig.SportSocketAddress],
                    ['websocket_endpoint', partnerConfig.JackpotSocketAddress],
                    ['websocket_endpoint', partnerConfig.LiveCenterSocket],
                    ['websocket_endpoint', partnerConfig.SportFIWS],
                    ['websocket_endpoint', partnerConfig.SportFWS],
                    ['data_endpoint', partnerConfig.LiveCenterApi],
                    ['data_endpoint', partnerConfig.StatsUrl],
                    ['data_endpoint', partnerConfig.LiveScoreUrl],
                ].forEach(([kind, value]) => {
                    pushHint(transportHints, transportHintKeys, kind, value, 'partner_config');
                });
                if (window.$globalSettings) {
                    pushBootstrapMarker('runtime:$globalSettings');
                }
                if (window.$P) {
                    pushBootstrapMarker('runtime:$P');
                }
                if (window.$httpApi) {
                    pushBootstrapMarker('runtime:$httpApi');
                    pushHint(transportHints, transportHintKeys, 'script_transport_marker', 'window.$httpApi', 'runtime');
                }
                if (Number(window.$P?.Id || window.$globalSettings?.partner?.Id || 0) > 0) {
                    pushBootstrapMarker('runtime:partnerId');
                }
                if (Number(window.$globalSettings?.language?.Id || 0) > 0) {
                    pushBootstrapMarker('runtime:langId');
                }
                if (String(window.$globalSettings?.user?.CountryCode || '').trim()) {
                    pushBootstrapMarker('runtime:countryCode');
                }

                const navigationTiming = (() => {
                    try {
                        return performance.getEntriesByType('navigation')[0] || null;
                    } catch (_) {
                        return null;
                    }
                })();
                const lastResourceEndMs = resourceTimeline.reduce(
                    (max, entry) => Math.max(max, Number(entry.responseEndMs || 0)),
                    0
                );
                const fetchLikeCount = resourceTimeline.filter((entry) => {
                    const lowerName = entry.name.toLowerCase();
                    return (
                        entry.initiatorType === 'fetch'
                        || entry.initiatorType === 'xmlhttprequest'
                        || lowerName.includes('/api/')
                        || lowerName.includes('feed')
                    );
                }).length;
                const websocketHintCount = transportHints.filter((hint) => hint.kind.includes('websocket')).length;

                return {
                    url: window.location.href,
                    origin: window.location.origin || '',
                    path: window.location.pathname || '',
                    iframeSources: Array.from(document.querySelectorAll('iframe[src]'))
                        .map((frame) => String(frame.getAttribute('src') || '').trim())
                        .filter(Boolean)
                        .slice(0, 8),
                    title: document.title || '',
                    readyState: document.readyState || '',
                    referrer: document.referrer || '',
                    bodyTextSample: bodyText.slice(0, 500),
                    cookie: document.cookie || '',
                    htmlClassList: Array.from(document.documentElement?.classList || []),
                    bodyClassList: Array.from(document.body?.classList || []),
                    metaViewport: document.querySelector('meta[name="viewport"]')?.getAttribute('content') || '',
                    rootNodeIds: ['root', 'app', '__next', '__nuxt', 'application'].filter((id) => Boolean(document.getElementById(id))),
                    localStorage: localStorageData,
                    sessionStorage: sessionStorageData,
                    scriptSources: scripts,
                    resourceTimeline,
                    transportHints: transportHints.slice(0, 20),
                    runtimeContext: {
                        hasHttpApi: Boolean(window.$httpApi),
                        httpApiMethods: Object.keys(window.$httpApi || {})
                            .filter((key) => typeof window.$httpApi?.[key] === 'function')
                            .sort()
                            .slice(0, 16),
                        partnerId: Number(window.$P?.Id || window.$globalSettings?.partner?.Id || 0),
                        langId: Number(window.$globalSettings?.language?.Id || 0),
                        countryCode: String(window.$globalSettings?.user?.CountryCode || ''),
                        hasGlobalSettings: Boolean(window.$globalSettings),
                        hasPartnerConfig: Boolean(window.$globalSettings?.partner || window.$P),
                        inlineScriptCount,
                        bootstrapMarkers: bootstrapMarkers.slice(0, 8)
                    },
                    readinessDiagnostics: {
                        readyState: document.readyState || '',
                        bodyTextLength: bodyText.length,
                        bodyChildCount: Number(document.body?.children?.length || 0),
                        resourceCount: resourceEntries.length,
                        scriptCount: scripts.length,
                        storageKeyCount: Object.keys(localStorageData).length + Object.keys(sessionStorageData).length,
                        rootNodeCount: ['root', 'app', '__next', '__nuxt', 'application'].filter((id) => Boolean(document.getElementById(id))).length,
                        fetchLikeCount,
                        websocketHintCount,
                        domContentLoadedMs: Math.round(Number(navigationTiming?.domContentLoadedEventEnd || 0)),
                        loadEventMs: Math.round(Number(navigationTiming?.loadEventEnd || 0)),
                        lastResourceEndMs: Math.round(lastResourceEndMs),
                        hasVisibleAppShell: Boolean(document.body && bodyText.length > 0 && document.body.children.length > 0)
                    },
                    hasServiceWorker: Boolean(navigator.serviceWorker),
                    userAgent: navigator.userAgent || '',
                    maxTouchPoints: Number(navigator.maxTouchPoints || 0),
                    innerWidth: Number(window.innerWidth || 0),
                    innerHeight: Number(window.innerHeight || 0),
                    profileLabel: window.__kiloProfileLabel || '',
                    appMarker: window.__kiloAppMarker || ''
                };
            })()"#,
            BOOTSTRAP_CAPTURE_ATTEMPTS,
            BOOTSTRAP_CAPTURE_RETRY_DELAY_MS,
        )
    }

    fn build_profile_bootstrap_js(profile: HeadlessProfile) -> String {
        let touch_points = if profile.is_mobile { 5 } else { 0 };
        let app_marker = serde_json::to_string(profile.app_marker.unwrap_or_default())
            .unwrap_or_else(|_| "\"\"".to_string());

        format!(
            "(() => {{\
                try {{ window.__kiloProfileLabel = {label:?}; }} catch (_) {{}}\
                try {{ window.__kiloAppMarker = {app_marker}; }} catch (_) {{}}\
                try {{ Object.defineProperty(navigator, 'maxTouchPoints', {{ configurable: true, get: () => {touch_points} }}); }} catch (_) {{}}\
                try {{ Object.defineProperty(navigator, 'webdriver', {{ configurable: true, get: () => undefined }}); }} catch (_) {{}}\
                try {{ if (!window.chrome) {{ window.chrome = {{ runtime: {{}} }}; }} }} catch (_) {{}}\
            }})();",
            label = profile.label,
            app_marker = app_marker,
            touch_points = touch_points,
        )
    }

    /// Capture lightweight runtime route/DOM state for parser diagnostics.
    pub fn capture_runtime_state(tab: &headless_chrome::Tab) -> Option<Value> {
        Self::evaluate_json_with_retry(
            tab,
            r#"(() => {
                const count = (selector) => {
                    try {
                        return document.querySelectorAll(selector).length;
                    } catch (_) {
                        return 0;
                    }
                };
                const normalizeText = (value) => String(value || '').replace(/\s+/g, ' ').trim();
                const bootstrapMarkers = [];
                const bootstrapMarkerKeys = new Set();
                const pushBootstrapMarker = (value) => {
                    const normalizedValue = normalizeText(value);
                    if (!normalizedValue || bootstrapMarkerKeys.has(normalizedValue)) return;
                    bootstrapMarkerKeys.add(normalizedValue);
                    bootstrapMarkers.push(normalizedValue);
                };
                const markRouteHint = (value, source) => {
                    const normalizedValue = normalizeText(value);
                    if (!normalizedValue) return;
                    pushBootstrapMarker(`route:${source}:${normalizedValue}`);
                    const lower = normalizedValue.toLowerCase();
                    if (lower.includes('/live')) pushBootstrapMarker(`route_family:${source}:live`);
                    if (lower.includes('/line')) pushBootstrapMarker(`route_family:${source}:line`);
                    if (lower.includes('/ru/sport') || lower.includes('sport.melbet.ru') || lower.includes('sportsbook/home')) {
                        pushBootstrapMarker(`route_family:${source}:sportsbook`);
                    }
                };
                const detectBlocker = (href, title, bodyText) => {
                    const checks = [
                        { kind: 'captcha', pattern: /captcha|капча|каптча/i },
                        { kind: 'access_denied', pattern: /access denied|forbidden|доступ запрещен|доступ ограничен/i },
                        { kind: 'human_verification', pattern: /verify you are human|prove you are human|подтвердите, что вы человек|проверка браузера/i },
                        { kind: 'cloudflare_challenge', pattern: /cloudflare|just a moment|checking your browser/i },
                        { kind: 'ddos_guard', pattern: /ddos-guard|anti-ddos|anti ddos/i },
                    ];
                    const sources = [
                        ['title', normalizeText(title)],
                        ['body', normalizeText(bodyText)],
                        ['href', String(href || '')],
                    ];
                    for (const check of checks) {
                        for (const [source, value] of sources) {
                            if (!value || !check.pattern.test(value)) continue;
                            return {
                                kind: check.kind,
                                source,
                                matchedText: normalizeText(value).slice(0, 160),
                            };
                        }
                    }
                    return null;
                };
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

                const href = String(window.location.href || '');
                const title = String(document.title || '');
                const bodyText = normalizeText(document.body?.innerText || document.body?.textContent || '');
                const blocker = detectBlocker(href, title, bodyText);
                const historyState = (() => {
                    try {
                        return window.history?.state || null;
                    } catch (_) {
                        return null;
                    }
                })();
                markRouteHint(href, 'href');
                markRouteHint(window.location.pathname || '', 'path');
                markRouteHint(window.location.search || '', 'search');
                markRouteHint(window.location.hash || '', 'hash');
                markRouteHint(document.referrer || '', 'referrer');
                markRouteHint(historyState?.as || historyState?.url || historyState?.path, 'history');
                markRouteHint(historyState?.initialRoute?.path, 'history_initial_route');
                Array.from(document.querySelectorAll('iframe[src], a[href]'))
                    .slice(0, 12)
                    .forEach((node) => {
                        const source = node.tagName.toLowerCase();
                        const route = node.getAttribute('src') || node.getAttribute('href') || '';
                        markRouteHint(route, source);
                        if (source === 'iframe') {
                            const lower = String(route || '').toLowerCase();
                            if (lower.includes('sport.melbet.ru') || lower.includes('sportsbook/home')) {
                                pushBootstrapMarker('iframe:sportsbook');
                            }
                        }
                    });
                ['ww-app-dsk', 'ww-feature-header-dsk', 'ww-feature-nav-bar-dsk', 'ww-feature-sport-menu-dsk', 'ww-feature-coupon-dsk', '#root', '#app', '[data-reactroot]', '[id="__next"]', '[id="__nuxt"]']
                    .forEach((selector) => {
                        try {
                            if (document.querySelector(selector)) {
                                pushBootstrapMarker(`shell:${selector}`);
                            }
                        } catch (_) {}
                    });
                const sportsbookRouteLinkCount = count('a[href*="/live"], a[href*="/line"], a[href*="/ru/sport"], a[href*="sport.melbet.ru"], iframe[src*="sport.melbet.ru"], iframe[src*="SportsBook/Home"]');
                if (sportsbookRouteLinkCount > 0) {
                    pushBootstrapMarker(`shell:route_links:${sportsbookRouteLinkCount}`);
                }
                if (title.toLowerCase().includes('melbet')) {
                    pushBootstrapMarker('title:melbet');
                }
                if (bodyText.toLowerCase().includes('melbet') || bodyText.toLowerCase().includes('sport')) {
                    pushBootstrapMarker('body:sportsbook_text');
                }
                if (window.$globalSettings) {
                    pushBootstrapMarker('runtime:$globalSettings');
                }
                if (window.$P) {
                    pushBootstrapMarker('runtime:$P');
                }
                if (window.$httpApi) {
                    pushBootstrapMarker('runtime:$httpApi');
                }
                if (Number(window.$P?.Id || window.$globalSettings?.partner?.Id || 0) > 0) {
                    pushBootstrapMarker('runtime:partnerId');
                }
                if (Number(window.$globalSettings?.language?.Id || 0) > 0) {
                    pushBootstrapMarker('runtime:langId');
                }
                if (String(window.$globalSettings?.user?.CountryCode || '').trim()) {
                    pushBootstrapMarker('runtime:countryCode');
                }

                return {
                    href,
                    pathname: String(window.location.pathname || ''),
                    search: String(window.location.search || ''),
                    hash: String(window.location.hash || ''),
                    title,
                    readyState: String(document.readyState || ''),
                    historyLength: Number(window.history?.length || 0),
                    bodyChildCount: Number(document.body?.children?.length || 0),
                    bodyTextLength: Number(bodyText.length),
                    customElementCount: Number(Array.from(document.querySelectorAll('*')).filter((node) => String(node.tagName || '').includes('-')).length),
                    buttonCount: count('button, [role="button"]'),
                    interactiveNodeCount: count('button, [role="button"], a[href], input, select, textarea'),
                    linkCount: count('a[href]'),
                    routeLinkCount: count('a[href^="/live"], a[href^="/stavki/"]'),
                    routerShellCount: count('ww-app-dsk, ww-feature-header-dsk, ww-feature-nav-bar-dsk, ww-feature-sport-menu-dsk, ww-feature-coupon-dsk'),
                    firstButtonText: normalizeText(document.querySelector('button, [role="button"]')?.textContent || '').slice(0, 160),
                    bodyTextSample: bodyText.slice(0, 220),
                    navigationEntry,
                    blocker,
                    bootstrapMarkers: bootstrapMarkers.slice(0, 12),
                };
            })()"#,
            BOOTSTRAP_CAPTURE_ATTEMPTS,
            BOOTSTRAP_CAPTURE_RETRY_DELAY_MS,
        )
    }

    /// Execute JavaScript and return JSON value
    pub fn evaluate_json(tab: &headless_chrome::Tab, js: &str) -> Option<Value> {
        match tab.evaluate(js, false) {
            Ok(result) => result.value.clone(),
            Err(e) => {
                debug!(error = %e, "HeadlessChrome: JS evaluation failed");
                None
            }
        }
    }

    /// Execute JavaScript with a few retries for late SPA hydration.
    pub fn evaluate_json_with_retry(
        tab: &headless_chrome::Tab,
        js: &str,
        attempts: usize,
        delay_ms: u64,
    ) -> Option<Value> {
        for attempt in 0..attempts.max(1) {
            if let Some(value) = Self::evaluate_json(tab, js) {
                return Some(value);
            }

            if attempt + 1 < attempts.max(1) && delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }

        None
    }

    /// Execute async JavaScript and poll until it resolves.
    pub fn evaluate_async_json_with_retry(
        tab: &headless_chrome::Tab,
        js_body: &str,
        attempts: usize,
        delay_ms: u64,
    ) -> Option<Value> {
        let eval_key = format!(
            "__kiloAsyncEval{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_micros()
        );
        let status_js = format!(
            r#"(() => {{
                const key = {key:?};
                if (!window[key]) {{
                    window[key] = {{ status: "pending" }};
                    Promise.resolve()
                        .then(async () => {{
                            {js_body}
                        }})
                        .then((value) => {{
                            window[key] = {{ status: "resolved", value }};
                        }})
                        .catch((error) => {{
                            window[key] = {{
                                status: "rejected",
                                error: String(error && (error.stack || error.message) || error || "unknown async evaluation error")
                            }};
                        }});
                }}
                return window[key];
            }})()"#,
            key = eval_key,
            js_body = js_body,
        );

        for attempt in 0..attempts.max(1) {
            if let Some(state) = Self::evaluate_json(tab, &status_js) {
                match state.get("status").and_then(|value| value.as_str()) {
                    Some("resolved") => {
                        let value = state.get("value").cloned();
                        let cleanup_js = format!(
                            r#"(() => {{ try {{ delete window[{key:?}]; }} catch (_) {{}} return true; }})()"#,
                            key = eval_key,
                        );
                        let _ = Self::evaluate_json(tab, &cleanup_js);
                        return value;
                    }
                    Some("rejected") => {
                        debug!(
                            error = %state
                                .get("error")
                                .and_then(|value| value.as_str())
                                .unwrap_or("unknown async evaluation failure"),
                            "HeadlessChrome: async JS evaluation failed"
                        );
                        break;
                    }
                    _ => {}
                }
            }

            if attempt + 1 < attempts.max(1) && delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }

        let cleanup_js = format!(
            r#"(() => {{ try {{ delete window[{key:?}]; }} catch (_) {{}} return true; }})()"#,
            key = eval_key,
        );
        let _ = Self::evaluate_json(tab, &cleanup_js);
        None
    }

    /// Wait until any of the provided selectors appears.
    pub fn wait_for_any_selector(
        tab: &headless_chrome::Tab,
        selectors: &[&str],
        timeout_ms: u64,
    ) -> bool {
        let per_selector_ms = 125;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));

        while Instant::now() < deadline {
            for selector in selectors {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }

                let remaining = deadline.saturating_duration_since(now);
                let timeout = remaining.min(Duration::from_millis(per_selector_ms));
                if tab
                    .wait_for_element_with_custom_timeout(selector, timeout)
                    .is_ok()
                {
                    return true;
                }
            }
        }

        false
    }

    /// Scroll page to trigger lazy loading
    pub fn scroll_page(
        tab: &headless_chrome::Tab,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = Self::scroll_page_with_deadline(tab, None)?;
        Ok(())
    }

    /// Scroll page to trigger lazy loading without overrunning an outer deadline.
    pub fn scroll_page_with_deadline(
        tab: &headless_chrome::Tab,
        deadline: Option<Instant>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        tab.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)", false)?;
        let first_wait_ms = Self::cap_with_deadline(SCROLL_STEP_WAIT_MS, deadline);
        if first_wait_ms == 0 {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(first_wait_ms));
        tab.evaluate("window.scrollTo(0, document.body.scrollHeight)", false)?;
        let second_wait_ms = Self::cap_with_deadline(SCROLL_STEP_WAIT_MS, deadline);
        if second_wait_ms == 0 {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(second_wait_ms));
        Ok(true)
    }

    /// Extract all text content from page
    pub fn get_page_text(tab: &headless_chrome::Tab) -> Option<String> {
        tab.evaluate("document.body.innerText", false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(String::from))
    }
}

fn detect_browser_executable() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CHROME") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(path) = detect_playwright_chromium() {
        return Some(path);
    }

    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);

    [
        local_app_data
            .as_ref()
            .map(|base| base.join("Google/Chrome/Application/chrome.exe")),
        program_files
            .as_ref()
            .map(|base| base.join("Google/Chrome/Application/chrome.exe")),
        program_files_x86
            .as_ref()
            .map(|base| base.join("Google/Chrome/Application/chrome.exe")),
        program_files
            .as_ref()
            .map(|base| base.join("Microsoft/Edge/Application/msedge.exe")),
        program_files_x86
            .as_ref()
            .map(|base| base.join("Microsoft/Edge/Application/msedge.exe")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

fn detect_playwright_chromium() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)?
        .join("ms-playwright");
    let entries = std::fs::read_dir(&base).ok()?;
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("chromium-"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.reverse();

    candidates
        .into_iter()
        .map(|dir| dir.join(Path::new("chrome-win64/chrome.exe")))
        .find(|path| path.is_file())
}

/// Parse team name validation
pub fn is_valid_team_name(name: &str) -> bool {
    let name = name.trim();
    if name.len() < 2 || name.len() > 80 {
        return false;
    }

    let blacklist = [
        "футбол",
        "счёт",
        "live",
        "матч",
        "спорт",
        "total",
        "тотал",
        "статистика",
        "time",
        "vs",
        "team",
        "команда",
        "player",
        "игрок",
        "unknown",
        "неизвест",
        "match",
        "game",
        "event",
    ];

    let lower = name.to_lowercase();
    if blacklist.iter().any(|&b| lower.contains(b)) {
        return false;
    }

    // Reject purely numeric names
    if name.chars().all(|c| c.is_numeric() || c.is_whitespace()) {
        return false;
    }

    true
}

/// Extract odds from text (finds decimal numbers between 1.01 and 100)
pub fn extract_odds_from_text(text: &str) -> Vec<f64> {
    let mut odds = Vec::new();
    for word in text.split_whitespace() {
        if let Ok(val) = word.replace(',', ".").parse::<f64>() {
            if (1.01..=100.0).contains(&val) {
                odds.push(val);
            }
        }
    }
    odds
}

#[cfg(test)]
mod tests {
    use super::{
        HeadlessChromeHelper, NavigationReadinessPolicy, NAVIGATION_START_BUDGET_MS,
        SELECTOR_READY_POST_WAIT_CAP_MS,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn caps_post_wait_after_selector_readiness() {
        assert_eq!(
            HeadlessChromeHelper::post_wait_budget_ms(1_800, true),
            SELECTOR_READY_POST_WAIT_CAP_MS
        );
        assert_eq!(HeadlessChromeHelper::post_wait_budget_ms(300, true), 300);
    }

    #[test]
    fn keeps_full_post_wait_when_selector_probe_misses() {
        assert_eq!(HeadlessChromeHelper::post_wait_budget_ms(1_800, false), 600);
        assert_eq!(HeadlessChromeHelper::post_wait_budget_ms(300, false), 300);
    }

    #[test]
    fn deadline_cap_trims_budget_to_remaining_time() {
        let deadline = Instant::now() + Duration::from_millis(40);
        assert!(HeadlessChromeHelper::cap_with_deadline(500, Some(deadline)) <= 40);
        assert_eq!(HeadlessChromeHelper::cap_with_deadline(120, None), 120);
    }

    #[test]
    fn deadline_cap_drops_to_zero_after_deadline_passes() {
        let deadline = Instant::now() - Duration::from_millis(1);
        assert_eq!(
            HeadlessChromeHelper::cap_with_deadline(500, Some(deadline)),
            0
        );
    }

    #[test]
    fn navigation_start_budget_stays_open_with_enough_remaining_time() {
        let deadline = Instant::now() + Duration::from_millis(NAVIGATION_START_BUDGET_MS + 250);
        assert!(HeadlessChromeHelper::ensure_navigation_start_budget(
            "https://winline.ru/live",
            "navigate",
            Some(deadline)
        )
        .is_ok());
    }

    #[test]
    fn navigation_start_budget_rejects_late_navigation_steps() {
        let deadline =
            Instant::now() + Duration::from_millis(NAVIGATION_START_BUDGET_MS.saturating_sub(250));
        let error = HeadlessChromeHelper::ensure_navigation_start_budget(
            "https://winline.ru/live",
            "navigate",
            Some(deadline),
        )
        .expect_err("late navigation step should be rejected");

        assert!(error
            .to_string()
            .contains("headless navigation start budget exhausted before navigate"));
    }

    #[test]
    fn uses_shell_tolerant_readiness_policy_for_winline_football_discovery() {
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy(
                "https://winline.ru/stavki/sport/futbol/"
            ),
            NavigationReadinessPolicy::AllowCommittedShell
        );
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy("https://winline.ru/live"),
            NavigationReadinessPolicy::Default
        );
    }

    #[test]
    fn uses_shell_tolerant_readiness_policy_for_melbet_sportsbook_home() {
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
            ),
            NavigationReadinessPolicy::AllowCommittedShell
        );
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy("https://melbet.ru/live"),
            NavigationReadinessPolicy::AllowCommittedShell
        );
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy("https://melbet.ru/m/live"),
            NavigationReadinessPolicy::AllowCommittedShell
        );
        assert_eq!(
            HeadlessChromeHelper::navigation_readiness_policy("https://m.melbet.com/line"),
            NavigationReadinessPolicy::AllowCommittedShell
        );
    }

    #[test]
    fn shell_tolerant_readiness_accepts_committed_empty_shell() {
        assert!(HeadlessChromeHelper::is_navigation_ready_for_policy(
            NavigationReadinessPolicy::AllowCommittedShell,
            "interactive",
            "https://winline.ru/stavki/sport/futbol/",
            0,
            0,
        ));
        assert!(!HeadlessChromeHelper::is_navigation_ready_for_policy(
            NavigationReadinessPolicy::Default,
            "interactive",
            "https://winline.ru/stavki/sport/futbol/",
            0,
            0,
        ));
    }
}
