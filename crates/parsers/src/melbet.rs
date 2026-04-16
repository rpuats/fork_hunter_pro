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
use std::sync::Arc;
use tracing::{debug, info, warn};

const BOOKMAKER_SLUG: &str = "melbet";
const BASE_URL: &str = "https://melbet.ru";
const SPORTSBOOK_BASE_URL: &str = "https://sport.melbet.ru/";
const SPORTSBOOK_HOME_URL: &str =
    "https://sport.melbet.ru/partner/SportsBook/Home?initialRoute=%7B%22path%22%3A%22%2F%22%7D";
const HEADLESS_WAIT_MS: u64 = 3_500;
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
        }
    }

    fn as_summary(&self) -> String {
        format!(
            "http_api={},methods={},partner_id={},lang_id={},country={},globals={},partner_cfg={}",
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
        })
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
    transport_mapping: MelbetTransportMappingSummary,
}

impl MelbetReadinessOutput {
    fn as_summary(&self) -> String {
        format!(
            "state={},reason={},route_status={},bootstrap_score={},{}",
            self.state,
            self.reason,
            self.route_status,
            self.bootstrap_score,
            self.transport_mapping.as_summary(),
        )
    }

    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "state": self.state,
            "reason": self.reason,
            "routeStatus": self.route_status,
            "bootstrapScore": self.bootstrap_score,
            "transportMapping": self.transport_mapping.as_json(),
        })
    }
}

#[derive(Debug, Clone)]
struct MelbetBootstrapSnapshot {
    final_url: String,
    origin: String,
    path: String,
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
                value.get("runtimeContext").unwrap_or(&serde_json::Value::Null),
            ),
            readiness: MelbetReadinessDiagnostics::from_value(
                value
                    .get("readinessDiagnostics")
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
                    message: "Real transport runtime work is intentionally not implemented in this step; websocket interception remains disabled until protocol behavior is verified.".to_string(),
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

        final_url.contains("/ru/sport")
            || path == "/ru/sport"
            || path == "/ru/sport/"
            || title.contains("melbet.ru")
            || body.contains("sport")
            || body.contains("melbet")
            || scripts.iter().any(|script| {
                script.contains("main.js")
                    || script.contains("bundle.js")
                    || script.contains("bootstrapper")
                    || script.contains("sport")
            })
    }

    fn select_sportsbook_route(
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
    ) -> Option<String> {
        if probe.surface != MelbetSurface::Desktop {
            return None;
        }

        if let Some(route) = Self::select_embedded_route(probe, snapshot) {
            return Some(route);
        }

        if Self::has_sportsbook_shell_markers(snapshot) {
            return Some(SPORTSBOOK_HOME_URL.to_string());
        }

        None
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
    ) -> MelbetSportsbookHttpApiAttempt {
        let Some(direct_route) = Self::select_sportsbook_route(probe, snapshot) else {
            return MelbetSportsbookHttpApiAttempt {
                blocker: "no_sportsbook_route".to_string(),
                ..MelbetSportsbookHttpApiAttempt::default()
            };
        };
        let referer = if snapshot.final_url.is_empty() {
            probe.url
        } else {
            snapshot.final_url.as_str()
        };
        let tab = match helper
            .navigate_with_profile_and_referer(
                &direct_route,
                HEADLESS_WAIT_MS,
                probe.profile,
                Some(referer),
            )
        {
            Ok(tab) => tab,
            Err(_) => {
                return MelbetSportsbookHttpApiAttempt {
                    route: direct_route,
                    blocker: "sportsbook_navigation_failed".to_string(),
                    ..MelbetSportsbookHttpApiAttempt::default()
                };
            }
        };
        let mut bootstrap = Self::extract_bootstrap_snapshot(&tab);
        if bootstrap.final_url.is_empty() {
            bootstrap.final_url = direct_route.clone();
        }
        let context = match Self::extract_sport_api_context(&tab) {
            Some(context) => context,
            None => {
                return MelbetSportsbookHttpApiAttempt {
                    bootstrap: Some(bootstrap.clone()),
                    route: direct_route,
                    blocker: if bootstrap.runtime_context.has_http_api {
                        "missing_http_api_context"
                    } else {
                        "no_http_api_runtime"
                    }
                    .to_string(),
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

        MelbetSportsbookHttpApiAttempt {
            bootstrap: Some(bootstrap),
            route: direct_route,
            seed_count,
            blocker: if payload.is_empty() {
                "no_http_api_event_payload".to_string()
            } else {
                "payload_ready".to_string()
            },
            payload,
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

        if probe.surface != MelbetSurface::Desktop
            && Self::route_matches_probe(probe, snapshot)
            && Self::has_bootstrap_markers(probe, snapshot)
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
        );
        format!(
            "route_hint={},route_family={},status={},source={},blocker={},state={},reason={},payload_len={},final_url={}",
            result.probe.route_hint,
            result.probe.route_family,
            result.status.as_str(),
            if result.extraction.source.is_empty() {
                "none"
            } else {
                result.extraction.source.as_str()
            },
            if result.extraction.blocker.is_empty() {
                "unknown"
            } else {
                result.extraction.blocker.as_str()
            },
            readiness.state,
            readiness.reason,
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
            "melbet_readiness_output".to_string(),
            Self::build_readiness_output(probe, snapshot, status, payload_len).as_json(),
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
        HeadlessChromeHelper::capture_session_bootstrap(tab)
            .as_ref()
            .map(MelbetBootstrapSnapshot::from_value)
            .unwrap_or_else(|| MelbetBootstrapSnapshot {
                final_url: String::new(),
                origin: String::new(),
                path: String::new(),
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
            })
    }

    fn extract_embedded_payload(
        helper: &HeadlessChromeHelper,
        probe: &HeadlessProbe,
        snapshot: &MelbetBootstrapSnapshot,
    ) -> Option<(Vec<serde_json::Value>, MelbetBootstrapSnapshot)> {
        let embedded_route = Self::select_embedded_route(probe, snapshot)?;
        let referer = if snapshot.final_url.is_empty() {
            probe.url
        } else {
            snapshot.final_url.as_str()
        };
        let tab = helper
            .navigate_with_profile_and_referer(
                &embedded_route,
                HEADLESS_WAIT_MS,
                probe.profile,
                Some(referer),
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

    fn fetch_runtime_data_blocking(
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let helper = HeadlessChromeHelper::new()?;
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen_events = HashSet::new();
        let mut seen_odds = HashSet::new();
        let mut route_matrix = Vec::new();

        for probe in Self::runtime_probe_plan() {
            let tab = match helper.navigate_with_profile_and_wait(
                probe.url,
                HEADLESS_WAIT_MS,
                probe.profile,
            ) {
                Ok(tab) => tab,
                Err(error) => {
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
            if payload.is_empty() {
                extraction.embedded_route = Self::select_embedded_route(probe, &bootstrap)
                    .unwrap_or_default();
                if let Some((embedded_payload, embedded_bootstrap)) =
                    Self::extract_embedded_payload(&helper, probe, &bootstrap)
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
            if payload.is_empty() {
                let http_api_attempt =
                    Self::extract_sportsbook_http_api_payload(&helper, probe, &bootstrap);
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
                        readiness = bootstrap.readiness.as_summary(),
                        readiness_output = Self::build_readiness_output(
                            probe,
                            &bootstrap,
                            &route_status,
                            payload_len,
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
        let (events, odds) = tokio::task::spawn_blocking(Self::fetch_runtime_data_blocking)
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
        MelbetRouteStatus, HEADLESS_PROBES, SPORTSBOOK_BASE_URL, SPORTSBOOK_HOME_URL,
    };
    use shared::Sport;

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
            Some(SPORTSBOOK_HOME_URL.to_string())
        );
        assert_eq!(
            MelbetParser::select_sportsbook_route(&HEADLESS_PROBES[2], &snapshot),
            None
        );
    }

    #[test]
    fn detects_block_page_from_snapshot() {
        let snapshot = MelbetBootstrapSnapshot {
            final_url: "https://melbet.ru/blocked".to_string(),
            origin: "https://melbet.ru".to_string(),
            path: "/blocked".to_string(),
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
                    blocker: "no_http_api_runtime".to_string(),
                    ..super::MelbetExtractionDiagnostics::default()
                },
            },
        ];

        assert_eq!(
            MelbetParser::summarize_route_matrix(&matrix),
            "desktop-live:canonical-live:ready:7:0:dom:payload_ready:dom_payload_ready:rendered_dom_payload_detected:/live,webview-live:webview-shell-live:bootstrap_only:0:0:none:no_http_api_runtime:shell_bootstrapped:bootstrap_signals_without_dom_payload:/live"
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
                blocker: "no_http_api_runtime".to_string(),
                ..super::MelbetExtractionDiagnostics::default()
            },
        });

        assert_eq!(
            blocker,
            "route_hint=desktop-live,route_family=canonical-live,status=blocked,source=none,blocker=no_http_api_runtime,state=blocked_or_unconfirmed,reason=insufficient_bootstrap_signals,payload_len=0,final_url=https://melbet.ru/live"
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
                "hasPartnerConfig": true
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
