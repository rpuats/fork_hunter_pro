use crate::base::{BookmakerParser, ParserResult};
use crate::headless_helper::{HeadlessChromeHelper, HeadlessProfile};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, Url};
use shared::odds::OddsType;
use shared::{
    DiagnosticSeverity, Event, Odd, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage,
    Sport,
};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

const BOOKMAKER_SLUG: &str = "melbet";
const BASE_URL: &str = "https://melbet.ru";
const SPORTSBOOK_BASE_URL: &str = "https://sport.melbet.ru/";
const SPORTSBOOK_HOME_URL: &str =
    "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D";
const HEADLESS_WAIT_MS: u64 = 3_500;
const SPORTSBOOK_NAVIGATION_TIMEOUT_MS: u64 = 5_000;
const MELBET_RUNTIME_WALL_CLOCK_TIMEOUT_MS: u64 = 25_000;
const HEADLESS_RETRY_DELAY_MS: u64 = 1_500;
const HEADLESS_EVAL_ATTEMPTS: usize = 3;
const HEADLESS_SCROLL_ROUNDS: usize = 2;
const HEADLESS_ASYNC_EVAL_ATTEMPTS: usize = 18;
const SPORTSBOOK_SPORT_LIMIT: usize = 5;
const SPORTSBOOK_EVENT_LIMIT: usize = 20;
const SPORTSBOOK_HTTP_API_TIMEOUT_MS: u64 = 5_000;
const TRANSPORT_HINT_TIMELINE_LIMIT: usize = 8;
const TRANSPORT_HINT_NOTES: &[&str] = &[
    "Melbet transport groundwork captures performance resource timeline and passive transport hints only.",
    "No websocket interception, frame decoding, or runtime transport execution is enabled in this step.",
    "Mobile and webview shells can now report bootstrap-only readiness with resource-backed diagnostics.",
];

const DESKTOP_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const MOBILE_USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Mobile Safari/537.36";
const WEBVIEW_USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 13; Pixel 7 Build/TQ3A.230805.001; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/124.0.0.0 Mobile Safari/537.36";

const DESKTOP_PROFILE: HeadlessProfile = HeadlessProfile {
    label: "desktop",
    user_agent: DESKTOP_USER_AGENT,
    accept_language: "ru-RU,ru;q=0.9",
    platform: "Win32",
    viewport: (1440, 2200),
    is_mobile: false,
    app_marker: None,
};

const MOBILE_PROFILE: HeadlessProfile = HeadlessProfile {
    label: "mobile",
    user_agent: MOBILE_USER_AGENT,
    accept_language: "ru-RU,ru;q=0.9",
    platform: "Linux armv8l",
    viewport: (412, 915),
    is_mobile: true,
    app_marker: None,
};

const WEBVIEW_PROFILE: HeadlessProfile = HeadlessProfile {
    label: "webview",
    user_agent: WEBVIEW_USER_AGENT,
    accept_language: "ru-RU,ru;q=0.9",
    platform: "Linux armv8l",
    viewport: (412, 915),
    is_mobile: true,
    app_marker: Some("com.melbet.app"),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MelbetSurface {
    Desktop,
    Mobile,
    WebView,
}

impl MelbetSurface {
    fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::WebView => "webview",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct HeadlessProbe {
    url: &'static str,
    is_live: bool,
    surface: MelbetSurface,
    route_hint: &'static str,
    route_family: &'static str,
    profile: HeadlessProfile,
}

const HEADLESS_PROBES: &[HeadlessProbe] = &[
    HeadlessProbe {
        url: "https://melbet.ru/live",
        is_live: true,
        surface: MelbetSurface::Desktop,
        route_hint: "desktop-live",
        route_family: "canonical-live",
        profile: DESKTOP_PROFILE,
    },
    HeadlessProbe {
        url: "https://melbet.ru/line",
        is_live: false,
        surface: MelbetSurface::Desktop,
        route_hint: "desktop-line",
        route_family: "canonical-line",
        profile: DESKTOP_PROFILE,
    },
    HeadlessProbe {
        url: "https://melbet.ru/m/live",
        is_live: true,
        surface: MelbetSurface::Mobile,
        route_hint: "mobile-live",
        route_family: "mobile-shell-live",
        profile: MOBILE_PROFILE,
    },
    HeadlessProbe {
        url: "https://melbet.ru/m/line",
        is_live: false,
        surface: MelbetSurface::Mobile,
        route_hint: "mobile-line",
        route_family: "mobile-shell-line",
        profile: MOBILE_PROFILE,
    },
    HeadlessProbe {
        url: "https://m.melbet.com/live",
        is_live: true,
        surface: MelbetSurface::WebView,
        route_hint: "webview-live",
        route_family: "webview-shell-live",
        profile: WEBVIEW_PROFILE,
    },
    HeadlessProbe {
        url: "https://m.melbet.com/line",
        is_live: false,
        surface: MelbetSurface::WebView,
        route_hint: "webview-line",
        route_family: "webview-shell-line",
        profile: WEBVIEW_PROFILE,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum MelbetRouteStatus {
    Ready,
    BootstrapOnly,
    Blocked,
}

impl MelbetRouteStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::BootstrapOnly => "bootstrap_only",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone)]
struct MelbetTransportHint {
    kind: String,
    value: String,
    source: String,
}

impl MelbetTransportHint {
    fn from_value(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            kind: value.get("kind")?.as_str()?.to_string(),
            value: value.get("value")?.as_str()?.to_string(),
            source: value
                .get("source")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetNormalizedTransportHint {
    kind: String,
    family: String,
    source: String,
    normalized_value: String,
    host: String,
    path: String,
    protocol: String,
    confidence: String,
}

impl MelbetNormalizedTransportHint {
    fn as_summary(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.family, self.protocol, self.host, self.normalized_value
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": self.kind,
            "family": self.family,
            "source": self.source,
            "normalizedValue": self.normalized_value,
            "host": self.host,
            "path": self.path,
            "protocol": self.protocol,
            "confidence": self.confidence,
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetResourceTimelineEntry {
    name: String,
    initiator_type: String,
    next_hop_protocol: String,
    transfer_size: u64,
    duration_ms: u64,
    start_time_ms: u64,
    response_end_ms: u64,
}

impl MelbetResourceTimelineEntry {
    fn from_value(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            name: value.get("name")?.as_str()?.to_string(),
            initiator_type: value
                .get("initiatorType")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            next_hop_protocol: value
                .get("nextHopProtocol")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            transfer_size: value
                .get("transferSize")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            duration_ms: value
                .get("durationMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            start_time_ms: value
                .get("startTimeMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            response_end_ms: value
                .get("responseEndMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
        })
    }

    fn as_summary(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.initiator_type, self.next_hop_protocol, self.duration_ms, self.name
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "initiatorType": self.initiator_type,
            "nextHopProtocol": self.next_hop_protocol,
            "transferSize": self.transfer_size,
            "durationMs": self.duration_ms,
            "startTimeMs": self.start_time_ms,
            "responseEndMs": self.response_end_ms,
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetReadinessDiagnostics {
    ready_state: String,
    body_text_length: usize,
    body_child_count: usize,
    resource_count: usize,
    script_count: usize,
    storage_key_count: usize,
    root_node_count: usize,
    fetch_like_count: usize,
    websocket_hint_count: usize,
    dom_content_loaded_ms: u64,
    load_event_ms: u64,
    last_resource_end_ms: u64,
    has_visible_app_shell: bool,
}

#[derive(Debug, Clone, Default)]
struct MelbetRuntimeState {
    href: String,
    pathname: String,
    search: String,
    hash: String,
    title: String,
    ready_state: String,
    history_length: usize,
    body_child_count: usize,
    body_text_length: usize,
    custom_element_count: usize,
    button_count: usize,
    interactive_node_count: usize,
    link_count: usize,
    route_link_count: usize,
    router_shell_count: usize,
    first_button_text: String,
    body_text_sample: String,
    navigation_type: String,
    dom_content_loaded_ms: u64,
    load_ms: u64,
    blocker_kind: String,
    blocker_source: String,
    blocker_text: String,
    bootstrap_markers: Vec<String>,
}

impl MelbetRuntimeState {
    fn from_value(value: &serde_json::Value) -> Self {
        let blocker = value.get("blocker").unwrap_or(&serde_json::Value::Null);

        Self {
            href: value
                .get("href")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            pathname: value
                .get("pathname")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            search: value
                .get("search")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            hash: value
                .get("hash")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            title: value
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            ready_state: value
                .get("readyState")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            history_length: value
                .get("historyLength")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            body_child_count: value
                .get("bodyChildCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            body_text_length: value
                .get("bodyTextLength")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            custom_element_count: value
                .get("customElementCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            button_count: value
                .get("buttonCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            interactive_node_count: value
                .get("interactiveNodeCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            link_count: value
                .get("linkCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            route_link_count: value
                .get("routeLinkCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            router_shell_count: value
                .get("routerShellCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            first_button_text: value
                .get("firstButtonText")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            body_text_sample: value
                .get("bodyTextSample")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            navigation_type: value
                .get("navigationEntry")
                .and_then(|value| value.get("type"))
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            dom_content_loaded_ms: value
                .get("navigationEntry")
                .and_then(|value| value.get("domContentLoadedMs"))
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            load_ms: value
                .get("navigationEntry")
                .and_then(|value| value.get("loadMs"))
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            blocker_kind: blocker
                .get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            blocker_source: blocker
                .get("source")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            blocker_text: blocker
                .get("matchedText")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            bootstrap_markers: value
                .get("bootstrapMarkers")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        }
    }

    fn as_summary(&self) -> String {
        format!(
            "href={},path={},search={},hash={},state={},history={},shells={},route_links={},links={},buttons={},interactive_nodes={},custom_elements={},body_children={},body_len={},nav_type={},dcl_ms={},load_ms={},blocker={}@{}:{},bootstrap_markers={}",
            if self.href.is_empty() { "none" } else { self.href.as_str() },
            if self.pathname.is_empty() { "none" } else { self.pathname.as_str() },
            if self.search.is_empty() { "none" } else { self.search.as_str() },
            if self.hash.is_empty() { "none" } else { self.hash.as_str() },
            if self.ready_state.is_empty() { "unknown" } else { self.ready_state.as_str() },
            self.history_length,
            self.router_shell_count,
            self.route_link_count,
            self.link_count,
            self.button_count,
            self.interactive_node_count,
            self.custom_element_count,
            self.body_child_count,
            self.body_text_length,
            if self.navigation_type.is_empty() { "none" } else { self.navigation_type.as_str() },
            self.dom_content_loaded_ms,
            self.load_ms,
            if self.blocker_kind.is_empty() { "none" } else { self.blocker_kind.as_str() },
            if self.blocker_source.is_empty() { "none" } else { self.blocker_source.as_str() },
            if self.blocker_text.is_empty() { "-" } else { self.blocker_text.as_str() },
            if self.bootstrap_markers.is_empty() {
                "none".to_string()
            } else {
                self.bootstrap_markers.join("|")
            },
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "href": self.href,
            "pathname": self.pathname,
            "search": self.search,
            "hash": self.hash,
            "title": self.title,
            "readyState": self.ready_state,
            "historyLength": self.history_length,
            "bodyChildCount": self.body_child_count,
            "bodyTextLength": self.body_text_length,
            "customElementCount": self.custom_element_count,
            "buttonCount": self.button_count,
            "interactiveNodeCount": self.interactive_node_count,
            "linkCount": self.link_count,
            "routeLinkCount": self.route_link_count,
            "routerShellCount": self.router_shell_count,
            "firstButtonText": self.first_button_text,
            "bodyTextSample": self.body_text_sample,
            "navigationEntry": {
                "type": self.navigation_type,
                "domContentLoadedMs": self.dom_content_loaded_ms,
                "loadMs": self.load_ms,
            },
            "blocker": {
                "kind": self.blocker_kind,
                "source": self.blocker_source,
                "matchedText": self.blocker_text,
            },
            "bootstrapMarkers": self.bootstrap_markers,
        })
    }

    fn has_sportsbook_shell_markers(&self) -> bool {
        self.router_shell_count > 0
            || self.route_link_count > 0
            || self.custom_element_count > 0
            || self.bootstrap_markers.iter().any(|marker| {
                let marker = marker.to_lowercase();
                marker.contains("route_family:") && marker.contains("sportsbook")
                    || marker.contains("iframe:sportsbook")
                    || marker.contains("shell:")
            })
            || self.pathname.eq_ignore_ascii_case("/ru/sport")
            || self.pathname.eq_ignore_ascii_case("/ru/sport/")
            || self.href.to_lowercase().contains("/ru/sport")
    }

    fn has_bootstrap_markers(&self) -> bool {
        !self.bootstrap_markers.is_empty()
            || self.has_sportsbook_shell_markers()
            || self.body_child_count > 0
            || self.body_text_length > 0
            || self.interactive_node_count > 0
            || self.link_count > 0
            || self.button_count > 0
            || matches!(self.ready_state.as_str(), "interactive" | "complete")
    }

    fn has_runtime_blocker(&self) -> bool {
        !self.blocker_kind.is_empty()
    }

    fn blocker_code(&self) -> String {
        if self.blocker_kind.is_empty() {
            return String::new();
        }

        let mut code = format!("runtime_blocker:{}", self.blocker_kind);
        if !self.blocker_source.is_empty() {
            code.push('@');
            code.push_str(&self.blocker_source);
        }
        code
    }
}

impl MelbetReadinessDiagnostics {
    fn from_value(value: &serde_json::Value) -> Self {
        Self {
            ready_state: value
                .get("readyState")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            body_text_length: value
                .get("bodyTextLength")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            body_child_count: value
                .get("bodyChildCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            resource_count: value
                .get("resourceCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            script_count: value
                .get("scriptCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            storage_key_count: value
                .get("storageKeyCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            root_node_count: value
                .get("rootNodeCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            fetch_like_count: value
                .get("fetchLikeCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            websocket_hint_count: value
                .get("websocketHintCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            dom_content_loaded_ms: value
                .get("domContentLoadedMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            load_event_ms: value
                .get("loadEventMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            last_resource_end_ms: value
                .get("lastResourceEndMs")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            has_visible_app_shell: value
                .get("hasVisibleAppShell")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        }
    }

    fn as_summary(&self) -> String {
        format!(
            "state={},resources={},fetch_like={},ws_hints={},scripts={},storage_keys={},roots={},body_children={},dcl_ms={},load_ms={},last_res_ms={},visible_shell={},body_len={}",
            self.ready_state,
            self.resource_count,
            self.fetch_like_count,
            self.websocket_hint_count,
            self.script_count,
            self.storage_key_count,
            self.root_node_count,
            self.body_child_count,
            self.dom_content_loaded_ms,
            self.load_event_ms,
            self.last_resource_end_ms,
            self.has_visible_app_shell,
            self.body_text_length,
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "readyState": self.ready_state,
            "resourceCount": self.resource_count,
            "scriptCount": self.script_count,
            "storageKeyCount": self.storage_key_count,
            "rootNodeCount": self.root_node_count,
            "fetchLikeCount": self.fetch_like_count,
            "websocketHintCount": self.websocket_hint_count,
            "domContentLoadedMs": self.dom_content_loaded_ms,
            "loadEventMs": self.load_event_ms,
            "lastResourceEndMs": self.last_resource_end_ms,
            "hasVisibleAppShell": self.has_visible_app_shell,
            "bodyTextLength": self.body_text_length,
            "bodyChildCount": self.body_child_count,
        })
    }
}

#[derive(Debug, Clone, Default)]
struct MelbetRuntimeContext {
    has_http_api: bool,
    http_api_methods: Vec<String>,
    partner_id: u64,
    lang_id: u64,
    country_code: String,
    has_global_settings: bool,
    has_partner_config: bool,
    inline_script_count: usize,
    bootstrap_markers: Vec<String>,
}

impl MelbetRuntimeContext {
    fn from_value(value: &serde_json::Value) -> Self {
        Self {
            has_http_api: value
                .get("hasHttpApi")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            http_api_methods: value
                .get("httpApiMethods")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            partner_id: value
                .get("partnerId")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            lang_id: value
                .get("langId")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            country_code: value
                .get("countryCode")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            has_global_settings: value
                .get("hasGlobalSettings")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            has_partner_config: value
                .get("hasPartnerConfig")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            inline_script_count: value
                .get("inlineScriptCount")
                .and_then(|value| value.as_u64())
                .unwrap_or_default() as usize,
            bootstrap_markers: value
                .get("bootstrapMarkers")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        }
    }

    fn as_summary(&self) -> String {
        format!(
            "http_api={},methods={},partner_id={},lang_id={},country={},globals={},partner_cfg={},inline_scripts={},bootstrap_markers={}",
            self.has_http_api,
            if self.http_api_methods.is_empty() {
                "none".to_string()
            } else {
                self.http_api_methods.join("|")
            },
            self.partner_id,
            self.lang_id,
            if self.country_code.trim().is_empty() {
                "none"
            } else {
                self.country_code.as_str()
            },
            self.has_global_settings,
            self.has_partner_config,
            self.inline_script_count,
            if self.bootstrap_markers.is_empty() {
                "none".to_string()
            } else {
                self.bootstrap_markers.join("|")
            },
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "hasHttpApi": self.has_http_api,
            "httpApiMethods": self.http_api_methods,
            "partnerId": self.partner_id,
            "langId": self.lang_id,
            "countryCode": self.country_code,
            "hasGlobalSettings": self.has_global_settings,
            "hasPartnerConfig": self.has_partner_config,
            "inlineScriptCount": self.inline_script_count,
            "bootstrapMarkers": self.bootstrap_markers,
        })
    }

    fn has_bootstrap_source_markers(&self) -> bool {
        self.has_global_settings
            || self.has_partner_config
            || self.inline_script_count > 0
            || !self.bootstrap_markers.is_empty()
    }

    fn missing_http_api_requirements(&self, is_live: bool) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.has_http_api {
            missing.push("http_api_runtime");
        }
        if self.partner_id == 0 {
            missing.push("partner_id");
        }
        if self.lang_id == 0 {
            missing.push("lang_id");
        }
        if self.country_code.trim().is_empty() {
            missing.push("country_code");
        }

        let required_methods = if is_live {
            ["getTopLiveSports", "getTopLiveEvents"].as_slice()
        } else {
            ["getPrematchSports", "getTopEvents"].as_slice()
        };
        for method in required_methods {
            if !self.http_api_methods.iter().any(|item| item == method) {
                missing.push(method);
            }
        }

        missing
    }

    fn http_api_blocker(&self, is_live: bool) -> String {
        let missing = self.missing_http_api_requirements(is_live);
        if missing.is_empty() {
            "http_api_context_unconfirmed".to_string()
        } else if missing.len() == 1 && missing[0] == "http_api_runtime" {
            if self.has_bootstrap_source_markers() {
                "no_http_api_runtime:additional_bootstrap_source_required".to_string()
            } else {
                "no_http_api_runtime".to_string()
            }
        } else {
            let suffix = if self.has_bootstrap_source_markers() {
                ":additional_bootstrap_source_required"
            } else {
                ""
            };
            format!("missing_http_api_context:{}{}", missing.join("|"), suffix)
        }
    }
}

#[derive(Debug, Clone)]
struct MelbetTransportMappingSummary {
    families: Vec<String>,
    hosts: Vec<String>,
    protocols: Vec<String>,
    high_confidence_count: usize,
    websocket_like_count: usize,
    feed_like_count: usize,
}

impl MelbetTransportMappingSummary {
    fn as_summary(&self) -> String {
        format!(
            "families={},hosts={},protocols={},high_conf={},ws_like={},feed_like={}",
            if self.families.is_empty() {
                "none".to_string()
            } else {
                self.families.join("|")
            },
            if self.hosts.is_empty() {
                "none".to_string()
            } else {
                self.hosts.join("|")
            },
            if self.protocols.is_empty() {
                "none".to_string()
            } else {
                self.protocols.join("|")
            },
            self.high_confidence_count,
            self.websocket_like_count,
            self.feed_like_count,
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "families": self.families,
            "hosts": self.hosts,
            "protocols": self.protocols,
            "highConfidenceCount": self.high_confidence_count,
            "websocketLikeCount": self.websocket_like_count,
            "feedLikeCount": self.feed_like_count,
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetReadinessOutput {
    state: String,
    reason: String,
    route_status: String,
    bootstrap_score: usize,
    blocker: String,
    confirmed_blocker: String,
    next_step: String,
    transport_mapping: MelbetTransportMappingSummary,
}

impl MelbetReadinessOutput {
    fn as_summary(&self) -> String {
        format!(
            "state={},reason={},route_status={},bootstrap_score={},blocker={},confirmed_blocker={},next_step={},{}",
            self.state,
            self.reason,
            self.route_status,
            self.bootstrap_score,
            self.blocker,
            self.confirmed_blocker,
            self.next_step,
            self.transport_mapping.as_summary(),
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "state": self.state,
            "reason": self.reason,
            "routeStatus": self.route_status,
            "bootstrapScore": self.bootstrap_score,
            "blocker": self.blocker,
            "confirmedBlocker": self.confirmed_blocker,
            "nextStep": self.next_step,
            "transportMapping": self.transport_mapping.as_json(),
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetBootstrapAcquisitionPlan {
    blocker: String,
    confirmed_blocker: String,
    next_step: String,
    primary_target: String,
    referer: String,
    route_candidates: Vec<String>,
    required_runtime_fields: Vec<String>,
    bootstrap_markers: Vec<String>,
}

impl MelbetBootstrapAcquisitionPlan {
    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "blocker": self.blocker,
            "confirmedBlocker": self.confirmed_blocker,
            "nextStep": self.next_step,
            "primaryTarget": self.primary_target,
            "referer": self.referer,
            "routeCandidates": self.route_candidates,
            "requiredRuntimeFields": self.required_runtime_fields,
            "bootstrapMarkers": self.bootstrap_markers,
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetBootstrapSnapshot {
    final_url: String,
    origin: String,
    path: String,
    referrer: String,
    iframe_sources: Vec<String>,
    title: String,
    body_text_sample: String,
    cookie: String,
    local_storage_keys: Vec<String>,
    session_storage_keys: Vec<String>,
    html_class_list: Vec<String>,
    body_class_list: Vec<String>,
    root_node_ids: Vec<String>,
    meta_viewport: String,
    script_sources: Vec<String>,
    user_agent: String,
    profile_label: String,
    app_marker: String,
    max_touch_points: u64,
    inner_width: u64,
    inner_height: u64,
    has_service_worker: bool,
    resource_timeline: Vec<MelbetResourceTimelineEntry>,
    transport_hints: Vec<MelbetTransportHint>,
    runtime_context: MelbetRuntimeContext,
    readiness: MelbetReadinessDiagnostics,
    runtime_state: MelbetRuntimeState,
}

impl MelbetBootstrapSnapshot {
    fn from_value(value: &serde_json::Value) -> Self {
        let extract_keys = |field: &str| {
            value
                .get(field)
                .and_then(|value| value.as_object())
                .map(|map| {
                    let mut keys = map.keys().cloned().collect::<Vec<_>>();
                    keys.sort();
                    keys
                })
                .unwrap_or_default()
        };
        let extract_string_array = |field: &str| {
            value
                .get(field)
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let extract_resource_timeline = || {
            value
                .get("resourceTimeline")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(MelbetResourceTimelineEntry::from_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let extract_transport_hints = || {
            value
                .get("transportHints")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(MelbetTransportHint::from_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        Self {
            final_url: value
                .get("url")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            origin: value
                .get("origin")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            path: value
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            referrer: value
                .get("referrer")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            iframe_sources: extract_string_array("iframeSources"),
            title: value
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            body_text_sample: value
                .get("bodyTextSample")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            cookie: value
                .get("cookie")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            local_storage_keys: extract_keys("localStorage"),
            session_storage_keys: extract_keys("sessionStorage"),
            html_class_list: extract_string_array("htmlClassList"),
            body_class_list: extract_string_array("bodyClassList"),
            root_node_ids: extract_string_array("rootNodeIds"),
            meta_viewport: value
                .get("metaViewport")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            script_sources: extract_string_array("scriptSources"),
            user_agent: value
                .get("userAgent")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            profile_label: value
                .get("profileLabel")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            app_marker: value
                .get("appMarker")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            max_touch_points: value
                .get("maxTouchPoints")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            inner_width: value
                .get("innerWidth")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            inner_height: value
                .get("innerHeight")
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            has_service_worker: value
                .get("hasServiceWorker")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            resource_timeline: extract_resource_timeline(),
            transport_hints: extract_transport_hints(),
            runtime_context: MelbetRuntimeContext::from_value(
                value
                    .get("runtimeContext")
                    .unwrap_or(&serde_json::Value::Null),
            ),
            readiness: MelbetReadinessDiagnostics::from_value(
                value
                    .get("readinessDiagnostics")
                    .unwrap_or(&serde_json::Value::Null),
            ),
            runtime_state: MelbetRuntimeState::from_value(
                value
                    .get("runtimeState")
                    .unwrap_or(&serde_json::Value::Null),
            ),
        }
    }

    fn storage_key_count(&self) -> usize {
        self.local_storage_keys.len() + self.session_storage_keys.len()
    }

    fn script_count(&self) -> usize {
        self.script_sources.len()
    }

    fn root_node_count(&self) -> usize {
        self.root_node_ids.len()
    }

    fn transport_hint_count(&self) -> usize {
        self.transport_hints.len()
    }

    fn iframe_source_count(&self) -> usize {
        self.iframe_sources.len()
    }

    fn resource_timeline_count(&self) -> usize {
        self.resource_timeline.len()
    }

    fn normalized_transport_hints(&self) -> Vec<MelbetNormalizedTransportHint> {
        let mut normalized = self
            .transport_hints
            .iter()
            .map(MelbetParser::normalize_transport_hint)
            .collect::<Vec<_>>();

        normalized.sort_by(|left, right| {
            left.family
                .cmp(&right.family)
                .then(left.host.cmp(&right.host))
                .then(left.normalized_value.cmp(&right.normalized_value))
        });
        normalized.dedup_by(|left, right| {
            left.family == right.family
                && left.host == right.host
                && left.normalized_value == right.normalized_value
        });
        normalized
    }

    fn looks_like_blocked_runtime(&self) -> bool {
        self.runtime_state.has_runtime_blocker() || MelbetParser::looks_like_block_page(self)
    }
}

#[derive(Debug, Clone)]
struct MelbetRouteProbeResult {
    probe: HeadlessProbe,
    status: MelbetRouteStatus,
    payload_len: usize,
    bootstrap: MelbetBootstrapSnapshot,
    extraction: MelbetExtractionDiagnostics,
}

#[derive(Debug, Clone, Default)]
struct MelbetExtractionDiagnostics {
    source: String,
    dom_payload_len: usize,
    embedded_route: String,
    embedded_payload_len: usize,
    sportsbook_route: String,
    http_api_seed_count: usize,
    http_api_payload_len: usize,
    blocker: String,
}

impl MelbetExtractionDiagnostics {
    fn as_summary(&self) -> String {
        format!(
            "source={},blocker={},dom={},embedded={},http_api={},seeds={}",
            if self.source.is_empty() {
                "none"
            } else {
                self.source.as_str()
            },
            if self.blocker.is_empty() {
                "unknown"
            } else {
                self.blocker.as_str()
            },
            self.dom_payload_len,
            self.embedded_payload_len,
            self.http_api_payload_len,
            self.http_api_seed_count,
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source,
            "domPayloadLen": self.dom_payload_len,
            "embeddedRoute": self.embedded_route,
            "embeddedPayloadLen": self.embedded_payload_len,
            "sportsbookRoute": self.sportsbook_route,
            "httpApiSeedCount": self.http_api_seed_count,
            "httpApiPayloadLen": self.http_api_payload_len,
            "blocker": self.blocker,
        })
    }
}

#[derive(Debug, Clone, Default)]
struct MelbetSportsbookHttpApiAttempt {
    payload: Vec<serde_json::Value>,
    bootstrap: Option<MelbetBootstrapSnapshot>,
    route: String,
    seed_count: usize,
    blocker: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MelbetRouteMatrixCounts {
    ready: usize,
    bootstrap_only: usize,
    blocked: usize,
}

impl MelbetRouteMatrixCounts {
    fn as_summary(&self) -> String {
        format!(
            "ready={},bootstrap_only={},blocked={}",
            self.ready, self.bootstrap_only, self.blocked
        )
    }
}

#[derive(Debug, Clone)]
struct MelbetSportApiContext {
    partner_id: u64,
    lang_id: u64,
    country_code: String,
}

#[derive(Debug, Clone)]
struct MelbetSportSeed {
    sport_id: u64,
    event_count: usize,
}

const HEADLESS_EXTRACT_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    const parseOdds = (value) => {
        const normalized = normalizeText(value).replace(',', '.');
        if (!normalized) return null;
        const parsed = Number.parseFloat(normalized);
        return Number.isFinite(parsed) && parsed >= 1.01 && parsed <= 100 ? parsed : null;
    };
    const parseOddsFromHref = (value) => {
        const href = String(value || '');
        const match = href.match(/addStake\([^,]+,[^,]+,\s*([0-9]+(?:\.[0-9]+)?)/i);
        if (!match) return null;
        return parseOdds(match[1]);
    };
    const isValidName = (value) => {
        const name = normalizeText(value);
        if (!name || name.length < 2 || name.length > 80) return false;
        if (/^[\d\s:.-]+$/.test(name)) return false;
        if (/^(live|prematch|line|match|event|game|sport|all sports|p1|p2|x|1|2|1x|x2|12|more|e\w+|head to head)$/i.test(name)) return false;
        if (/^(home|away|draw|yes|no)$/i.test(name)) return false;
        return true;
    };
    const isDateLike = (value) => /^\d{1,2}\.\d{1,2}(\.\d{2,4})?(\d{1,2}:\d{2})?$/.test(value);
    const isScoreLike = (value) => /^\d+[:.-]\d+(\s*\(\d+[:.-]\d+\))?$/.test(value);
    const isNoiseLine = (value) => {
        const line = normalizeText(value);
        if (!line) return true;
        if (isDateLike(line) || isScoreLike(line)) return true;
        if (/^\(?\d+\)?$/.test(line)) return true;
        if (/^(p1|p2|x|1|2|1x|x2|12|more|e\w+|live|event|match|calendar|results|tournament|statistics)$/i.test(line)) return true;
        return false;
    };
    const splitTeams = (value) => {
        const text = normalizeText(value);
        for (const separator of [' - ', ' – ', ' — ', ' vs ', ' VS ', '\n']) {
            if (text.includes(separator)) {
                const parts = text.split(separator).map(normalizeText).filter(Boolean);
                if (parts.length >= 2 && isValidName(parts[0]) && isValidName(parts[1])) {
                    return [parts[0], parts[1]];
                }
            }
        }
        return null;
    };
    const pickFirstText = (root, selectors) => {
        for (const selector of selectors) {
            const node = root.querySelector(selector);
            const text = normalizeText(node && (node.getAttribute('title') || node.textContent));
            if (text) return text;
        }
        return '';
    };
    const extractOdds = (node) => {
        const odds = [];
        const seen = new Set();
        node.querySelectorAll('a[href*="addStake"], button, [role="button"], span, div').forEach((child) => {
            const parsed = parseOddsFromHref(child.getAttribute && child.getAttribute('href')) || parseOdds(child.textContent || '');
            if (parsed === null) return;
            const key = parsed.toFixed(2);
            if (seen.has(key)) return;
            seen.add(key);
            if (odds.length < 6) odds.push(parsed);
        });
        return odds;
    };
    const extractNames = (text, names) => {
        if (names.length >= 2) return [names[0], names[1]];

        const lines = text
            .split(/\n+/)
            .map(normalizeText)
            .filter((line) => line && !isNoiseLine(line));

        for (const line of lines) {
            const split = splitTeams(line);
            if (split) return split;
        }

        const candidates = lines.filter((line) => isValidName(line));
        for (let index = 0; index + 1 < candidates.length; index += 1) {
            const home = candidates[index];
            const away = candidates[index + 1];
            if (home !== away) return [home, away];
        }

        if (names.length === 1) {
            const split = splitTeams(names[0]);
            if (split) return split;
        }

        return null;
    };
    const extractLeague = (node, text, home, away) => {
        const direct = pickFirstText(node, [
            '[class*="league"]',
            '[class*="champ"]',
            '[class*="tournament"]',
            '[class*="category"]',
            '[class*="competition"]'
        ]);
        if (direct) return direct;

        const lines = text
            .split(/\n+/)
            .map(normalizeText)
            .filter((line) => line && !isNoiseLine(line));
        const homeIndex = lines.findIndex((line) => line === home);
        const awayIndex = lines.findIndex((line) => line === away);
        const teamIndex = [homeIndex, awayIndex].filter((index) => index >= 0).sort((a, b) => a - b)[0];
        if (teamIndex > 0) {
            const candidate = lines[teamIndex - 1];
            if (candidate && candidate !== home && candidate !== away && !splitTeams(candidate)) {
                return candidate;
            }
        }

        return '';
    };
    const extractSport = (node, league, text) => {
        return pickFirstText(node, [
            '[class*="sport"]',
            '[data-sport-name]',
            '[class*="discipline"]'
        ]) || league || text;
    };
    const registerCandidate = (candidates, seenNodes, node) => {
        if (!node || seenNodes.has(node)) return;
        if (!node.children || node.children.length === 0) return;
        seenNodes.add(node);
        candidates.push(node);
    };
    const closestStakeContainer = (node) => {
        let current = node;
        while (current && current !== document.body) {
            const text = normalizeText(current.innerText || current.textContent || '');
            const stakeCount = current.querySelectorAll ? current.querySelectorAll('a[href*="addStake"]').length : 0;
            if (stakeCount >= 2 && text.length >= 20 && text.length <= 900) {
                return current;
            }
            current = current.parentElement;
        }
        return null;
    };
    const results = [];
    const seen = new Set();
    const selectors = [
        '[data-event-id]',
        '[data-id][class*="event"]',
        'a[href*="addStake"]',
        '[class*="event"]',
        '[class*="match"]',
        '[class*="game"]',
        '[class*="coupon"]',
        'article'
    ];
    const candidates = [];
    const seenNodes = new Set();
    selectors.forEach((selector) => {
        document.querySelectorAll(selector).forEach((node) => {
            registerCandidate(candidates, seenNodes, closestStakeContainer(node) || node);
        });
    });

    candidates.forEach((node) => {
        try {
            const text = normalizeText(node.innerText || node.textContent || '');
            if (!text || text.length < 20 || text.length > 900) return;

            const odds = extractOdds(node);
            if (odds.length < 2) return;

            const teamSelectors = [
                '[class*="team"]',
                '[class*="competitor"]',
                '[class*="participant"]',
                '[class*="player"]',
                '[class*="name"]'
            ];
            const names = [];
            teamSelectors.forEach((selector) => {
                node.querySelectorAll(selector).forEach((child) => {
                    const text = normalizeText(child.getAttribute('title') || child.textContent || '');
                    if (isValidName(text) && !names.includes(text)) names.push(text);
                });
            });

            const extractedNames = extractNames(node.innerText || node.textContent || '', names);
            if (!extractedNames) return;
            const [home, away] = extractedNames;

            if (!isValidName(home) || !isValidName(away) || home === away) return;

            const hrefNode = node.closest('a[href]') || node.querySelector('a[href]');
            const href = hrefNode ? normalizeText(hrefNode.getAttribute('href') || '') : '';
            const eventId = normalizeText(
                node.getAttribute('data-event-id')
                || node.getAttribute('data-id')
                || ((href.match(/addStake\((\d+)/i) || [])[1] || '')
                || node.id
                || ''
            );
            const league = extractLeague(node, node.innerText || node.textContent || '', home, away);
            const sport = extractSport(node, league, text) || document.body.getAttribute('data-sport') || '';
            const key = [eventId, home, away, href].join('|');
            if (seen.has(key)) return;
            seen.add(key);

            results.push({
                eventId,
                home,
                away,
                league,
                sport,
                href,
                odds,
                sourceUrl: window.location.href
            });
        } catch (_) {}
    });

    return results;
})()"#;

fn is_valid_competitor(name: &str) -> bool {
    let normalized = name.trim();
    if normalized.len() < 2 || normalized.len() > 80 {
        return false;
    }
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch.is_whitespace() || matches!(ch, ':' | '.' | '-'))
    {
        return false;
    }

    let lower = normalized.to_lowercase();
    ![
        "live",
        "prematch",
        "match",
        "event",
        "sport",
        "команда",
        "игрок",
        "player",
        "unknown",
        "неизвестно",
    ]
    .iter()
    .any(|blocked| lower == *blocked)
}

#[derive(Debug)]
pub struct MelbetParser {
    client: Arc<Client>,
}

impl MelbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn runtime_probe_plan() -> [&'static HeadlessProbe; 1] {
        [&HEADLESS_PROBES[0]]
    }

    fn synthetic_navigation_failure_snapshot(probe: &HeadlessProbe) -> MelbetBootstrapSnapshot {
        let parsed = Url::parse(probe.url).ok();

        MelbetBootstrapSnapshot {
            final_url: probe.url.to_string(),
            origin: parsed
                .as_ref()
                .map(|url| {
                    let mut origin =
                        format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
                    if let Some(port) = url.port() {
                        origin.push(':');
                        origin.push_str(&port.to_string());
                    }
                    origin
                })
                .unwrap_or_default(),
            path: parsed
                .as_ref()
                .map(|url| url.path().to_string())
                .unwrap_or_default(),
            referrer: String::new(),
            iframe_sources: Vec::new(),
            title: String::new(),
            body_text_sample: String::new(),
            cookie: String::new(),
            local_storage_keys: Vec::new(),
            session_storage_keys: Vec::new(),
            html_class_list: Vec::new(),
            body_class_list: Vec::new(),
            root_node_ids: Vec::new(),
            meta_viewport: String::new(),
            script_sources: Vec::new(),
            user_agent: probe.profile.user_agent.to_string(),
            profile_label: probe.profile.label.to_string(),
            app_marker: probe.profile.app_marker.unwrap_or_default().to_string(),
            max_touch_points: 0,
            inner_width: probe.profile.viewport.0 as u64,
            inner_height: probe.profile.viewport.1 as u64,
            has_service_worker: false,
            resource_timeline: Vec::new(),
            transport_hints: Vec::new(),
            runtime_context: MelbetRuntimeContext::default(),
            readiness: MelbetReadinessDiagnostics::from_value(&serde_json::Value::Null),
            runtime_state: MelbetRuntimeState::default(),
        }
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::DiagnosticOnly,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "melbet_dom_extraction_available".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Rendered DOM extraction remains available for desktop Melbet coverage while groundwork diagnostics evolve.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_resource_timeline_capture_available".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Session bootstrap captures passive Performance API resource timeline samples for route diagnostics.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_transport_hints_passive_only".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Transport hints are inferred from scripts, storage, body text, and resource names without websocket interception or frame decoding.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_mobile_webview_readiness_diagnostics_available".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Bootstrap-only mobile/webview routes now record readiness diagnostics for app-shell visibility, load timing, and feed-like resources.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_transport_mapping_classification_available".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Passive transport hints are normalized into structured families, host/protocol summaries, and readiness output for future transport mapping.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_transport_runtime_guardrail".to_string(),
                    severity: DiagnosticSeverity::Fail,
                    message: "Confirmed blocker is additional_bootstrap_source_required: next practical step is manual bootstrap acquisition of window.$httpApi, partnerId, langId, countryCode, and required Melbet HTTP API methods before any transport/runtime work.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "melbet_transport_groundwork_notes_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: TRANSPORT_HINT_NOTES.join(" "),
                },
            ],
        }
    }

    fn normalize_url(raw: &str, source_url: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return source_url.to_string();
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return trimmed.to_string();
        }
        if trimmed.starts_with('/') {
            return format!("{BASE_URL}{trimmed}");
        }
        format!("{BASE_URL}/{trimmed}")
    }

    fn infer_sport(value: &str, source_url: &str) -> Sport {
        let combined = format!("{} {}", value, source_url).to_lowercase();
        match () {
            _ if combined.contains("football")
                || combined.contains("soccer")
                || combined.contains("футбол") =>
            {
                Sport::Football
            }
            _ if combined.contains("tennis") || combined.contains("теннис") => Sport::Tennis,
            _ if combined.contains("basket") || combined.contains("баскет") => {
                Sport::Basketball
            }
            _ if combined.contains("hockey") || combined.contains("хоккей") => Sport::Hockey,
            _ if combined.contains("volley") || combined.contains("волейбол") => {
                Sport::Volleyball
            }
            _ if combined.contains("table tennis") || combined.contains("настольный") => {
                Sport::TableTennis
            }
            _ if combined.contains("esport") || combined.contains("кибер") => Sport::Esports,
            _ if combined.contains("handball") || combined.contains("гандбол") => {
                Sport::Handball
            }
            _ if combined.contains("futsal") || combined.contains("мини-футбол") => {
                Sport::Futsal
            }
            _ => Sport::Other,
        }
    }

    fn looks_like_block_page(snapshot: &MelbetBootstrapSnapshot) -> bool {
        let haystack = format!(
            "{} {} {} {} {} {}",
            snapshot.final_url,
            snapshot.path,
            snapshot.title,
            snapshot.body_text_sample,
            snapshot.user_agent,
            snapshot.meta_viewport,
        )
        .to_lowercase();
        [
            "captcha",
            "cloudflare",
            "access denied",
            "forbidden",
            "verify you are human",
            "blocked",
            "robot",
            "ddos",
            "security check",
            "доступ ограничен",
            "доступ запрещен",
            "проверка безопасности",
            "подтвердите, что вы не робот",
            "browser check",
            "cf-challenge",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
    }

    fn route_matches_probe(probe: &HeadlessProbe, snapshot: &MelbetBootstrapSnapshot) -> bool {
        let final_url = snapshot.final_url.to_lowercase();
        let path = snapshot.path.to_lowercase();
        let route_token = if probe.is_live { "live" } else { "line" };
        let surface_token = match probe.surface {
            MelbetSurface::Desktop => "melbet.ru",
            MelbetSurface::Mobile => "/m/",
            MelbetSurface::WebView => "m.melbet.com",
        };
        let iframe_match = snapshot.iframe_sources.iter().any(|source| {
            let source = source.to_lowercase();
            probe.surface == MelbetSurface::Desktop
                && source.contains("sport.melbet.ru")
                && source.contains("sportsbook/home")
        });

        ((final_url.contains(route_token) || path.contains(route_token))
            && (final_url.contains(surface_token) || path.contains(surface_token)))
            || (probe.surface == MelbetSurface::Desktop
                && (final_url.contains("/ru/sport") || path == "/ru/sport" || path == "/ru/sport/")
                && iframe_match)
    }

    fn has_bootstrap_markers(probe: &HeadlessProbe, snapshot: &MelbetBootstrapSnapshot) -> bool {
        let lower_text = format!("{} {}", snapshot.title, snapshot.body_text_sample).to_lowercase();
        let lower_classes = snapshot
            .html_class_list
            .iter()
            .chain(snapshot.body_class_list.iter())
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let lower_keys = snapshot
            .local_storage_keys
            .iter()
            .chain(snapshot.session_storage_keys.iter())
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let lower_scripts = snapshot
            .script_sources
            .iter()
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let app_marker_match = snapshot.app_marker == probe.profile.app_marker.unwrap_or_default();

        !snapshot.cookie.trim().is_empty()
            || snapshot.storage_key_count() > 0
            || snapshot.root_node_count() > 0
            || snapshot.iframe_source_count() > 0
            || snapshot.script_count() > 2
            || snapshot.has_service_worker
            || snapshot.transport_hint_count() > 0
            || snapshot.resource_timeline_count() > 0
            || snapshot.readiness.has_visible_app_shell
            || snapshot.readiness.fetch_like_count > 0
            || snapshot.runtime_state.has_bootstrap_markers()
            || snapshot.max_touch_points >= 1
            || snapshot.inner_width >= 320
            || snapshot.inner_height >= 480
            || !snapshot.meta_viewport.trim().is_empty()
            || !snapshot.profile_label.trim().is_empty()
            || app_marker_match
            || lower_text.contains("melbet")
            || lower_text.contains("live")
            || lower_text.contains("prematch")
            || lower_classes
                .iter()
                .any(|item| item.contains("app") || item.contains("root"))
            || lower_keys.iter().any(|item| {
                item.contains("app")
                    || item.contains("session")
                    || item.contains("route")
                    || item.contains("token")
                    || item.contains("device")
            })
            || lower_scripts.iter().any(|item| {
                item.contains("app")
                    || item.contains("bundle")
                    || item.contains("chunk")
                    || item.contains("integrationloader")
            })
            || snapshot.iframe_sources.iter().any(|item| {
                let item = item.to_lowercase();
                item.contains("sport.melbet.ru") || item.contains("sportsbook/home")
            })
    }

    fn has_useful_desktop_bootstrap(snapshot: &MelbetBootstrapSnapshot) -> bool {
        let runtime = &snapshot.runtime_state;
        let readiness = &snapshot.readiness;
        let title = format!("{} {}", snapshot.title, runtime.title).to_lowercase();
        let body =
            format!("{} {}", snapshot.body_text_sample, runtime.body_text_sample).to_lowercase();

        (matches!(runtime.ready_state.as_str(), "interactive" | "complete")
            || matches!(readiness.ready_state.as_str(), "interactive" | "complete"))
            && (runtime.router_shell_count > 0
                || runtime.route_link_count > 0
                || runtime.link_count > 0
                || runtime.interactive_node_count > 0
                || runtime.body_child_count > 0
                || runtime.body_text_length > 0
                || readiness.body_child_count > 0
                || readiness.body_text_length > 0
                || readiness.has_visible_app_shell
                || readiness.fetch_like_count > 0)
            && (title.contains("melbet")
                || title.contains("live")
                || body.contains("melbet")
                || body.contains("sport")
                || body.contains("live")
                || runtime.has_sportsbook_shell_markers())
    }

    fn select_embedded_route(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
    ) -> Option<String> {
        if probe.surface != MelbetSurface::Desktop {
            return None;
        }

        snapshot
            .iframe_sources
            .iter()
            .find(|source| {
                let lower = source.to_lowercase();
                lower.contains("sport.melbet.ru") && lower.contains("sportsbook/home")
            })
            .cloned()
    }

    fn sportsbook_initial_route_url(path: &str) -> String {
        let mut url = Url::parse(&format!("{}partner/SportsBook/Home", SPORTSBOOK_BASE_URL))
            .expect("valid Melbet sportsbook base url");
        let initial_route = serde_json::json!({ "path": path }).to_string();
        url.query_pairs_mut()
            .append_pair("initialRoute", &initial_route);
        url.into()
    }

    fn default_sportsbook_route_candidates(probe: &HeadlessProbe) -> Vec<String> {
        let route_path = if probe.is_live { "/live" } else { "/line" };
        vec![
            Self::sportsbook_initial_route_url(route_path),
            SPORTSBOOK_HOME_URL.to_string(),
        ]
    }

    fn has_empty_runtime_bootstrap(snapshot: &MelbetBootstrapSnapshot) -> bool {
        snapshot.final_url.trim().is_empty()
            && snapshot.path.trim().is_empty()
            && snapshot.title.trim().is_empty()
            && snapshot.body_text_sample.trim().is_empty()
            && snapshot.iframe_sources.is_empty()
            && snapshot.script_sources.is_empty()
            && snapshot.resource_timeline.is_empty()
            && snapshot.cookie.trim().is_empty()
            && snapshot.storage_key_count() == 0
            && snapshot.root_node_count() == 0
            && snapshot.transport_hint_count() == 0
            && !snapshot.readiness.has_visible_app_shell
            && snapshot.readiness.body_child_count == 0
            && snapshot.readiness.body_text_length == 0
            && snapshot.runtime_state.body_child_count == 0
            && snapshot.runtime_state.body_text_length == 0
            && snapshot.runtime_state.interactive_node_count == 0
            && !snapshot.runtime_state.has_bootstrap_markers()
            && !snapshot.runtime_context.has_bootstrap_source_markers()
    }

    fn push_unique_route_candidate(route_candidates: &mut Vec<String>, route: impl Into<String>) {
        let route = route.into();
        if route.trim().is_empty() || route_candidates.iter().any(|item| item == &route) {
            return;
        }
        route_candidates.push(route);
    }

    fn as_sportsbook_route_candidate(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("javascript:") {
            return None;
        }

        let url = Url::parse(trimmed).ok()?;
        let host = url.host_str()?.to_lowercase();
        let path = url.path().to_lowercase();
        if host != "sport.melbet.ru" {
            return None;
        }
        if path == "/"
            || path.contains("/partner/sportsbook/home")
            || path.contains("/sportsbook/home")
        {
            return Some(url.into());
        }

        None
    }

    fn sportsbook_route_candidates(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
    ) -> Vec<String> {
        if probe.surface != MelbetSurface::Desktop {
            return Vec::new();
        }

        let mut route_candidates = Vec::new();
        if let Some(route) = Self::select_embedded_route(probe, snapshot) {
            Self::push_unique_route_candidate(&mut route_candidates, route);
        }

        for route in snapshot.iframe_sources.iter().chain([
            &snapshot.final_url,
            &snapshot.referrer,
            &snapshot.runtime_state.href,
        ]) {
            if let Some(candidate) = Self::as_sportsbook_route_candidate(route) {
                Self::push_unique_route_candidate(&mut route_candidates, candidate);
            }
        }

        for route in snapshot
            .resource_timeline
            .iter()
            .map(|entry| entry.name.as_str())
            .chain(snapshot.script_sources.iter().map(String::as_str))
        {
            if let Some(candidate) = Self::as_sportsbook_route_candidate(route) {
                Self::push_unique_route_candidate(&mut route_candidates, candidate);
            }
        }

        let should_add_defaults = probe.surface == MelbetSurface::Desktop
            || Self::has_sportsbook_shell_markers(snapshot)
            || (Self::route_matches_probe(probe, snapshot)
                && Self::has_bootstrap_markers(probe, snapshot))
            || snapshot.runtime_context.has_bootstrap_source_markers()
            || snapshot.readiness.has_visible_app_shell
            || snapshot.readiness.fetch_like_count > 0
            || snapshot.runtime_state.has_sportsbook_shell_markers()
            || snapshot.resource_timeline.iter().any(|entry| {
                let lower = entry.name.to_lowercase();
                lower.contains("sport.melbet.ru")
                    || lower.contains("/partner/sportsbook/home")
                    || lower.contains("/sportsbook/home")
            });
        if should_add_defaults {
            for route in Self::default_sportsbook_route_candidates(probe) {
                Self::push_unique_route_candidate(&mut route_candidates, route);
            }
        }

        route_candidates
    }

    fn has_sportsbook_shell_markers(snapshot: &MelbetBootstrapSnapshot) -> bool {
        let final_url = snapshot.final_url.to_lowercase();
        let path = snapshot.path.to_lowercase();
        let title = snapshot.title.to_lowercase();
        let body = snapshot.body_text_sample.to_lowercase();
        let scripts = snapshot
            .script_sources
            .iter()
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();
        let runtime_title = snapshot.runtime_state.title.to_lowercase();
        let runtime_body = snapshot.runtime_state.body_text_sample.to_lowercase();
        let bootstrap_markers = snapshot
            .runtime_context
            .bootstrap_markers
            .iter()
            .map(|item| item.to_lowercase())
            .collect::<Vec<_>>();

        final_url.contains("/ru/sport")
            || path == "/ru/sport"
            || path == "/ru/sport/"
            || snapshot.runtime_state.has_sportsbook_shell_markers()
            || title.contains("melbet.ru")
            || runtime_title.contains("melbet")
            || body.contains("sport")
            || body.contains("melbet")
            || runtime_body.contains("sport")
            || runtime_body.contains("melbet")
            || scripts.iter().any(|script| {
                script.contains("main.js")
                    || script.contains("bundle.js")
                    || script.contains("bootstrapper")
                    || script.contains("sport")
            })
            || bootstrap_markers.iter().any(|marker| {
                marker.contains("route_family:") && marker.contains("sportsbook")
                    || marker.contains("sportsbook_route")
                    || marker.contains("inline:initialroute")
                    || marker.contains("shell:ww-")
                    || marker.contains("shell:#root")
                    || marker.contains("shell:#app")
                    || marker.contains("shell:route_links:")
            })
    }

    fn recovered_bootstrap_snapshot_from_runtime_state(
        runtime_state: &MelbetRuntimeState,
    ) -> Option<MelbetBootstrapSnapshot> {
        if runtime_state.href.trim().is_empty()
            && runtime_state.pathname.trim().is_empty()
            && runtime_state.title.trim().is_empty()
            && runtime_state.body_text_sample.trim().is_empty()
            && runtime_state.ready_state.trim().is_empty()
            && runtime_state.body_child_count == 0
            && runtime_state.body_text_length == 0
            && runtime_state.interactive_node_count == 0
            && !runtime_state.has_bootstrap_markers()
        {
            return None;
        }

        let parsed = Url::parse(&runtime_state.href).ok();
        let mut bootstrap_markers = runtime_state.bootstrap_markers.clone();
        if !runtime_state.href.trim().is_empty() {
            bootstrap_markers.push(format!("route:runtime:{}", runtime_state.href));
        }
        if !runtime_state.pathname.trim().is_empty() {
            bootstrap_markers.push(format!("route:path:{}", runtime_state.pathname));
        }
        if runtime_state.pathname.contains("/live") || runtime_state.href.contains("/live") {
            bootstrap_markers.push("route_family:runtime:live".to_string());
        }
        if runtime_state.pathname.contains("/line") || runtime_state.href.contains("/line") {
            bootstrap_markers.push("route_family:runtime:line".to_string());
        }
        if runtime_state.has_sportsbook_shell_markers() {
            bootstrap_markers.push("route_family:runtime:sportsbook".to_string());
            bootstrap_markers.push("shell:runtime_state".to_string());
        }

        Some(MelbetBootstrapSnapshot {
            final_url: runtime_state.href.clone(),
            origin: parsed
                .as_ref()
                .map(|url| {
                    let mut origin =
                        format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
                    if let Some(port) = url.port() {
                        origin.push(':');
                        origin.push_str(&port.to_string());
                    }
                    origin
                })
                .unwrap_or_default(),
            path: if runtime_state.pathname.trim().is_empty() {
                parsed
                    .as_ref()
                    .map(|url| url.path().to_string())
                    .unwrap_or_default()
            } else {
                runtime_state.pathname.clone()
            },
            referrer: String::new(),
            iframe_sources: Vec::new(),
            title: runtime_state.title.clone(),
            body_text_sample: runtime_state.body_text_sample.clone(),
            cookie: String::new(),
            local_storage_keys: Vec::new(),
            session_storage_keys: Vec::new(),
            html_class_list: Vec::new(),
            body_class_list: Vec::new(),
            root_node_ids: Vec::new(),
            meta_viewport: String::new(),
            script_sources: Vec::new(),
            user_agent: String::new(),
            profile_label: String::new(),
            app_marker: String::new(),
            max_touch_points: 0,
            inner_width: 0,
            inner_height: 0,
            has_service_worker: false,
            resource_timeline: Vec::new(),
            transport_hints: Vec::new(),
            runtime_context: MelbetRuntimeContext {
                bootstrap_markers,
                ..MelbetRuntimeContext::default()
            },
            readiness: MelbetReadinessDiagnostics {
                ready_state: runtime_state.ready_state.clone(),
                body_text_length: runtime_state.body_text_length,
                body_child_count: runtime_state.body_child_count,
                dom_content_loaded_ms: runtime_state.dom_content_loaded_ms,
                load_event_ms: runtime_state.load_ms,
                has_visible_app_shell: runtime_state.has_bootstrap_markers()
                    || runtime_state.interactive_node_count > 0,
                ..MelbetReadinessDiagnostics::from_value(&serde_json::Value::Null)
            },
            runtime_state: runtime_state.clone(),
        })
    }

    fn select_sportsbook_route(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
    ) -> Option<String> {
        Self::sportsbook_route_candidates(probe, snapshot)
            .into_iter()
            .next()
    }

    fn has_explicit_sportsbook_route_candidate(snapshot: &MelbetBootstrapSnapshot) -> bool {
        snapshot
            .iframe_sources
            .iter()
            .chain([
                &snapshot.final_url,
                &snapshot.referrer,
                &snapshot.runtime_state.href,
            ])
            .any(|route| Self::as_sportsbook_route_candidate(route).is_some())
            || snapshot
                .resource_timeline
                .iter()
                .map(|entry| entry.name.as_str())
                .chain(snapshot.script_sources.iter().map(String::as_str))
                .any(|route| Self::as_sportsbook_route_candidate(route).is_some())
    }

    fn extract_sport_api_context(tab: &headless_chrome::Tab) -> Option<MelbetSportApiContext> {
        let value = HeadlessChromeHelper::evaluate_async_json_with_retry(
            tab,
            r#"
                return {
                    partnerId: Number(window.$P?.Id || window.$globalSettings?.partner?.Id || 0),
                    langId: Number(window.$globalSettings?.language?.Id || 1),
                    countryCode: String(window.$globalSettings?.user?.CountryCode || 'RU'),
                    hasHttpApi: Boolean(window.$httpApi)
                };
            "#,
            HEADLESS_ASYNC_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )?;
        if !value
            .get("hasHttpApi")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return None;
        }

        Some(MelbetSportApiContext {
            partner_id: value
                .get("partnerId")
                .and_then(|value| value.as_u64())
                .filter(|value| *value > 0)?,
            lang_id: value
                .get("langId")
                .and_then(|value| value.as_u64())
                .filter(|value| *value > 0)
                .unwrap_or(1),
            country_code: value
                .get("countryCode")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("RU")
                .to_string(),
        })
    }

    fn extract_sport_seeds(
        tab: &headless_chrome::Tab,
        context: &MelbetSportApiContext,
        is_live: bool,
    ) -> Vec<MelbetSportSeed> {
        let request_expr = if is_live {
            Self::wrap_http_api_call_with_timeout(
                &format!(
                    "window.$httpApi.getTopLiveSports({sport_limit}, {country_code:?}, {lang_id}, {partner_id})",
                    sport_limit = SPORTSBOOK_SPORT_LIMIT,
                    country_code = context.country_code,
                    lang_id = context.lang_id,
                    partner_id = context.partner_id,
                ),
                "getTopLiveSports",
            )
        } else {
            Self::wrap_http_api_call_with_timeout(
                &format!(
                    "window.$httpApi.getPrematchSports(now.toISOString(), end.toISOString(), 1, false, null, {lang_id}, {partner_id}, {country_code:?})",
                    country_code = context.country_code,
                    lang_id = context.lang_id,
                    partner_id = context.partner_id,
                ),
                "getPrematchSports",
            )
        };
        let js = if is_live {
            format!(
                r#"
                    const items = ({request_expr}) || [];
                    return items
                        .filter((item) => Number(item?.EC || 0) > 0)
                        .slice(0, {sport_limit})
                        .map((item) => ({{
                            sportId: Number(item.Id || 0),
                            eventCount: Number(item.EC || 0)
                        }}));
                "#,
                sport_limit = SPORTSBOOK_SPORT_LIMIT,
                request_expr = request_expr,
            )
        } else {
            format!(
                r#"
                    const now = new Date();
                    const end = new Date(now.getTime() + 48 * 60 * 60 * 1000);
                    const items = ({request_expr}) || [];
                    return items
                        .filter((item) => Number(item?.EC || 0) > 0)
                        .slice(0, {sport_limit})
                        .map((item) => ({{
                            sportId: Number(item.Id || 0),
                            eventCount: Number(item.EC || 0)
                        }}));
                "#,
                sport_limit = SPORTSBOOK_SPORT_LIMIT,
                request_expr = request_expr,
            )
        };

        HeadlessChromeHelper::evaluate_async_json_with_retry(
            tab,
            &js,
            HEADLESS_ASYNC_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| {
            Some(MelbetSportSeed {
                sport_id: value.get("sportId")?.as_u64()?,
                event_count: value
                    .get("eventCount")
                    .and_then(|value| value.as_u64())
                    .unwrap_or_default() as usize,
            })
        })
        .filter(|seed| seed.sport_id > 0 && seed.event_count > 0)
        .collect()
    }

    fn extract_sport_api_items_for_sport(
        tab: &headless_chrome::Tab,
        context: &MelbetSportApiContext,
        is_live: bool,
        sport_id: u64,
    ) -> Vec<serde_json::Value> {
        let request_expr = if is_live {
            Self::wrap_http_api_call_with_timeout(
                &format!(
                    "getter({sport_id}, [1, 2, 3], {event_limit}, {country_code:?}, {lang_id}, {partner_id})",
                    sport_id = sport_id,
                    event_limit = SPORTSBOOK_EVENT_LIMIT,
                    country_code = context.country_code,
                    lang_id = context.lang_id,
                    partner_id = context.partner_id,
                ),
                "getTopLiveEvents",
            )
        } else {
            Self::wrap_http_api_call_with_timeout(
                &format!(
                    "getter({sport_id}, [1, 2, 3], {event_limit}, {country_code:?}, {lang_id}, {partner_id})",
                    sport_id = sport_id,
                    event_limit = SPORTSBOOK_EVENT_LIMIT,
                    country_code = context.country_code,
                    lang_id = context.lang_id,
                    partner_id = context.partner_id,
                ),
                "getTopEvents",
            )
        };
        let js = format!(
            r#"
                const normalizeOdds = (event) => {{
                    const stakeTypes = Array.isArray(event?.StakeType) ? event.StakeType : [];
                    const preferredMarket = stakeTypes.find((item) => Number(item?.Id || 0) === 1 && Array.isArray(item?.Stakes) && item.Stakes.length >= 2)
                        || stakeTypes.find((item) => Array.isArray(item?.Stakes) && item.Stakes.length >= 2)
                        || null;
                    if (!preferredMarket) return [];

                    const bySelection = new Map();
                    for (const stake of preferredMarket.Stakes || []) {{
                        if (!stake || typeof stake.F !== 'number') continue;
                        if (!bySelection.has(Number(stake.SC || 0))) {{
                            bySelection.set(Number(stake.SC || 0), Number(stake.F));
                        }}
                    }}

                    if (bySelection.has(1) && bySelection.has(2) && bySelection.has(3)) {{
                        return [bySelection.get(1), bySelection.get(2), bySelection.get(3)];
                    }}
                    if (bySelection.has(1) && bySelection.has(3)) {{
                        return [bySelection.get(1), bySelection.get(3)];
                    }}

                    return Array.from(bySelection.values()).slice(0, 3);
                }};
                const sourceUrl = window.location.href;
                const getter = {getter};
                const events = ({request_expr}) || [];
                return events
                    .map((event) => ({{
                        eventId: String(event?.Id || ''),
                        home: String(event?.HT || '').trim(),
                        away: String(event?.AT || '').trim(),
                        league: String(event?.ECN || event?.CN || '').trim(),
                        sport: String(event?.ESN || event?.SN || '').trim(),
                        href: '',
                        sourceUrl,
                        odds: normalizeOdds(event)
                    }}))
                    .filter((event) => event.home && event.away && event.odds.length >= 2);
            "#,
            getter = if is_live {
                "window.$httpApi.getTopLiveEvents"
            } else {
                "window.$httpApi.getTopEvents"
            },
            request_expr = request_expr,
        );

        HeadlessChromeHelper::evaluate_async_json_with_retry(
            tab,
            &js,
            HEADLESS_ASYNC_EVAL_ATTEMPTS,
            HEADLESS_RETRY_DELAY_MS,
        )
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
    }

    fn extract_sportsbook_http_api_payload(
        helper: &HeadlessChromeHelper,
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        deadline: Instant,
    ) -> MelbetSportsbookHttpApiAttempt {
        let route_candidates = Self::sportsbook_route_candidates(probe, snapshot);
        if route_candidates.is_empty() {
            return MelbetSportsbookHttpApiAttempt {
                blocker: "no_sportsbook_route".to_string(),
                ..MelbetSportsbookHttpApiAttempt::default()
            };
        }
        let referer = if snapshot.final_url.is_empty() {
            probe.url
        } else {
            snapshot.final_url.as_str()
        };
        let mut navigation_failure = None;

        if let Some(attempt) =
            Self::runtime_blocker_cutoff_attempt(probe, snapshot, route_candidates.first().cloned())
        {
            return attempt;
        }

        for direct_route in route_candidates {
            let tab = match helper.navigate_with_profile_and_referer_with_timeout_and_deadline(
                &direct_route,
                HEADLESS_WAIT_MS,
                probe.profile,
                Some(referer),
                SPORTSBOOK_NAVIGATION_TIMEOUT_MS,
                deadline,
            ) {
                Ok(tab) => tab,
                Err(_) => {
                    navigation_failure = Some(Self::sportsbook_navigation_failure_fallback(
                        probe,
                        snapshot,
                        Some(direct_route),
                    ));
                    continue;
                }
            };

            let mut bootstrap = Self::extract_bootstrap_snapshot(&tab);
            if bootstrap.final_url.is_empty() {
                bootstrap.final_url = direct_route.clone();
            }
            let mut dom_payload = Self::extract_headless_payload(&tab);
            if dom_payload.is_empty() {
                for _ in 0..HEADLESS_SCROLL_ROUNDS {
                    let _ = HeadlessChromeHelper::scroll_page(&tab);
                    let next_payload = Self::extract_headless_payload(&tab);
                    if next_payload.len() > dom_payload.len() {
                        dom_payload = next_payload;
                    }
                }
            }
            if !dom_payload.is_empty() {
                return MelbetSportsbookHttpApiAttempt {
                    bootstrap: Some(bootstrap),
                    route: direct_route,
                    seed_count: 0,
                    blocker: "dom_payload_ready_on_sportsbook_route".to_string(),
                    payload: dom_payload,
                };
            }

            let runtime_context_gap = bootstrap
                .runtime_context
                .missing_http_api_requirements(probe.is_live);
            if !runtime_context_gap.is_empty() {
                return MelbetSportsbookHttpApiAttempt {
                    bootstrap: Some(bootstrap.clone()),
                    route: direct_route,
                    blocker: bootstrap.runtime_context.http_api_blocker(probe.is_live),
                    ..MelbetSportsbookHttpApiAttempt::default()
                };
            }
            let context = match Self::extract_sport_api_context(&tab) {
                Some(context) => context,
                None => {
                    return MelbetSportsbookHttpApiAttempt {
                        bootstrap: Some(bootstrap.clone()),
                        route: direct_route,
                        blocker: bootstrap.runtime_context.http_api_blocker(probe.is_live),
                        ..MelbetSportsbookHttpApiAttempt::default()
                    };
                }
            };
            let seeds = Self::extract_sport_seeds(&tab, &context, probe.is_live);
            if seeds.is_empty() {
                return MelbetSportsbookHttpApiAttempt {
                    bootstrap: Some(bootstrap),
                    route: direct_route,
                    blocker: "no_sport_seeds".to_string(),
                    ..MelbetSportsbookHttpApiAttempt::default()
                };
            }

            let mut payload = Vec::new();
            let mut seen_event_ids = HashSet::new();
            let seed_count = seeds.len();
            for seed in seeds {
                for item in Self::extract_sport_api_items_for_sport(
                    &tab,
                    &context,
                    probe.is_live,
                    seed.sport_id,
                ) {
                    let event_id = item
                        .get("eventId")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !event_id.is_empty() && seen_event_ids.insert(event_id) {
                        payload.push(item);
                    }
                }
            }

            return MelbetSportsbookHttpApiAttempt {
                bootstrap: Some(bootstrap),
                route: direct_route,
                seed_count,
                blocker: if payload.is_empty() {
                    "no_http_api_event_payload".to_string()
                } else {
                    "payload_ready".to_string()
                },
                payload,
            };
        }

        navigation_failure
            .unwrap_or_else(|| Self::sportsbook_navigation_failure_fallback(probe, snapshot, None))
    }

    fn runtime_blocker_cutoff_attempt(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        route: Option<String>,
    ) -> Option<MelbetSportsbookHttpApiAttempt> {
        if Self::should_short_circuit_sportsbook_http_api(snapshot, probe.is_live) {
            Some(Self::sportsbook_navigation_failure_fallback(
                probe, snapshot, route,
            ))
        } else {
            None
        }
    }

    fn should_short_circuit_sportsbook_http_api(
        snapshot: &MelbetBootstrapSnapshot,
        is_live: bool,
    ) -> bool {
        snapshot.looks_like_blocked_runtime()
            || snapshot
                .runtime_context
                .http_api_blocker(is_live)
                .contains("additional_bootstrap_source_required")
    }

    fn sportsbook_navigation_failure_fallback(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        route: Option<String>,
    ) -> MelbetSportsbookHttpApiAttempt {
        let route = route
            .unwrap_or_else(|| Self::select_sportsbook_route(probe, snapshot).unwrap_or_default());
        if snapshot.looks_like_blocked_runtime() {
            return MelbetSportsbookHttpApiAttempt {
                bootstrap: Some(snapshot.clone()),
                route,
                blocker: snapshot.runtime_state.blocker_code(),
                ..MelbetSportsbookHttpApiAttempt::default()
            };
        }
        let has_bootstrapped_shell = Self::has_bootstrap_markers(probe, snapshot)
            && (Self::has_sportsbook_shell_markers(snapshot)
                || Self::has_explicit_sportsbook_route_candidate(snapshot)
                || (probe.surface == MelbetSurface::Desktop
                    && Self::has_useful_desktop_bootstrap(snapshot)));

        if has_bootstrapped_shell {
            return MelbetSportsbookHttpApiAttempt {
                bootstrap: Some(snapshot.clone()),
                route,
                blocker: snapshot.runtime_context.http_api_blocker(probe.is_live),
                ..MelbetSportsbookHttpApiAttempt::default()
            };
        }

        MelbetSportsbookHttpApiAttempt {
            route,
            blocker: "sportsbook_navigation_failed".to_string(),
            ..MelbetSportsbookHttpApiAttempt::default()
        }
    }

    fn summarize_transport_hints(snapshot: &MelbetBootstrapSnapshot) -> String {
        let normalized = snapshot.normalized_transport_hints();
        if normalized.is_empty() {
            return "none".to_string();
        }

        normalized
            .iter()
            .take(5)
            .map(MelbetNormalizedTransportHint::as_summary)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn wrap_http_api_call_with_timeout(call_expr: &str, label: &str) -> String {
        format!(
            "await Promise.race([Promise.resolve({call_expr}), new Promise((_, reject) => setTimeout(() => reject(new Error({label:?} + ' timed out after ' + {timeout_ms} + 'ms')), {timeout_ms}))])",
            call_expr = call_expr,
            label = label,
            timeout_ms = SPORTSBOOK_HTTP_API_TIMEOUT_MS,
        )
    }

    fn summarize_resource_timeline(snapshot: &MelbetBootstrapSnapshot) -> String {
        if snapshot.resource_timeline.is_empty() {
            return "none".to_string();
        }

        snapshot
            .resource_timeline
            .iter()
            .take(TRANSPORT_HINT_TIMELINE_LIMIT)
            .map(MelbetResourceTimelineEntry::as_summary)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn summarize_runtime_context(snapshot: &MelbetBootstrapSnapshot) -> String {
        snapshot.runtime_context.as_summary()
    }

    fn summarize_runtime_state(snapshot: &MelbetBootstrapSnapshot) -> String {
        snapshot.runtime_state.as_summary()
    }

    fn required_runtime_fields(is_live: bool) -> Vec<String> {
        let mut fields = vec![
            "window.$httpApi".to_string(),
            "partnerId".to_string(),
            "langId".to_string(),
            "countryCode".to_string(),
        ];
        fields.extend(
            if is_live {
                ["getTopLiveSports", "getTopLiveEvents"]
            } else {
                ["getPrematchSports", "getTopEvents"]
            }
            .into_iter()
            .map(str::to_string),
        );
        fields
    }

    fn build_bootstrap_acquisition_plan(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        blocker: &str,
    ) -> MelbetBootstrapAcquisitionPlan {
        let confirmed_blocker = if blocker.contains("additional_bootstrap_source_required") {
            "additional_bootstrap_source_required"
        } else {
            "not_confirmed"
        };
        let next_step = if confirmed_blocker == "additional_bootstrap_source_required" {
            "manual_bootstrap_acquisition"
        } else {
            "no_manual_bootstrap_acquisition_required"
        };

        let mut route_candidates = Self::sportsbook_route_candidates(probe, snapshot);
        if !snapshot.final_url.trim().is_empty()
            && !route_candidates
                .iter()
                .any(|item| item == &snapshot.final_url)
        {
            route_candidates.push(snapshot.final_url.clone());
        }
        if route_candidates.is_empty() {
            route_candidates.push(probe.url.to_string());
        }

        MelbetBootstrapAcquisitionPlan {
            blocker: blocker.to_string(),
            confirmed_blocker: confirmed_blocker.to_string(),
            next_step: next_step.to_string(),
            primary_target: route_candidates.first().cloned().unwrap_or_default(),
            referer: if snapshot.referrer.trim().is_empty() {
                snapshot.final_url.clone()
            } else {
                snapshot.referrer.clone()
            },
            route_candidates,
            required_runtime_fields: Self::required_runtime_fields(probe.is_live),
            bootstrap_markers: snapshot.runtime_context.bootstrap_markers.clone(),
        }
    }

    fn classify_route_status(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        payload_len: usize,
    ) -> MelbetRouteStatus {
        if Self::looks_like_block_page(snapshot) {
            return MelbetRouteStatus::Blocked;
        }
        if payload_len > 0 {
            return MelbetRouteStatus::Ready;
        }

        if Self::has_bootstrap_markers(probe, snapshot)
            && ((probe.surface != MelbetSurface::Desktop
                && Self::route_matches_probe(probe, snapshot))
                || (probe.surface == MelbetSurface::Desktop
                    && (Self::has_sportsbook_shell_markers(snapshot)
                        || Self::has_explicit_sportsbook_route_candidate(snapshot)
                        || Self::has_useful_desktop_bootstrap(snapshot))))
        {
            return MelbetRouteStatus::BootstrapOnly;
        }

        MelbetRouteStatus::Blocked
    }

    fn normalize_transport_hint(hint: &MelbetTransportHint) -> MelbetNormalizedTransportHint {
        let mut normalized_value = hint.value.trim().to_lowercase();
        let mut host = String::new();
        let mut path = String::new();
        let mut protocol = String::new();

        if let Ok(url) = Url::parse(&hint.value) {
            normalized_value = url.as_str().trim_end_matches('/').to_lowercase();
            host = url.host_str().unwrap_or_default().to_string();
            path = url.path().to_string();
            protocol = url.scheme().to_string();
        } else if hint.value.contains('=') {
            normalized_value = hint
                .value
                .split_once('=')
                .map(|(key, _)| key.trim().to_lowercase())
                .unwrap_or(normalized_value);
            path = normalized_value.clone();
            protocol = "storage".to_string();
        } else if normalized_value.starts_with('/') {
            path = normalized_value.clone();
            protocol = "path".to_string();
        } else if !normalized_value.is_empty() {
            path = normalized_value.clone();
            protocol = hint.source.to_lowercase();
        }

        let family = match hint.kind.as_str() {
            "websocket_endpoint" | "websocket_candidate" => "websocket_like",
            "script_transport_marker" | "body_transport_marker" | "storage_transport_marker" => {
                "transport_marker"
            }
            "data_endpoint" | "script_feed_marker" => "feed_endpoint",
            "http2_transport" => "http_transport",
            _ => "other",
        }
        .to_string();

        let confidence = match hint.kind.as_str() {
            "websocket_endpoint" | "data_endpoint" => "high",
            "websocket_candidate" | "storage_transport_marker" | "script_transport_marker" => {
                "medium"
            }
            _ => "low",
        }
        .to_string();

        MelbetNormalizedTransportHint {
            kind: hint.kind.clone(),
            family,
            source: hint.source.clone(),
            normalized_value,
            host,
            path,
            protocol,
            confidence,
        }
    }

    fn summarize_transport_mapping(
        snapshot: &MelbetBootstrapSnapshot,
    ) -> MelbetTransportMappingSummary {
        let normalized = snapshot.normalized_transport_hints();
        let mut families = normalized
            .iter()
            .map(|hint| hint.family.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        families.sort();
        let mut hosts = normalized
            .iter()
            .filter(|hint| !hint.host.is_empty())
            .map(|hint| hint.host.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        hosts.sort();
        let mut protocols = normalized
            .iter()
            .filter(|hint| !hint.protocol.is_empty())
            .map(|hint| hint.protocol.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        protocols.sort();

        MelbetTransportMappingSummary {
            families,
            hosts,
            protocols,
            high_confidence_count: normalized
                .iter()
                .filter(|hint| hint.confidence == "high")
                .count(),
            websocket_like_count: normalized
                .iter()
                .filter(|hint| hint.family == "websocket_like")
                .count(),
            feed_like_count: normalized
                .iter()
                .filter(|hint| hint.family == "feed_endpoint")
                .count(),
        }
    }

    fn build_readiness_output(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        status: &MelbetRouteStatus,
        payload_len: usize,
        blocker: &str,
    ) -> MelbetReadinessOutput {
        let bootstrap_score = usize::from(!snapshot.cookie.trim().is_empty())
            + usize::from(snapshot.storage_key_count() > 0)
            + usize::from(snapshot.root_node_count() > 0)
            + usize::from(snapshot.script_count() > 2)
            + usize::from(snapshot.has_service_worker)
            + usize::from(snapshot.transport_hint_count() > 0)
            + usize::from(snapshot.resource_timeline_count() > 0)
            + usize::from(snapshot.readiness.has_visible_app_shell)
            + usize::from(snapshot.readiness.fetch_like_count > 0)
            + usize::from(snapshot.readiness.ready_state == "complete");
        let transport_mapping = Self::summarize_transport_mapping(snapshot);
        let route_status = status.as_str().to_string();
        let bootstrap_plan = Self::build_bootstrap_acquisition_plan(probe, snapshot, blocker);
        let (state, reason) = match status {
            MelbetRouteStatus::Ready => (
                "dom_payload_ready",
                if payload_len > 0 {
                    "rendered_dom_payload_detected"
                } else {
                    "route_ready_without_payload"
                },
            ),
            MelbetRouteStatus::BootstrapOnly => (
                if probe.surface == MelbetSurface::WebView {
                    "shell_bootstrapped"
                } else {
                    "bootstrap_only"
                },
                if transport_mapping.feed_like_count > 0 {
                    "bootstrap_signals_with_feed_hints"
                } else {
                    "bootstrap_signals_without_dom_payload"
                },
            ),
            MelbetRouteStatus::Blocked => (
                "blocked_or_unconfirmed",
                if Self::looks_like_block_page(snapshot) {
                    "block_page_markers_detected"
                } else {
                    "insufficient_bootstrap_signals"
                },
            ),
        };

        MelbetReadinessOutput {
            state: state.to_string(),
            reason: reason.to_string(),
            route_status,
            bootstrap_score,
            blocker: blocker.to_string(),
            confirmed_blocker: bootstrap_plan.confirmed_blocker,
            next_step: bootstrap_plan.next_step,
            transport_mapping,
        }
    }

    fn summarize_route_matrix(results: &[MelbetRouteProbeResult]) -> String {
        results
            .iter()
            .map(|result| {
                let readiness = Self::build_readiness_output(
                    &result.probe,
                    &result.bootstrap,
                    &result.status,
                    result.payload_len,
                    &result.extraction.blocker,
                );
                format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                    result.probe.route_hint,
                    result.probe.route_family,
                    result.status.as_str(),
                    result.payload_len,
                    result.bootstrap.transport_hint_count(),
                    result.extraction.source,
                    result.extraction.blocker,
                    readiness.state,
                    readiness.reason,
                    if result.bootstrap.path.is_empty() {
                        "/"
                    } else {
                        result.bootstrap.path.as_str()
                    },
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn summarize_route_matrix_counts(
        results: &[MelbetRouteProbeResult],
    ) -> MelbetRouteMatrixCounts {
        results
            .iter()
            .fold(MelbetRouteMatrixCounts::default(), |mut counts, result| {
                match result.status {
                    MelbetRouteStatus::Ready => counts.ready += 1,
                    MelbetRouteStatus::BootstrapOnly => counts.bootstrap_only += 1,
                    MelbetRouteStatus::Blocked => counts.blocked += 1,
                }
                counts
            })
    }

    fn summarize_runtime_blocker(result: &MelbetRouteProbeResult) -> String {
        let readiness = Self::build_readiness_output(
            &result.probe,
            &result.bootstrap,
            &result.status,
            result.payload_len,
            &result.extraction.blocker,
        );
        let bootstrap_plan = Self::build_bootstrap_acquisition_plan(
            &result.probe,
            &result.bootstrap,
            &result.extraction.blocker,
        );
        let canonical_navigation_failed = result.payload_len == 0
            && result.probe.surface == MelbetSurface::Desktop
            && result.extraction.blocker == "navigation_failed";
        let preserve_early_blocker_path = result.payload_len == 0
            && result.probe.surface == MelbetSurface::Desktop
            && bootstrap_plan.confirmed_blocker == "additional_bootstrap_source_required";
        let summary_status = if preserve_early_blocker_path || canonical_navigation_failed {
            MelbetRouteStatus::Blocked.as_str()
        } else {
            result.status.as_str()
        };
        let (summary_state, summary_reason) = if canonical_navigation_failed {
            (
                "blocked_or_unconfirmed",
                "canonical_navigation_failed_before_bootstrap_capture",
            )
        } else if preserve_early_blocker_path {
            ("blocked_or_unconfirmed", "insufficient_bootstrap_signals")
        } else {
            (readiness.state.as_str(), readiness.reason.as_str())
        };
        let summary_blocker = if canonical_navigation_failed {
            "navigation_failed:retry_known_sportsbook_route"
        } else if result.extraction.blocker.is_empty() {
            "unknown"
        } else {
            result.extraction.blocker.as_str()
        };
        let (confirmed_blocker, next_step) = if canonical_navigation_failed {
            (
                "desktop_live_navigation_failed",
                "retry_known_sportsbook_route",
            )
        } else {
            (
                bootstrap_plan.confirmed_blocker.as_str(),
                bootstrap_plan.next_step.as_str(),
            )
        };

        format!(
            "route_hint={},route_family={},status={},source={},blocker={},confirmed_blocker={},next_step={},target_route={},state={},reason={},payload_len={},final_url={}",
            result.probe.route_hint,
            result.probe.route_family,
            summary_status,
            if result.extraction.source.is_empty() {
                "none"
            } else {
                result.extraction.source.as_str()
            },
            summary_blocker,
            confirmed_blocker,
            next_step,
            bootstrap_plan.primary_target,
            summary_state,
            summary_reason,
            result.payload_len,
            if result.bootstrap.final_url.is_empty() {
                result.probe.url
            } else {
                result.bootstrap.final_url.as_str()
            },
        )
    }

    fn annotate_event_route(event: &mut Event, probe: &HeadlessProbe, status: &MelbetRouteStatus) {
        event.extra.insert(
            "source_url".to_string(),
            serde_json::Value::String(probe.url.to_string()),
        );
        event.extra.insert(
            "melbet_surface".to_string(),
            serde_json::Value::String(probe.surface.as_str().to_string()),
        );
        event.extra.insert(
            "melbet_route_hint".to_string(),
            serde_json::Value::String(probe.route_hint.to_string()),
        );
        event.extra.insert(
            "melbet_route_family".to_string(),
            serde_json::Value::String(probe.route_family.to_string()),
        );
        event.extra.insert(
            "melbet_route_status".to_string(),
            serde_json::Value::String(status.as_str().to_string()),
        );
        event.extra.insert(
            "melbet_transport_hints".to_string(),
            serde_json::Value::Array(
                probe
                    .profile
                    .app_marker
                    .map(|marker| {
                        vec![serde_json::json!({
                            "kind": "app_marker",
                            "value": marker,
                            "source": "profile",
                        })]
                    })
                    .unwrap_or_default(),
            ),
        );
    }

    fn annotate_event_diagnostics(
        event: &mut Event,
        probe: &HeadlessProbe,
        status: &MelbetRouteStatus,
        snapshot: &MelbetBootstrapSnapshot,
        payload_len: usize,
        blocker: &str,
    ) {
        let normalized_transport_hints = snapshot.normalized_transport_hints();
        event.extra.insert(
            "melbet_transport_hints".to_string(),
            serde_json::Value::Array(
                normalized_transport_hints
                    .iter()
                    .take(5)
                    .map(MelbetNormalizedTransportHint::as_json)
                    .collect::<Vec<_>>(),
            ),
        );
        event.extra.insert(
            "melbet_resource_timeline".to_string(),
            serde_json::Value::Array(
                snapshot
                    .resource_timeline
                    .iter()
                    .take(TRANSPORT_HINT_TIMELINE_LIMIT)
                    .map(MelbetResourceTimelineEntry::as_json)
                    .collect::<Vec<_>>(),
            ),
        );
        event.extra.insert(
            "melbet_readiness_diagnostics".to_string(),
            snapshot.readiness.as_json(),
        );
        event.extra.insert(
            "melbet_iframe_sources".to_string(),
            serde_json::Value::Array(
                snapshot
                    .iframe_sources
                    .iter()
                    .take(3)
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
        event.extra.insert(
            "melbet_transport_mapping".to_string(),
            Self::summarize_transport_mapping(snapshot).as_json(),
        );
        event.extra.insert(
            "melbet_runtime_context".to_string(),
            snapshot.runtime_context.as_json(),
        );
        event.extra.insert(
            "melbet_runtime_state".to_string(),
            snapshot.runtime_state.as_json(),
        );
        event.extra.insert(
            "melbet_bootstrap_acquisition_plan".to_string(),
            Self::build_bootstrap_acquisition_plan(probe, snapshot, blocker).as_json(),
        );
        event.extra.insert(
            "melbet_readiness_output".to_string(),
            Self::build_readiness_output(probe, snapshot, status, payload_len, blocker).as_json(),
        );
    }

    fn parse_headless_item(
        item: &serde_json::Value,
        fallback_live: bool,
        probe_url: &str,
    ) -> Option<(Event, Vec<Odd>)> {
        let home_team = item.get("home").and_then(|value| value.as_str())?.trim();
        let away_team = item.get("away").and_then(|value| value.as_str())?.trim();
        if !is_valid_competitor(home_team) || !is_valid_competitor(away_team) {
            return None;
        }

        let odds_values = item
            .get("odds")
            .and_then(|value| value.as_array())?
            .iter()
            .filter_map(|value| value.as_f64())
            .filter(|value| (1.01..=100.0).contains(value))
            .collect::<Vec<_>>();
        if odds_values.len() < 2 {
            return None;
        }

        let raw_id = item
            .get("eventId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}-{}-{}",
                    if fallback_live { "live" } else { "line" },
                    home_team.replace(' ', "_"),
                    away_team.replace(' ', "_")
                )
            });
        let event_id = format!("{BOOKMAKER_SLUG}-{raw_id}");
        let source_url = item
            .get("sourceUrl")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(probe_url);
        let sport = item
            .get("sport")
            .and_then(|value| value.as_str())
            .map(|value| Self::infer_sport(value, source_url))
            .unwrap_or_else(|| Self::infer_sport("", source_url));
        let league = item
            .get("league")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let raw_url = item
            .get("href")
            .and_then(|value| value.as_str())
            .map(|value| Self::normalize_url(value, source_url))
            .unwrap_or_else(|| source_url.to_string());

        let event = Event {
            id: event_id.clone(),
            sport,
            league,
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            start_time: None,
            is_live: fallback_live,
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            raw_url: Some(raw_url),
            extra: HashMap::new(),
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
                    id: format!("{event_id}-{selection}"),
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
            let selections = [
                ("1", OddsType::Home, odds_values[0]),
                ("2", OddsType::Away, odds_values[1]),
            ];
            for (selection, odds_type, value) in selections {
                odds.push(Odd {
                    id: format!("{event_id}-{selection}"),
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

    fn extract_bootstrap_snapshot(tab: &headless_chrome::Tab) -> MelbetBootstrapSnapshot {
        let runtime_state_value = HeadlessChromeHelper::capture_runtime_state(tab);

        HeadlessChromeHelper::capture_session_bootstrap(tab)
            .map(|mut value| {
                if let Some(runtime_state) = runtime_state_value.clone() {
                    if let Some(object) = value.as_object_mut() {
                        object.insert("runtimeState".to_string(), runtime_state);
                    }
                }
                MelbetBootstrapSnapshot::from_value(&value)
            })
            .or_else(|| {
                runtime_state_value.as_ref().and_then(|value| {
                    Self::recovered_bootstrap_snapshot_from_runtime_state(
                        &MelbetRuntimeState::from_value(value),
                    )
                })
            })
            .unwrap_or_else(|| MelbetBootstrapSnapshot {
                final_url: String::new(),
                origin: String::new(),
                path: String::new(),
                referrer: String::new(),
                iframe_sources: Vec::new(),
                title: String::new(),
                body_text_sample: String::new(),
                cookie: String::new(),
                local_storage_keys: Vec::new(),
                session_storage_keys: Vec::new(),
                html_class_list: Vec::new(),
                body_class_list: Vec::new(),
                root_node_ids: Vec::new(),
                meta_viewport: String::new(),
                script_sources: Vec::new(),
                user_agent: String::new(),
                profile_label: String::new(),
                app_marker: String::new(),
                max_touch_points: 0,
                inner_width: 0,
                inner_height: 0,
                has_service_worker: false,
                resource_timeline: Vec::new(),
                transport_hints: Vec::new(),
                runtime_context: MelbetRuntimeContext::default(),
                readiness: MelbetReadinessDiagnostics::from_value(&serde_json::Value::Null),
                runtime_state: MelbetRuntimeState::default(),
            })
    }

    fn extract_embedded_payload(
        helper: &HeadlessChromeHelper,
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
        deadline: Instant,
    ) -> Option<(Vec<serde_json::Value>, MelbetBootstrapSnapshot)> {
        let embedded_route = Self::select_embedded_route(probe, snapshot)?;
        let referer = if snapshot.final_url.is_empty() {
            probe.url
        } else {
            snapshot.final_url.as_str()
        };
        let tab = helper
            .navigate_with_profile_and_referer_with_timeout_and_deadline(
                &embedded_route,
                HEADLESS_WAIT_MS,
                probe.profile,
                Some(referer),
                SPORTSBOOK_NAVIGATION_TIMEOUT_MS,
                deadline,
            )
            .ok()?;
        for _ in 0..HEADLESS_SCROLL_ROUNDS {
            let _ = HeadlessChromeHelper::scroll_page(&tab);
        }
        let payload = Self::extract_headless_payload(&tab);
        let mut embedded_snapshot = Self::extract_bootstrap_snapshot(&tab);
        if embedded_snapshot.final_url.is_empty() {
            embedded_snapshot.final_url = embedded_route;
        }
        Some((payload, embedded_snapshot))
    }

    fn run_blocking_with_wall_clock_timeout<T, F>(
        timeout_ms: u64,
        worker: F,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        T: Send + 'static,
        F: FnOnce(Instant) -> Result<T, Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
    {
        let timeout = Duration::from_millis(timeout_ms.max(1));
        let deadline = Instant::now() + timeout;
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let _ = tx.send(worker(deadline));
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
                "melbet runtime wall clock timeout after {timeout_ms}ms before a useful blocker/result"
            )
            .into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("melbet runtime worker disconnected before returning a blocker/result".into())
            }
        }
    }

    fn fetch_runtime_data_blocking_with_deadline(
        deadline: Instant,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let helper = HeadlessChromeHelper::new()?;
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen_events = HashSet::new();
        let mut seen_odds = HashSet::new();
        let mut route_matrix = Vec::new();

        for probe in Self::runtime_probe_plan() {
            if Instant::now() >= deadline {
                let blocker = route_matrix
                    .first()
                    .map(Self::summarize_runtime_blocker)
                    .unwrap_or_else(|| {
                        format!(
                            "route_hint={},route_family={},status=blocked,source=none,blocker=runtime_budget_exhausted,confirmed_blocker=runtime_budget_exhausted,next_step=reduce_probe_scope,target_route={},state=blocked,reason=wall_clock_cutoff,payload_len=0,final_url={}",
                            probe.route_hint, probe.route_family, probe.url, probe.url,
                        )
                    });
                return Err(blocker.into());
            }

            let tab = match helper.navigate_with_profile_and_referer_with_timeout_and_deadline(
                probe.url,
                HEADLESS_WAIT_MS,
                probe.profile,
                None,
                SPORTSBOOK_NAVIGATION_TIMEOUT_MS,
                deadline,
            ) {
                Ok(tab) => tab,
                Err(error) => {
                    let bootstrap = Self::synthetic_navigation_failure_snapshot(probe);
                    let extraction = MelbetExtractionDiagnostics {
                        source: "none".to_string(),
                        blocker: "navigation_failed".to_string(),
                        ..MelbetExtractionDiagnostics::default()
                    };
                    route_matrix.push(MelbetRouteProbeResult {
                        probe: *probe,
                        status: MelbetRouteStatus::Blocked,
                        payload_len: 0,
                        bootstrap,
                        extraction,
                    });
                    debug!(
                        url = probe.url,
                        route_hint = probe.route_hint,
                        surface = probe.surface.as_str(),
                        error = %error,
                        "Melbet: headless navigation failed"
                    );
                    continue;
                }
            };

            let mut payload = Self::extract_headless_payload(&tab);
            let mut extraction = MelbetExtractionDiagnostics {
                source: if payload.is_empty() {
                    "none".to_string()
                } else {
                    "dom".to_string()
                },
                dom_payload_len: payload.len(),
                blocker: if payload.is_empty() {
                    "dom_payload_empty".to_string()
                } else {
                    "payload_ready".to_string()
                },
                ..MelbetExtractionDiagnostics::default()
            };
            for _ in 0..HEADLESS_SCROLL_ROUNDS {
                let _ = HeadlessChromeHelper::scroll_page(&tab);
                let next_payload = Self::extract_headless_payload(&tab);
                if next_payload.len() > payload.len() {
                    payload = next_payload;
                    extraction.dom_payload_len = payload.len();
                    extraction.source = "dom".to_string();
                    extraction.blocker = "payload_ready".to_string();
                }
            }

            let mut bootstrap = Self::extract_bootstrap_snapshot(&tab);
            let bootstrap_is_empty = Self::has_empty_runtime_bootstrap(&bootstrap);
            if payload.is_empty() {
                if bootstrap_is_empty {
                    extraction.blocker = "empty_bootstrap".to_string();
                } else if let Some(early_cutoff) = Self::runtime_blocker_cutoff_attempt(
                    probe,
                    &bootstrap,
                    Self::select_sportsbook_route(probe, &bootstrap),
                ) {
                    extraction.sportsbook_route = early_cutoff.route.clone();
                    extraction.http_api_seed_count = early_cutoff.seed_count;
                    extraction.http_api_payload_len = early_cutoff.payload.len();
                    extraction.blocker = early_cutoff.blocker.clone();
                    if let Some(early_bootstrap) = early_cutoff.bootstrap {
                        if !early_bootstrap.final_url.is_empty() {
                            bootstrap = early_bootstrap;
                        }
                    }
                    info!(
                        route_hint = probe.route_hint,
                        surface = probe.surface.as_str(),
                        blocker = extraction.blocker,
                        sportsbook_route = extraction.sportsbook_route,
                        runtime_state = Self::summarize_runtime_state(&bootstrap),
                        runtime_context = Self::summarize_runtime_context(&bootstrap),
                        readiness = bootstrap.readiness.as_summary(),
                        "Melbet: early runtime blocker cutoff triggered before extended extraction"
                    );
                } else {
                    extraction.embedded_route =
                        Self::select_embedded_route(probe, &bootstrap).unwrap_or_default();
                    if let Some((embedded_payload, embedded_bootstrap)) =
                        Self::extract_embedded_payload(&helper, probe, &bootstrap, deadline)
                    {
                        extraction.embedded_payload_len = embedded_payload.len();
                        if embedded_payload.len() > payload.len() {
                            payload = embedded_payload;
                            bootstrap = embedded_bootstrap;
                            extraction.source = "embedded".to_string();
                            extraction.blocker = if payload.is_empty() {
                                "embedded_payload_empty"
                            } else {
                                "payload_ready"
                            }
                            .to_string();
                        }
                    }
                }
            }
            if payload.is_empty() && !bootstrap_is_empty {
                let http_api_attempt =
                    Self::extract_sportsbook_http_api_payload(&helper, probe, &bootstrap, deadline);
                extraction.sportsbook_route = http_api_attempt.route.clone();
                extraction.http_api_seed_count = http_api_attempt.seed_count;
                extraction.http_api_payload_len = http_api_attempt.payload.len();
                extraction.blocker = http_api_attempt.blocker.clone();
                if let Some(http_api_bootstrap) = http_api_attempt.bootstrap {
                    if !http_api_bootstrap.final_url.is_empty() {
                        bootstrap = http_api_bootstrap;
                    }
                }
                if http_api_attempt.payload.len() > payload.len() {
                    payload = http_api_attempt.payload;
                    extraction.source = "http_api".to_string();
                }
            }
            let payload_len = payload.len();
            let route_status = Self::classify_route_status(probe, &bootstrap, payload_len);
            route_matrix.push(MelbetRouteProbeResult {
                probe: *probe,
                status: route_status.clone(),
                payload_len,
                bootstrap: bootstrap.clone(),
                extraction: extraction.clone(),
            });

            match route_status {
                MelbetRouteStatus::Blocked => {
                    warn!(
                        url = probe.url,
                        route_hint = probe.route_hint,
                        surface = probe.surface.as_str(),
                        final_url = bootstrap.final_url,
                        title = bootstrap.title,
                        "Melbet: route appears blocked"
                    );
                    continue;
                }
                MelbetRouteStatus::BootstrapOnly => {
                    info!(
                        url = probe.url,
                        route_hint = probe.route_hint,
                        surface = probe.surface.as_str(),
                        cookies_present = !bootstrap.cookie.trim().is_empty(),
                        local_storage_keys = bootstrap.local_storage_keys.len(),
                        session_storage_keys = bootstrap.session_storage_keys.len(),
                        final_path = bootstrap.path,
                        profile_label = bootstrap.profile_label,
                        app_marker = bootstrap.app_marker,
                        extraction = extraction.as_summary(),
                        transport_hints = Self::summarize_transport_hints(&bootstrap),
                        runtime_context = Self::summarize_runtime_context(&bootstrap),
                        runtime_state = Self::summarize_runtime_state(&bootstrap),
                        readiness = bootstrap.readiness.as_summary(),
                        readiness_output = Self::build_readiness_output(
                            probe,
                            &bootstrap,
                            &route_status,
                            payload_len,
                            &extraction.blocker,
                        )
                        .as_summary(),
                        resource_timeline = Self::summarize_resource_timeline(&bootstrap),
                        "Melbet: mobile/webview route bootstrapped without DOM payload"
                    );
                    continue;
                }
                MelbetRouteStatus::Ready => {}
            }

            debug!(
                url = probe.url,
                route_hint = probe.route_hint,
                surface = probe.surface.as_str(),
                items = payload.len(),
                "Melbet: headless payload extracted"
            );

            for item in payload {
                if let Some((mut event, odds)) =
                    Self::parse_headless_item(&item, probe.is_live, probe.url)
                {
                    Self::annotate_event_route(&mut event, probe, &route_status);
                    Self::annotate_event_diagnostics(
                        &mut event,
                        probe,
                        &route_status,
                        &bootstrap,
                        payload_len,
                        &extraction.blocker,
                    );
                    if seen_events.insert(event.id.clone()) {
                        all_events.push(event);
                    }
                    for odd in odds {
                        if seen_odds.insert(odd.id.clone()) {
                            all_odds.push(odd);
                        }
                    }
                }
            }

            if !all_events.is_empty() && probe.surface == MelbetSurface::Desktop {
                info!(
                    route_hint = probe.route_hint,
                    events = all_events.len(),
                    odds = all_odds.len(),
                    "Melbet: stopping probe sweep after first ready desktop route"
                );
                break;
            }
        }

        if !route_matrix.is_empty() {
            let counts = Self::summarize_route_matrix_counts(&route_matrix);
            let bootstrap_hosts = route_matrix
                .iter()
                .filter(|item| item.status == MelbetRouteStatus::BootstrapOnly)
                .map(|item| item.bootstrap.origin.clone())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
                .join(",");

            info!(
                ready = counts.ready,
                bootstrap_only = counts.bootstrap_only,
                blocked = counts.blocked,
                counts = counts.as_summary(),
                bootstrap_hosts,
                matrix = Self::summarize_route_matrix(&route_matrix),
                transport_hints = route_matrix
                    .iter()
                    .map(|item| format!(
                        "{}:{}",
                        item.probe.route_hint,
                        Self::summarize_transport_hints(&item.bootstrap)
                    ))
                    .collect::<Vec<_>>()
                    .join(";"),
                "Melbet: route matrix evaluated"
            );

            if all_events.is_empty() {
                let blocker = route_matrix
                    .first()
                    .map(Self::summarize_runtime_blocker)
                    .unwrap_or_else(|| "route_hint=unknown,blocker=no_route_result".to_string());
                warn!(
                    counts = counts.as_summary(),
                    matrix = Self::summarize_route_matrix(&route_matrix),
                    runtime_context = route_matrix
                        .iter()
                        .map(|item| format!(
                            "{}:{}",
                            item.probe.route_hint,
                            Self::summarize_runtime_context(&item.bootstrap)
                        ))
                        .collect::<Vec<_>>()
                        .join(";"),
                    runtime_state = route_matrix
                        .iter()
                        .map(|item| format!(
                            "{}:{}",
                            item.probe.route_hint,
                            Self::summarize_runtime_state(&item.bootstrap)
                        ))
                        .collect::<Vec<_>>()
                        .join(";"),
                    blocker,
                    "Melbet: focused runtime route yielded no events"
                );
                return Err(blocker.into());
            }
        }

        Ok((all_events, all_odds))
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let _ = &self.client;
        let (events, odds) = tokio::task::spawn_blocking(move || {
            Self::run_blocking_with_wall_clock_timeout(
                MELBET_RUNTIME_WALL_CLOCK_TIMEOUT_MS,
                Self::fetch_runtime_data_blocking_with_deadline,
            )
        })
        .await
        .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })??;

        let live_count = events.iter().filter(|event| event.is_live).count();
        let prematch_count = events.len().saturating_sub(live_count);
        info!(
            total = events.len(),
            live = live_count,
            prematch = prematch_count,
            odds = odds.len(),
            probes = HEADLESS_PROBES.len(),
            "Melbet: runtime data collected from headless DOM extraction"
        );

        Ok((events, odds))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MelbetBootstrapSnapshot, MelbetParser, MelbetReadinessDiagnostics, MelbetRouteProbeResult,
        MelbetRouteStatus, HEADLESS_PROBES, MELBET_RUNTIME_WALL_CLOCK_TIMEOUT_MS,
        SPORTSBOOK_BASE_URL, SPORTSBOOK_HOME_URL,
    };
    use shared::Sport;
    use std::time::Duration;

    #[test]
    fn parses_fixture_three_way_headless_payload() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.first())
            .expect("first fixture item");

        let (event, odds) = MelbetParser::parse_headless_item(item, true, "https://melbet.ru/live")
            .expect("headless item");

        assert_eq!(event.id, "melbet-7845123");
        assert_eq!(event.sport, Sport::Football);
        assert!(event.is_live);
        assert_eq!(event.league, "England. Premier League");
        assert_eq!(odds.len(), 3);
        assert_eq!(odds[0].selection, "1");
        assert_eq!(odds[1].selection, "X");
        assert_eq!(odds[2].selection, "2");
        assert!(event.extra.is_empty());
    }

    #[test]
    fn parses_fixture_two_way_headless_payload() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.get(1))
            .expect("second fixture item");

        let (event, odds) =
            MelbetParser::parse_headless_item(item, false, "https://melbet.ru/line")
                .expect("headless item");

        assert_eq!(event.id, "melbet-line-Novak_Djokovic-Daniil_Medvedev");
        assert_eq!(event.sport, Sport::Tennis);
        assert!(!event.is_live);
        assert_eq!(odds.len(), 2);
        assert_eq!(odds[0].market, "Moneyline");
        assert_eq!(odds[0].selection, "1");
        assert_eq!(odds[1].selection, "2");
    }

    #[test]
    fn parses_iframe_style_headless_payload() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.get(3))
            .expect("fourth fixture item");

        let (event, odds) = MelbetParser::parse_headless_item(
            item,
            true,
            "https://sport.melbet.ru/partner/SportsBook/Home",
        )
        .expect("iframe headless item");

        assert_eq!(event.id, "melbet-36672593");
        assert_eq!(event.sport, Sport::Football);
        assert_eq!(event.league, "UEFA Champions League");
        assert_eq!(odds.len(), 3);
        assert_eq!(odds[0].odds, 3.81);
        assert_eq!(odds[1].selection, "X");
        assert_eq!(odds[2].odds, 1.77);
    }

    #[test]
    fn parses_sportsbook_live_http_api_fixture_payload() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.get(4))
            .expect("fifth fixture item");

        let (event, odds) = MelbetParser::parse_headless_item(item, true, SPORTSBOOK_BASE_URL)
            .expect("sportsbook http api live item");

        assert_eq!(event.id, "melbet-36649170");
        assert_eq!(event.sport, Sport::Football);
        assert!(event.is_live);
        assert_eq!(event.league, "England. Development League. U21");
        assert_eq!(event.raw_url.as_deref(), Some(SPORTSBOOK_BASE_URL));
        assert_eq!(odds.len(), 3);
        assert_eq!(odds[0].odds, 33.0);
        assert_eq!(odds[1].selection, "X");
        assert_eq!(odds[2].odds, 1.02);
    }

    #[test]
    fn parses_sportsbook_prematch_http_api_fixture_payload() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.get(5))
            .expect("sixth fixture item");

        let (event, odds) = MelbetParser::parse_headless_item(item, false, SPORTSBOOK_BASE_URL)
            .expect("sportsbook http api prematch item");

        assert_eq!(event.id, "melbet-36526641");
        assert_eq!(event.sport, Sport::Football);
        assert!(!event.is_live);
        assert_eq!(event.league, "UEFA Champions League");
        assert_eq!(event.raw_url.as_deref(), Some(SPORTSBOOK_BASE_URL));
        assert_eq!(odds.len(), 3);
        assert_eq!(odds[0].odds, 3.8);
        assert_eq!(odds[1].odds, 4.64);
        assert_eq!(odds[2].odds, 1.76);
    }

    #[test]
    fn extracts_bootstrap_snapshot_storage_keys() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "iframeSources": [
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D"
            ],
            "title": "Melbet Live",
            "bodyTextSample": "Live events",
            "cookie": "SESSION=abc123",
            "htmlClassList": ["hydrated", "app-root"],
            "bodyClassList": ["page-live"],
            "rootNodeIds": ["app"],
            "metaViewport": "width=device-width,initial-scale=1",
            "localStorage": {
                "app.session": "1",
                "device.id": "2"
            },
            "sessionStorage": {
                "bootstrap.route": "live"
            },
            "scriptSources": ["/assets/app.js", "/assets/chunk-vendors.js"],
            "userAgent": super::WEBVIEW_USER_AGENT,
            "profileLabel": "webview",
            "appMarker": "com.melbet.app",
            "maxTouchPoints": 5,
            "innerWidth": 412,
            "innerHeight": 915,
            "hasServiceWorker": true
        }));

        assert_eq!(snapshot.final_url, "https://melbet.ru/live");
        assert_eq!(snapshot.storage_key_count(), 3);
        assert_eq!(snapshot.origin, "https://melbet.ru");
        assert_eq!(snapshot.path, "/live");
        assert_eq!(
            snapshot.local_storage_keys,
            vec!["app.session", "device.id"]
        );
        assert_eq!(snapshot.session_storage_keys, vec!["bootstrap.route"]);
        assert_eq!(snapshot.profile_label, "webview");
        assert_eq!(snapshot.app_marker, "com.melbet.app");
        assert_eq!(snapshot.max_touch_points, 5);
        assert_eq!(snapshot.root_node_ids, vec!["app"]);
        assert_eq!(snapshot.iframe_source_count(), 1);
        assert_eq!(snapshot.transport_hint_count(), 0);
        assert!(!snapshot.runtime_context.has_http_api);
    }

    #[test]
    fn selects_embedded_sportsbook_route_from_bootstrap_snapshot() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/ru/sport",
            "origin": "https://melbet.ru",
            "path": "/ru/sport",
            "iframeSources": [
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D",
                "https://example.com/other"
            ]
        }));

        assert!(MelbetParser::route_matches_probe(
            &HEADLESS_PROBES[0],
            &snapshot
        ));
        assert_eq!(
            MelbetParser::select_embedded_route(&HEADLESS_PROBES[0], &snapshot),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D"
                    .to_string()
            )
        );
    }

    #[test]
    fn selects_direct_sportsbook_route_from_shell_markers() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/ru/sport",
            "origin": "https://melbet.ru",
            "path": "/ru/sport",
            "title": "melbet.ru",
            "scriptSources": [
                "https://cdn.dgbuilder1.ru/version/0.7.460/main.js"
            ],
            "rootNodeIds": ["root"]
        }));

        assert!(MelbetParser::has_sportsbook_shell_markers(&snapshot));
        assert_eq!(
            MelbetParser::select_sportsbook_route(&HEADLESS_PROBES[0], &snapshot),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
                    .to_string()
            )
        );
        assert_eq!(
            MelbetParser::select_sportsbook_route(&HEADLESS_PROBES[2], &snapshot),
            None
        );
    }

    #[test]
    fn expands_desktop_live_sportsbook_route_candidates_from_bootstrap_markers() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "bodyTextSample": "",
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": false,
                "inlineScriptCount": 1,
                "bootstrapMarkers": ["inline:$globalSettings"]
            },
            "readinessDiagnostics": {
                "readyState": "complete",
                "fetchLikeCount": 1,
                "hasVisibleAppShell": true
            }
        }));

        let candidates = MelbetParser::sportsbook_route_candidates(&HEADLESS_PROBES[0], &snapshot);

        assert_eq!(
            candidates.first().map(String::as_str),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
            )
        );
        assert!(candidates.iter().any(|route| route == SPORTSBOOK_HOME_URL));
    }

    #[test]
    fn collects_sportsbook_route_candidates_from_resource_timeline() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "resourceTimeline": [
                {
                    "name": "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Fline%22%7D",
                    "initiatorType": "iframe",
                    "nextHopProtocol": "h2",
                    "transferSize": 1024,
                    "durationMs": 11,
                    "startTimeMs": 3,
                    "responseEndMs": 14
                }
            ]
        }));

        assert_eq!(
            MelbetParser::select_sportsbook_route(&HEADLESS_PROBES[0], &snapshot),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Fline%22%7D"
                    .to_string()
            )
        );
    }

    #[test]
    fn focuses_default_desktop_sportsbook_routes() {
        let candidates = MelbetParser::default_sportsbook_route_candidates(&HEADLESS_PROBES[0]);

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates.first().map(String::as_str),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
            )
        );
        assert_eq!(
            candidates.get(1).map(String::as_str),
            Some(SPORTSBOOK_HOME_URL)
        );
    }

    #[test]
    fn detects_empty_runtime_bootstrap_snapshot() {
        let empty = MelbetBootstrapSnapshot::from_value(&serde_json::Value::Null);
        let bootstrap = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "path": "/live",
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "bodyChildCount": 1,
                "bodyTextLength": 8
            }
        }));

        assert!(MelbetParser::has_empty_runtime_bootstrap(&empty));
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&bootstrap));
    }

    #[test]
    fn reuses_bootstrap_snapshot_when_sportsbook_navigation_fails() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/ru/sport",
            "origin": "https://melbet.ru",
            "path": "/ru/sport",
            "title": "Melbet sportsbook",
            "scriptSources": [
                "https://cdn.dgbuilder1.ru/version/0.7.460/main.js"
            ],
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": true,
                "inlineScriptCount": 1,
                "bootstrapMarkers": ["inline:$globalSettings"]
            },
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 32,
                "bodyChildCount": 4,
                "resourceCount": 1,
                "fetchLikeCount": 1,
                "hasVisibleAppShell": true
            },
            "runtimeState": {
                "href": "https://melbet.ru/ru/sport",
                "pathname": "/ru/sport",
                "title": "Melbet sportsbook",
                "readyState": "complete",
                "bodyChildCount": 4,
                "bodyTextLength": 32,
                "customElementCount": 2,
                "buttonCount": 1,
                "linkCount": 3,
                "routeLinkCount": 1,
                "routerShellCount": 2,
                "bodyTextSample": "Sport Live Prematch"
            }
        }));

        let attempt = MelbetParser::sportsbook_navigation_failure_fallback(
            &HEADLESS_PROBES[0],
            &snapshot,
            Some(SPORTSBOOK_HOME_URL.to_string()),
        );

        assert_eq!(attempt.route, SPORTSBOOK_HOME_URL);
        assert_eq!(
            attempt.blocker,
            "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
        assert_eq!(
            attempt
                .bootstrap
                .as_ref()
                .map(|item| item.final_url.as_str()),
            Some("https://melbet.ru/ru/sport")
        );
        assert_eq!(
            MelbetParser::classify_route_status(
                &HEADLESS_PROBES[0],
                attempt.bootstrap.as_ref().expect("bootstrap snapshot"),
                attempt.payload.len(),
            ),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn short_circuits_http_api_route_probe_when_bootstrap_source_is_still_missing() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Melbet Live",
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": false,
                "inlineScriptCount": 1,
                "bootstrapMarkers": ["inline:$globalSettings"]
            },
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 24,
                "bodyChildCount": 3,
                "resourceCount": 1,
                "fetchLikeCount": 1,
                "hasVisibleAppShell": true
            },
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Melbet Live",
                "readyState": "complete",
                "bodyChildCount": 3,
                "bodyTextLength": 24,
                "routeLinkCount": 1,
                "routerShellCount": 1,
                "bodyTextSample": "Live events"
            }
        }));

        assert!(MelbetParser::should_short_circuit_sportsbook_http_api(
            &snapshot, true
        ));

        let fallback = MelbetParser::sportsbook_navigation_failure_fallback(
            &HEADLESS_PROBES[0],
            &snapshot,
            Some(SPORTSBOOK_HOME_URL.to_string()),
        );

        assert_eq!(fallback.payload.len(), 0);
        assert_eq!(fallback.seed_count, 0);
        assert_eq!(fallback.route, SPORTSBOOK_HOME_URL);
        assert_eq!(
            fallback.blocker,
            "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
        assert_eq!(
            fallback
                .bootstrap
                .as_ref()
                .map(|item| item.final_url.as_str()),
            Some("https://melbet.ru/live")
        );
    }

    #[test]
    fn desktop_live_navigation_fallback_keeps_useful_blocker_for_sportsbook_shell_route() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D",
            "origin": "https://sport.melbet.ru",
            "path": "/partner/SportsBook/Home",
            "title": "Melbet sportsbook",
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": true,
                "inlineScriptCount": 1,
                "bootstrapMarkers": ["inline:$globalSettings", "route_family:href:sportsbook"]
            },
            "readinessDiagnostics": {
                "readyState": "interactive",
                "bodyTextLength": 0,
                "bodyChildCount": 0,
                "fetchLikeCount": 1,
                "hasVisibleAppShell": true
            },
            "runtimeState": {
                "href": "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D",
                "pathname": "/partner/SportsBook/Home",
                "title": "Melbet sportsbook",
                "readyState": "interactive",
                "bodyChildCount": 0,
                "bodyTextLength": 0,
                "routeLinkCount": 1,
                "routerShellCount": 1,
                "bodyTextSample": ""
            }
        }));

        let fallback = MelbetParser::sportsbook_navigation_failure_fallback(
            &HEADLESS_PROBES[0],
            &snapshot,
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
                    .to_string(),
            ),
        );

        assert_eq!(
            fallback.blocker,
            "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
        assert_eq!(
            fallback.bootstrap.as_ref().map(|item| item.final_url.as_str()),
            Some(
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
            )
        );
        assert_eq!(
            MelbetParser::classify_route_status(
                &HEADLESS_PROBES[0],
                fallback.bootstrap.as_ref().expect("sportsbook bootstrap"),
                fallback.payload.len(),
            ),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn short_circuits_http_api_route_probe_when_runtime_blocker_is_detected() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Just a moment...",
            "bodyTextSample": "Checking your browser before accessing Melbet",
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Just a moment...",
                "readyState": "complete",
                "historyLength": 2,
                "bodyChildCount": 1,
                "bodyTextLength": 46,
                "blocker": {
                    "kind": "cloudflare_challenge",
                    "source": "title",
                    "matchedText": "Just a moment..."
                }
            }
        }));

        assert!(MelbetParser::should_short_circuit_sportsbook_http_api(
            &snapshot, true
        ));

        let fallback = MelbetParser::sportsbook_navigation_failure_fallback(
            &HEADLESS_PROBES[0],
            &snapshot,
            Some(SPORTSBOOK_HOME_URL.to_string()),
        );

        assert_eq!(fallback.payload.len(), 0);
        assert_eq!(fallback.seed_count, 0);
        assert_eq!(fallback.route, SPORTSBOOK_HOME_URL);
        assert_eq!(
            fallback.blocker,
            "runtime_blocker:cloudflare_challenge@title"
        );
        assert_eq!(
            fallback
                .bootstrap
                .as_ref()
                .map(|item| item.final_url.as_str()),
            Some("https://melbet.ru/live")
        );
    }

    #[test]
    fn builds_early_runtime_cutoff_attempt_for_missing_bootstrap_source() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Melbet Live",
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": false,
                "inlineScriptCount": 1,
                "bootstrapMarkers": ["inline:$globalSettings"]
            },
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 24,
                "bodyChildCount": 3,
                "resourceCount": 1,
                "fetchLikeCount": 1,
                "hasVisibleAppShell": true
            },
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Melbet Live",
                "readyState": "complete",
                "bodyChildCount": 3,
                "bodyTextLength": 24,
                "routeLinkCount": 1,
                "routerShellCount": 1,
                "bodyTextSample": "Live events"
            }
        }));

        let attempt = MelbetParser::runtime_blocker_cutoff_attempt(
            &HEADLESS_PROBES[0],
            &snapshot,
            Some(SPORTSBOOK_HOME_URL.to_string()),
        )
        .expect("early cutoff attempt");

        assert_eq!(attempt.payload.len(), 0);
        assert_eq!(attempt.seed_count, 0);
        assert_eq!(attempt.route, SPORTSBOOK_HOME_URL);
        assert_eq!(
            attempt.blocker,
            "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
    }

    #[test]
    fn classifies_sparse_desktop_live_runtime_as_bootstrap_only() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Melbet Live",
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": false,
                "hasPartnerConfig": false,
                "inlineScriptCount": 0,
                "bootstrapMarkers": []
            },
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 18,
                "bodyChildCount": 2,
                "resourceCount": 0,
                "fetchLikeCount": 0,
                "hasVisibleAppShell": false
            },
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Melbet Live",
                "readyState": "complete",
                "bodyChildCount": 2,
                "bodyTextLength": 18,
                "linkCount": 1,
                "routeLinkCount": 0,
                "routerShellCount": 0,
                "bodyTextSample": "Live events",
                "bootstrapMarkers": []
            }
        }));

        assert!(MelbetParser::has_useful_desktop_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn detects_block_page_from_snapshot() {
        let snapshot = MelbetBootstrapSnapshot {
            final_url: "https://melbet.ru/blocked".to_string(),
            origin: "https://melbet.ru".to_string(),
            path: "/blocked".to_string(),
            referrer: String::new(),
            iframe_sources: Vec::new(),
            title: "Access denied".to_string(),
            body_text_sample: "Please complete the captcha to continue".to_string(),
            cookie: String::new(),
            local_storage_keys: Vec::new(),
            session_storage_keys: Vec::new(),
            html_class_list: Vec::new(),
            body_class_list: Vec::new(),
            root_node_ids: Vec::new(),
            meta_viewport: String::new(),
            script_sources: Vec::new(),
            user_agent: String::new(),
            profile_label: String::new(),
            app_marker: String::new(),
            max_touch_points: 0,
            inner_width: 0,
            inner_height: 0,
            has_service_worker: false,
            resource_timeline: Vec::new(),
            transport_hints: Vec::new(),
            runtime_context: super::MelbetRuntimeContext::default(),
            readiness: MelbetReadinessDiagnostics::from_value(&serde_json::Value::Null),
            runtime_state: super::MelbetRuntimeState::default(),
        };

        assert!(MelbetParser::looks_like_block_page(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[4], &snapshot, 0),
            MelbetRouteStatus::Blocked
        );
    }

    #[test]
    fn classifies_mobile_route_as_bootstrap_only_without_payload() {
        let snapshot = MelbetBootstrapSnapshot {
            final_url: "https://melbet.ru/m/live".to_string(),
            origin: "https://melbet.ru".to_string(),
            path: "/m/live".to_string(),
            referrer: String::new(),
            iframe_sources: Vec::new(),
            title: "Melbet Live".to_string(),
            body_text_sample: "Live events and coupons".to_string(),
            cookie: "SESSION=abc123".to_string(),
            local_storage_keys: vec!["app.session".to_string()],
            session_storage_keys: vec!["bootstrap.route".to_string()],
            html_class_list: vec!["hydrated".to_string(), "app-root".to_string()],
            body_class_list: vec!["page-live".to_string()],
            root_node_ids: vec!["app".to_string()],
            meta_viewport: "width=device-width,initial-scale=1".to_string(),
            script_sources: vec!["/assets/app.js".to_string()],
            user_agent: super::MOBILE_USER_AGENT.to_string(),
            profile_label: "mobile".to_string(),
            app_marker: String::new(),
            max_touch_points: 5,
            inner_width: 412,
            inner_height: 915,
            has_service_worker: true,
            resource_timeline: vec![super::MelbetResourceTimelineEntry {
                name: "https://melbet.ru/m/live-feed".to_string(),
                initiator_type: "fetch".to_string(),
                next_hop_protocol: "h2".to_string(),
                transfer_size: 2_048,
                duration_ms: 88,
                start_time_ms: 12,
                response_end_ms: 100,
            }],
            transport_hints: vec![super::MelbetTransportHint {
                kind: "data_endpoint".to_string(),
                value: "https://melbet.ru/m/live-feed".to_string(),
                source: "resource".to_string(),
            }],
            runtime_context: super::MelbetRuntimeContext::default(),
            readiness: MelbetReadinessDiagnostics {
                ready_state: "complete".to_string(),
                body_text_length: 24,
                body_child_count: 3,
                resource_count: 4,
                script_count: 1,
                storage_key_count: 2,
                root_node_count: 1,
                fetch_like_count: 1,
                websocket_hint_count: 0,
                dom_content_loaded_ms: 350,
                load_event_ms: 610,
                last_resource_end_ms: 100,
                has_visible_app_shell: true,
            },
            runtime_state: super::MelbetRuntimeState::default(),
        };

        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[2], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::Blocked
        );
    }

    #[test]
    fn annotates_event_with_route_metadata() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.get(2))
            .expect("third fixture item");

        let (mut event, _) =
            MelbetParser::parse_headless_item(item, true, "https://melbet.ru/live")
                .expect("headless item");
        MelbetParser::annotate_event_route(
            &mut event,
            &HEADLESS_PROBES[4],
            &MelbetRouteStatus::Ready,
        );

        assert_eq!(
            event.extra.get("melbet_surface"),
            Some(&serde_json::Value::String("webview".to_string()))
        );
        assert_eq!(
            event.extra.get("melbet_route_hint"),
            Some(&serde_json::Value::String("webview-live".to_string()))
        );
        assert_eq!(
            event.extra.get("melbet_route_family"),
            Some(&serde_json::Value::String("webview-shell-live".to_string()))
        );
        assert_eq!(
            event.extra.get("melbet_route_status"),
            Some(&serde_json::Value::String("ready".to_string()))
        );
    }

    #[test]
    fn classifies_webview_shell_as_bootstrap_only_without_dom_payload() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://m.melbet.com/live",
            "origin": "https://m.melbet.com",
            "path": "/live",
            "title": "Melbet app shell",
            "bodyTextSample": "Loading live data",
            "cookie": "SESSION=wv-123",
            "localStorage": {
                "app.token": "1"
            },
            "sessionStorage": {
                "route.name": "live"
            },
            "htmlClassList": ["app-shell"],
            "rootNodeIds": ["root"],
            "metaViewport": "width=device-width,initial-scale=1",
            "scriptSources": ["https://m.melbet.com/assets/app.js", "https://m.melbet.com/assets/chunk.js", "https://m.melbet.com/assets/runtime.js"],
            "resourceTimeline": [
                {
                    "name": "https://m.melbet.com/api/live-feed",
                    "initiatorType": "fetch",
                    "nextHopProtocol": "h2",
                    "transferSize": 4096,
                    "durationMs": 72,
                    "startTimeMs": 15,
                    "responseEndMs": 87
                }
            ],
            "transportHints": [
                {
                    "kind": "data_endpoint",
                    "value": "https://m.melbet.com/api/live-feed",
                    "source": "resource"
                }
            ],
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 18,
                "resourceCount": 3,
                "fetchLikeCount": 1,
                "websocketHintCount": 0,
                "domContentLoadedMs": 440,
                "loadEventMs": 700,
                "lastResourceEndMs": 87,
                "hasVisibleAppShell": true
            },
            "userAgent": super::WEBVIEW_USER_AGENT,
            "profileLabel": "webview",
            "appMarker": "com.melbet.app",
            "maxTouchPoints": 5,
            "innerWidth": 412,
            "innerHeight": 915,
            "hasServiceWorker": true
        }));

        assert!(MelbetParser::route_matches_probe(
            &HEADLESS_PROBES[4],
            &snapshot
        ));
        assert!(MelbetParser::has_bootstrap_markers(
            &HEADLESS_PROBES[4],
            &snapshot
        ));
        assert_eq!(snapshot.transport_hint_count(), 1);
        assert_eq!(snapshot.resource_timeline_count(), 1);
        assert_eq!(snapshot.readiness.fetch_like_count, 1);
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[4], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn summarizes_route_matrix_with_statuses() {
        let bootstrap = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://m.melbet.com/live",
            "origin": "https://m.melbet.com",
            "path": "/live"
        }));
        let matrix = vec![
            MelbetRouteProbeResult {
                probe: HEADLESS_PROBES[0],
                status: MelbetRouteStatus::Ready,
                payload_len: 7,
                bootstrap: bootstrap.clone(),
                extraction: super::MelbetExtractionDiagnostics {
                    source: "dom".to_string(),
                    dom_payload_len: 7,
                    blocker: "payload_ready".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
            MelbetRouteProbeResult {
                probe: HEADLESS_PROBES[4],
                status: MelbetRouteStatus::BootstrapOnly,
                payload_len: 0,
                bootstrap,
                extraction: super::MelbetExtractionDiagnostics {
                    source: "none".to_string(),
                    blocker: "no_http_api_runtime:additional_bootstrap_source_required".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
        ];

        assert_eq!(
            MelbetParser::summarize_route_matrix(&matrix),
            "desktop-live:canonical-live:ready:7:0:dom:payload_ready:dom_payload_ready:rendered_dom_payload_detected:/live,webview-live:webview-shell-live:bootstrap_only:0:0:none:no_http_api_runtime:additional_bootstrap_source_required:shell_bootstrapped:bootstrap_signals_without_dom_payload:/live"
        );
    }

    #[test]
    fn summarizes_route_matrix_counts_by_status() {
        let bootstrap = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://m.melbet.com/live",
            "origin": "https://m.melbet.com",
            "path": "/live"
        }));
        let matrix = vec![
            MelbetRouteProbeResult {
                probe: HEADLESS_PROBES[0],
                status: MelbetRouteStatus::Ready,
                payload_len: 7,
                bootstrap: bootstrap.clone(),
                extraction: super::MelbetExtractionDiagnostics {
                    source: "dom".to_string(),
                    dom_payload_len: 7,
                    blocker: "payload_ready".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
            MelbetRouteProbeResult {
                probe: HEADLESS_PROBES[2],
                status: MelbetRouteStatus::BootstrapOnly,
                payload_len: 0,
                bootstrap: bootstrap.clone(),
                extraction: super::MelbetExtractionDiagnostics {
                    source: "none".to_string(),
                    blocker: "no_sportsbook_route".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
            MelbetRouteProbeResult {
                probe: HEADLESS_PROBES[4],
                status: MelbetRouteStatus::Blocked,
                payload_len: 0,
                bootstrap,
                extraction: super::MelbetExtractionDiagnostics {
                    source: "none".to_string(),
                    blocker: "dom_payload_empty".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
        ];

        let counts = MelbetParser::summarize_route_matrix_counts(&matrix);
        assert_eq!(
            counts,
            super::MelbetRouteMatrixCounts {
                ready: 1,
                bootstrap_only: 1,
                blocked: 1,
            }
        );
        assert_eq!(counts.as_summary(), "ready=1,bootstrap_only=1,blocked=1");
    }

    #[test]
    fn runtime_probe_plan_focuses_desktop_live_route() {
        let plan = MelbetParser::runtime_probe_plan();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].route_hint, "desktop-live");
        assert_eq!(plan[0].route_family, "canonical-live");
    }

    #[test]
    fn builds_synthetic_snapshot_for_navigation_failure() {
        let snapshot = MelbetParser::synthetic_navigation_failure_snapshot(&HEADLESS_PROBES[0]);

        assert_eq!(snapshot.final_url, "https://melbet.ru/live");
        assert_eq!(snapshot.origin, "https://melbet.ru");
        assert_eq!(snapshot.path, "/live");
        assert_eq!(snapshot.profile_label, "desktop");
        assert_eq!(snapshot.user_agent, super::DESKTOP_USER_AGENT);
        assert_eq!(snapshot.inner_width, 1440);
        assert_eq!(snapshot.inner_height, 2200);
        assert_eq!(snapshot.transport_hint_count(), 0);
        assert_eq!(snapshot.resource_timeline_count(), 0);
    }

    #[test]
    fn summarizes_runtime_blocker_for_focused_route() {
        let bootstrap = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live"
        }));
        let blocker = MelbetParser::summarize_runtime_blocker(&MelbetRouteProbeResult {
            probe: HEADLESS_PROBES[0],
            status: MelbetRouteStatus::Blocked,
            payload_len: 0,
            bootstrap,
            extraction: super::MelbetExtractionDiagnostics {
                source: "none".to_string(),
                blocker: "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required".to_string(),
                ..super::MelbetExtractionDiagnostics::default()
            },
        });

        assert_eq!(
            blocker,
            "route_hint=desktop-live,route_family=canonical-live,status=blocked,source=none,blocker=missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required,confirmed_blocker=additional_bootstrap_source_required,next_step=manual_bootstrap_acquisition,target_route=https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D,state=blocked_or_unconfirmed,reason=insufficient_bootstrap_signals,payload_len=0,final_url=https://melbet.ru/live"
        );
    }

    #[test]
    fn preserves_early_blocker_summary_when_classifier_keeps_bootstrap_only() {
        let bootstrap = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Melbet Live",
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 24,
                "bodyChildCount": 3,
                "hasVisibleAppShell": true
            },
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Melbet Live",
                "readyState": "complete",
                "bodyChildCount": 3,
                "bodyTextLength": 24,
                "routeLinkCount": 1,
                "routerShellCount": 1,
                "bodyTextSample": "Live events"
            }
        }));

        let blocker = MelbetParser::summarize_runtime_blocker(&MelbetRouteProbeResult {
            probe: HEADLESS_PROBES[0],
            status: MelbetRouteStatus::BootstrapOnly,
            payload_len: 0,
            bootstrap,
            extraction: super::MelbetExtractionDiagnostics {
                source: "none".to_string(),
                blocker: "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required".to_string(),
                ..super::MelbetExtractionDiagnostics::default()
            },
        });

        assert!(blocker.contains("status=blocked"));
        assert!(blocker.contains("confirmed_blocker=additional_bootstrap_source_required"));
        assert!(blocker.contains("next_step=manual_bootstrap_acquisition"));
        assert!(blocker.contains("state=blocked_or_unconfirmed"));
    }

    #[test]
    fn rewrites_desktop_live_navigation_failure_into_actionable_blocker() {
        let bootstrap = MelbetParser::synthetic_navigation_failure_snapshot(&HEADLESS_PROBES[0]);

        let blocker = MelbetParser::summarize_runtime_blocker(&MelbetRouteProbeResult {
            probe: HEADLESS_PROBES[0],
            status: MelbetRouteStatus::Blocked,
            payload_len: 0,
            bootstrap,
            extraction: super::MelbetExtractionDiagnostics {
                source: "none".to_string(),
                blocker: "navigation_failed".to_string(),
                ..super::MelbetExtractionDiagnostics::default()
            },
        });

        assert!(blocker.contains("blocker=navigation_failed:retry_known_sportsbook_route"));
        assert!(blocker.contains("confirmed_blocker=desktop_live_navigation_failed"));
        assert!(blocker.contains("next_step=retry_known_sportsbook_route"));
        assert!(blocker.contains("reason=canonical_navigation_failed_before_bootstrap_capture"));
        assert!(blocker.contains(
            "target_route=https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
        ));
    }

    #[test]
    fn builds_manual_bootstrap_acquisition_plan_for_confirmed_blocker() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/ru/sport",
            "origin": "https://melbet.ru",
            "path": "/ru/sport",
            "referrer": "https://melbet.ru/live",
            "iframeSources": [
                "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D"
            ],
            "runtimeContext": {
                "hasHttpApi": false,
                "httpApiMethods": [],
                "partnerId": 0,
                "langId": 0,
                "countryCode": "",
                "hasGlobalSettings": true,
                "hasPartnerConfig": true,
                "inlineScriptCount": 2,
                "bootstrapMarkers": ["inline:$globalSettings", "inline:$httpApi"]
            }
        }));

        let plan = MelbetParser::build_bootstrap_acquisition_plan(
            &HEADLESS_PROBES[0],
            &snapshot,
            "no_http_api_runtime:additional_bootstrap_source_required",
        )
        .as_json();

        assert_eq!(
            plan.get("confirmedBlocker")
                .and_then(|value| value.as_str()),
            Some("additional_bootstrap_source_required")
        );
        assert_eq!(
            plan.get("nextStep").and_then(|value| value.as_str()),
            Some("manual_bootstrap_acquisition")
        );
        assert_eq!(
            plan.get("primaryTarget").and_then(|value| value.as_str()),
            Some(SPORTSBOOK_HOME_URL)
        );
        assert_eq!(
            plan.get("referer").and_then(|value| value.as_str()),
            Some("https://melbet.ru/live")
        );
        assert!(plan
            .get("routeCandidates")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| {
                item.as_str()
                    == Some(
                        "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D"
                    )
            })));
        assert!(plan
            .get("requiredRuntimeFields")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items
                .iter()
                .any(|item| item.as_str() == Some("getTopLiveSports"))));
    }

    #[test]
    fn classifies_live_http_api_context_gap_with_missing_methods_and_ids() {
        let context = super::MelbetRuntimeContext::from_value(&serde_json::json!({
            "hasHttpApi": true,
            "httpApiMethods": ["getTopEvents"],
            "partnerId": 0,
            "langId": 1,
            "countryCode": "RU",
            "hasGlobalSettings": true,
            "hasPartnerConfig": true
        }));

        assert_eq!(
            context.missing_http_api_requirements(true),
            vec!["partner_id", "getTopLiveSports", "getTopLiveEvents"]
        );
        assert_eq!(
            context.http_api_blocker(true),
            "missing_http_api_context:partner_id|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
    }

    #[test]
    fn classifies_live_http_api_runtime_gap_without_bootstrap_markers() {
        let context = super::MelbetRuntimeContext::from_value(&serde_json::json!({
            "hasHttpApi": false,
            "httpApiMethods": [],
            "partnerId": 0,
            "langId": 0,
            "countryCode": "",
            "hasGlobalSettings": false,
            "hasPartnerConfig": false,
            "inlineScriptCount": 0,
            "bootstrapMarkers": []
        }));

        assert_eq!(
            context.http_api_blocker(true),
            "missing_http_api_context:http_api_runtime|partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents"
        );
    }

    #[test]
    fn parses_transport_hints_and_readiness_diagnostics_from_snapshot() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://m.melbet.com/live",
            "origin": "https://m.melbet.com",
            "path": "/live",
            "runtimeContext": {
                "hasHttpApi": true,
                "httpApiMethods": ["getTopEvents", "getTopLiveEvents", "getTopLiveSports"],
                "partnerId": 532,
                "langId": 1,
                "countryCode": "RU",
                "hasGlobalSettings": true,
                "hasPartnerConfig": true,
                "inlineScriptCount": 2,
                "bootstrapMarkers": ["inline:$globalSettings", "inline:partnerId"]
            },
            "resourceTimeline": [
                {
                    "name": "https://m.melbet.com/api/live-feed",
                    "initiatorType": "fetch",
                    "nextHopProtocol": "h2",
                    "transferSize": 5120,
                    "durationMs": 95,
                    "startTimeMs": 20,
                    "responseEndMs": 115
                },
                {
                    "name": "https://m.melbet.com/assets/app.js",
                    "initiatorType": "script",
                    "nextHopProtocol": "h2",
                    "transferSize": 8192,
                    "durationMs": 31,
                    "startTimeMs": 5,
                    "responseEndMs": 36
                }
            ],
            "transportHints": [
                {
                    "kind": "data_endpoint",
                    "value": "https://m.melbet.com/api/live-feed",
                    "source": "resource"
                },
                {
                    "kind": "http2_transport",
                    "value": "h2",
                    "source": "resource"
                }
            ],
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 25,
                "resourceCount": 2,
                "fetchLikeCount": 1,
                "websocketHintCount": 0,
                "domContentLoadedMs": 501,
                "loadEventMs": 780,
                "lastResourceEndMs": 115,
                "hasVisibleAppShell": true
            }
        }));

        assert_eq!(snapshot.transport_hint_count(), 2);
        assert_eq!(snapshot.resource_timeline_count(), 2);
        assert_eq!(snapshot.readiness.ready_state, "complete");
        assert_eq!(snapshot.readiness.fetch_like_count, 1);
        assert_eq!(snapshot.readiness.script_count, 0);
        assert!(snapshot.runtime_context.has_http_api);
        assert_eq!(snapshot.runtime_context.partner_id, 532);
        assert!(MelbetParser::summarize_runtime_context(&snapshot)
            .contains("methods=getTopEvents|getTopLiveEvents|getTopLiveSports"));
        assert!(MelbetParser::summarize_runtime_context(&snapshot).contains("inline_scripts=2"));
        assert!(MelbetParser::summarize_runtime_context(&snapshot)
            .contains("bootstrap_markers=inline:$globalSettings|inline:partnerId"));
        assert!(MelbetParser::summarize_transport_hints(&snapshot)
            .contains("feed_endpoint:https:m.melbet.com:https://m.melbet.com/api/live-feed"));
        assert!(MelbetParser::summarize_resource_timeline(&snapshot)
            .contains("fetch:h2:95:https://m.melbet.com/api/live-feed"));
        let mapping = MelbetParser::summarize_transport_mapping(&snapshot);
        assert_eq!(
            mapping.families,
            vec!["feed_endpoint".to_string(), "http_transport".to_string()]
        );
        assert_eq!(mapping.hosts, vec!["m.melbet.com".to_string()]);
        assert_eq!(
            mapping.protocols,
            vec!["https".to_string(), "resource".to_string()]
        );
    }

    #[test]
    fn wraps_http_api_calls_with_timeout_guard() {
        let wrapped = MelbetParser::wrap_http_api_call_with_timeout(
            "window.$httpApi.getTopLiveSports()",
            "getTopLiveSports",
        );

        assert!(wrapped.contains("Promise.race"));
        assert!(wrapped.contains("getTopLiveSports"));
        assert!(wrapped.contains("5000"));
        assert!(wrapped.contains("window.$httpApi.getTopLiveSports()"));
    }

    #[test]
    fn parses_runtime_state_from_snapshot_and_marks_desktop_shell_bootstrap_only() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/ru/sport",
            "origin": "https://melbet.ru",
            "path": "/ru/sport",
            "runtimeState": {
                "href": "https://melbet.ru/ru/sport",
                "pathname": "/ru/sport",
                "search": "?mode=live",
                "hash": "#top",
                "title": "Melbet sportsbook",
                "readyState": "complete",
                "historyLength": 4,
                "bodyChildCount": 6,
                "bodyTextLength": 64,
                "customElementCount": 4,
                "buttonCount": 7,
                "linkCount": 14,
                "routeLinkCount": 3,
                "routerShellCount": 5,
                "firstButtonText": "Live",
                "bodyTextSample": "Sport Live Prematch",
                "navigationEntry": {
                    "type": "navigate",
                    "domContentLoadedMs": 420,
                    "loadMs": 710
                }
            }
        }));

        assert_eq!(snapshot.runtime_state.pathname, "/ru/sport");
        assert_eq!(snapshot.runtime_state.search, "?mode=live");
        assert_eq!(snapshot.runtime_state.hash, "#top");
        assert_eq!(snapshot.runtime_state.history_length, 4);
        assert_eq!(snapshot.runtime_state.router_shell_count, 5);
        assert!(snapshot.runtime_state.has_sportsbook_shell_markers());
        assert!(MelbetParser::has_sportsbook_shell_markers(&snapshot));
        assert!(MelbetParser::summarize_runtime_state(&snapshot).contains("search=?mode=live"));
        assert!(MelbetParser::summarize_runtime_state(&snapshot).contains("hash=#top"));
        assert!(MelbetParser::summarize_runtime_state(&snapshot).contains("history=4"));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn desktop_live_route_uses_bootstrap_markers_for_shell_recovery() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "runtimeContext": {
                "bootstrapMarkers": [
                    "route:href:https://melbet.ru/live",
                    "route_family:href:live",
                    "route_family:history:sportsbook",
                    "shell:ww-app-dsk",
                    "shell:route_links:3"
                ]
            },
            "runtimeState": {
                "href": "https://melbet.ru/ru/sport",
                "pathname": "/ru/sport",
                "readyState": "complete",
                "bodyChildCount": 2,
                "bodyTextLength": 0,
                "routeLinkCount": 3,
                "routerShellCount": 1
            }
        }));

        assert!(MelbetParser::has_sportsbook_shell_markers(&snapshot));
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn runtime_state_bootstrap_markers_recover_desktop_live_shell() {
        let runtime_state = super::MelbetRuntimeState::from_value(&serde_json::json!({
            "href": "https://melbet.ru/live",
            "pathname": "/live",
            "readyState": "complete",
            "bodyChildCount": 0,
            "bodyTextLength": 0,
            "bootstrapMarkers": [
                "route:iframe:https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2Flive%22%7D",
                "route_family:iframe:sportsbook",
                "iframe:sportsbook",
                "shell:route_links:1"
            ]
        }));

        let snapshot =
            MelbetParser::recovered_bootstrap_snapshot_from_runtime_state(&runtime_state)
                .expect("recovered snapshot");

        assert!(snapshot.runtime_state.has_bootstrap_markers());
        assert!(snapshot.runtime_state.has_sportsbook_shell_markers());
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn interactive_runtime_nodes_keep_sparse_desktop_live_bootstrap_useful() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "url": "https://melbet.ru/live",
            "origin": "https://melbet.ru",
            "path": "/live",
            "title": "Melbet Live",
            "runtimeState": {
                "href": "https://melbet.ru/live",
                "pathname": "/live",
                "title": "Melbet Live",
                "readyState": "complete",
                "bodyChildCount": 0,
                "bodyTextLength": 0,
                "buttonCount": 0,
                "interactiveNodeCount": 6,
                "linkCount": 0,
                "routeLinkCount": 0,
                "routerShellCount": 0,
                "bodyTextSample": ""
            }
        }));

        assert!(MelbetParser::has_useful_desktop_bootstrap(&snapshot));
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn recovers_bootstrap_snapshot_from_runtime_state_when_full_capture_is_missing() {
        let runtime_state = super::MelbetRuntimeState::from_value(&serde_json::json!({
            "href": "https://melbet.ru/ru/sport",
            "pathname": "/ru/sport",
            "title": "Melbet sportsbook",
            "readyState": "complete",
            "bodyChildCount": 2,
            "bodyTextLength": 0,
            "routeLinkCount": 2,
            "routerShellCount": 2,
            "navigationEntry": {
                "domContentLoadedMs": 420,
                "loadMs": 650
            }
        }));

        let snapshot =
            MelbetParser::recovered_bootstrap_snapshot_from_runtime_state(&runtime_state)
                .expect("recovered snapshot");

        assert_eq!(snapshot.final_url, "https://melbet.ru/ru/sport");
        assert_eq!(snapshot.path, "/ru/sport");
        assert!(snapshot.runtime_context.has_bootstrap_source_markers());
        assert!(snapshot
            .runtime_context
            .bootstrap_markers
            .iter()
            .any(|item| item == "route_family:runtime:sportsbook"));
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn recovers_sparse_runtime_state_with_interactive_nodes() {
        let runtime_state = super::MelbetRuntimeState::from_value(&serde_json::json!({
            "href": "https://melbet.ru/live",
            "pathname": "/live",
            "title": "Melbet Live",
            "readyState": "complete",
            "bodyChildCount": 0,
            "bodyTextLength": 0,
            "interactiveNodeCount": 4,
            "linkCount": 0,
            "routeLinkCount": 0,
            "routerShellCount": 0
        }));

        let snapshot =
            MelbetParser::recovered_bootstrap_snapshot_from_runtime_state(&runtime_state)
                .expect("recovered snapshot");

        assert!(snapshot.readiness.has_visible_app_shell);
        assert!(!MelbetParser::has_empty_runtime_bootstrap(&snapshot));
        assert_eq!(
            MelbetParser::classify_route_status(&HEADLESS_PROBES[0], &snapshot, 0),
            MelbetRouteStatus::BootstrapOnly
        );
    }

    #[test]
    fn runtime_context_bootstrap_markers_keep_runtime_diagnostics() {
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "runtimeContext": {
                "hasHttpApi": true,
                "httpApiMethods": ["getTopLiveSports", "getTopLiveEvents"],
                "partnerId": 532,
                "langId": 1,
                "countryCode": "RU",
                "hasGlobalSettings": true,
                "hasPartnerConfig": true,
                "inlineScriptCount": 2,
                "bootstrapMarkers": [
                    "inline:$globalSettings",
                    "runtime:$globalSettings",
                    "runtime:$httpApi",
                    "runtime:partnerId",
                    "runtime:langId",
                    "runtime:countryCode"
                ]
            }
        }));

        assert!(snapshot.runtime_context.has_bootstrap_source_markers());
        assert!(MelbetParser::summarize_runtime_context(&snapshot)
            .contains("bootstrap_markers=inline:$globalSettings|runtime:$globalSettings|runtime:$httpApi|runtime:partnerId|runtime:langId|runtime:countryCode"));
    }

    #[test]
    fn recovered_runtime_state_preserves_bootstrap_source_blocker_context() {
        let runtime_state = super::MelbetRuntimeState::from_value(&serde_json::json!({
            "href": "https://melbet.ru/live",
            "pathname": "/live",
            "title": "Melbet Live",
            "readyState": "complete",
            "bodyChildCount": 1,
            "bodyTextLength": 0,
            "bootstrapMarkers": [
                "route:href:https://melbet.ru/live",
                "route_family:href:live",
                "runtime:$globalSettings",
                "runtime:$httpApi",
                "runtime:partnerId",
                "runtime:langId",
                "runtime:countryCode"
            ]
        }));

        let snapshot =
            MelbetParser::recovered_bootstrap_snapshot_from_runtime_state(&runtime_state)
                .expect("recovered snapshot");

        assert!(snapshot.runtime_context.has_bootstrap_source_markers());
        assert!(snapshot
            .runtime_context
            .bootstrap_markers
            .iter()
            .any(|item| item == "runtime:$httpApi"));
        assert_eq!(
            snapshot.runtime_context.http_api_blocker(true),
            "missing_http_api_context:partner_id|lang_id|country_code|getTopLiveSports|getTopLiveEvents:additional_bootstrap_source_required"
        );
    }

    #[test]
    fn annotates_event_with_transport_and_readiness_diagnostics() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/melbet_headless_payload.json"
        ))
        .expect("fixture json");
        let item = fixture
            .as_array()
            .and_then(|items| items.first())
            .expect("first fixture item");

        let (mut event, _) =
            MelbetParser::parse_headless_item(item, true, "https://melbet.ru/live")
                .expect("headless item");
        let snapshot = MelbetBootstrapSnapshot::from_value(&serde_json::json!({
            "transportHints": [
                {
                    "kind": "data_endpoint",
                    "value": "https://melbet.ru/api/live-feed",
                    "source": "resource"
                }
            ],
            "resourceTimeline": [
                {
                    "name": "https://melbet.ru/api/live-feed",
                    "initiatorType": "fetch",
                    "nextHopProtocol": "h2",
                    "transferSize": 2048,
                    "durationMs": 54,
                    "startTimeMs": 17,
                    "responseEndMs": 71
                }
            ],
            "readinessDiagnostics": {
                "readyState": "complete",
                "bodyTextLength": 42,
                "resourceCount": 1,
                "fetchLikeCount": 1,
                "websocketHintCount": 0,
                "domContentLoadedMs": 333,
                "loadEventMs": 555,
                "lastResourceEndMs": 71,
                "hasVisibleAppShell": true
            }
        }));

        MelbetParser::annotate_event_diagnostics(
            &mut event,
            &HEADLESS_PROBES[0],
            &MelbetRouteStatus::Ready,
            &snapshot,
            3,
            "payload_ready",
        );

        assert_eq!(
            event
                .extra
                .get("melbet_transport_hints")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            event
                .extra
                .get("melbet_resource_timeline")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            event
                .extra
                .get("melbet_readiness_diagnostics")
                .and_then(|value| value.get("readyState"))
                .and_then(|value| value.as_str()),
            Some("complete")
        );
        assert_eq!(
            event
                .extra
                .get("melbet_transport_hints")
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|value| value.get("family"))
                .and_then(|value| value.as_str()),
            Some("feed_endpoint")
        );
        assert_eq!(
            event
                .extra
                .get("melbet_runtime_context")
                .and_then(|value| value.get("hasHttpApi"))
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            event
                .extra
                .get("melbet_runtime_context")
                .and_then(|value| value.get("inlineScriptCount"))
                .and_then(|value| value.as_u64()),
            Some(0)
        );
        assert_eq!(
            event
                .extra
                .get("melbet_runtime_state")
                .and_then(|value| value.get("readyState"))
                .and_then(|value| value.as_str()),
            Some("")
        );
        assert_eq!(
            event
                .extra
                .get("melbet_readiness_output")
                .and_then(|value| value.get("state"))
                .and_then(|value| value.as_str()),
            Some("dom_payload_ready")
        );
        assert_eq!(
            event
                .extra
                .get("melbet_transport_mapping")
                .and_then(|value| value.get("feedLikeCount"))
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            event
                .extra
                .get("melbet_bootstrap_acquisition_plan")
                .and_then(|value| value.get("nextStep"))
                .and_then(|value| value.as_str()),
            Some("no_manual_bootstrap_acquisition_required")
        );
    }

    #[test]
    fn exposes_transport_groundwork_readiness_diagnostics() {
        let readiness = MelbetParser::readiness_snapshot();

        assert_eq!(
            readiness.stage,
            shared::ParserReadinessStage::DiagnosticOnly
        );
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "melbet_resource_timeline_capture_available"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "melbet_transport_mapping_classification_available"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "melbet_transport_runtime_guardrail"));
    }

    #[test]
    fn returns_wall_clock_blocker_before_outer_timeout() {
        let error = MelbetParser::run_blocking_with_wall_clock_timeout(25, |_deadline| {
            std::thread::sleep(Duration::from_millis(80));
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((
                Vec::<shared::Event>::new(),
                Vec::<shared::Odd>::new(),
            ))
        })
        .expect_err("wall clock timeout expected");

        let message = error.to_string();
        assert!(message.contains("melbet runtime wall clock timeout after 25ms"));
        assert!(message.contains("useful blocker/result"));
        assert!(MELBET_RUNTIME_WALL_CLOCK_TIMEOUT_MS < 90_000);
    }
}

#[async_trait]
impl BookmakerParser for MelbetParser {
    fn name(&self) -> &str {
        "Melbet"
    }

    fn slug(&self) -> &str {
        BOOKMAKER_SLUG
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let (events, _) = self.fetch_runtime_data().await?;
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let (events, odds) = self.fetch_runtime_data().await?;
        let elapsed = started.elapsed().as_millis() as u64;
        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "Melbet: fetch finished"
        );
        Ok(ParserResult::new(BOOKMAKER_SLUG, events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        DESKTOP_USER_AGENT
    }

    fn readiness(&self) -> Option<ParserReadiness> {
        Some(Self::readiness_snapshot())
    }
}
