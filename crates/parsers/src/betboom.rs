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

const BOOKMAKER_SLUG: &str = "betboom";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const BASE_URL: &str = "https://betboom.ru";
const HEADLESS_WAIT_MS: u64 = 5_000;
const FILTER_WAIT_MS: u64 = 2_500;
const SCROLL_STEPS: usize = 2;
const PREMATCH_FILTER_TEXT: &str = "1н";
const MIN_SNAPSHOT_TEXT_LEN: usize = 80;
const SNAPSHOT_PREVIEW_CHARS: usize = 96;

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod sporthub_helper {
    use super::BASE_URL;

    pub(crate) const BOOTSTRAP_DISCOVERY_URL: &str = "https://betboom.ru/sport/football";
    pub(crate) const WS_URL_HINT: &str = "wss://sporthub.betboom.ru/ws";
    pub(crate) const PROTO_ASSET_HINT: &str = "sporthub-feed.proto";
    pub(crate) const TRANSPORT_PROTOBUF: &str = "protobuf";
    pub(crate) const CHANNEL_PREMATCH: &str = "prematch_snapshot";
    pub(crate) const CHANNEL_LIVE: &str = "live_update";
    pub(crate) const DEFAULT_CHANNELS: &[&str] = &[CHANNEL_PREMATCH, CHANNEL_LIVE];
    pub(crate) const NOTES: &[&str] = &[
        "Bootstrap extraction records ws/proto/channel hints without opening a socket.",
        "Relative proto assets are normalized against the BetBoom origin for fixture coverage.",
        "Future feed work should decode Sporthub protobuf frames behind an explicit runtime gate only.",
    ];
    const MARKERS: &[&str] = &[
        "sporthub",
        "protobuf",
        "proto",
        "event_feed",
        "prematch_snapshot",
        "live_update",
    ];

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct BootstrapHints {
        pub(crate) ws_urls: Vec<String>,
        pub(crate) script_assets: Vec<String>,
        pub(crate) protobuf_markers: Vec<String>,
        pub(crate) has_sporthub_namespace: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct BootstrapConfig {
        pub(crate) discovery_url: &'static str,
        pub(crate) ws_url: String,
        pub(crate) transport: String,
        pub(crate) proto_asset: Option<String>,
        pub(crate) channels: Vec<String>,
        pub(crate) runtime_feature_enabled: bool,
        pub(crate) notes: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FeedBootstrapPlan {
        pub(crate) config: BootstrapConfig,
        pub(crate) bootstrap_detected: bool,
        pub(crate) protobuf_assets_detected: bool,
        pub(crate) frame_decoder_scaffolded: bool,
        pub(crate) runtime_guarded: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FrameEnvelope {
        pub(crate) length_delimited: bool,
        pub(crate) prefix_len: usize,
        pub(crate) message_len: usize,
        pub(crate) payload_hex_preview: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ContractManifest {
        pub(crate) discovery_url: &'static str,
        pub(crate) namespace: &'static str,
        pub(crate) transport: String,
        pub(crate) ws_url: String,
        pub(crate) proto_asset: Option<String>,
        pub(crate) script_assets: Vec<String>,
        pub(crate) channels: Vec<String>,
        pub(crate) protobuf_markers: Vec<String>,
        pub(crate) runtime_guarded: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct SubscriptionIntent {
        pub(crate) channel: String,
        pub(crate) purpose: String,
        pub(crate) subscribe_mode: String,
        pub(crate) runtime_guarded: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum FrameClass {
        Empty,
        JsonControl,
        TextHeartbeat,
        LengthDelimitedProtobuf,
        BinaryOpaque,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FrameClassification {
        pub(crate) class: FrameClass,
        pub(crate) envelope: FrameEnvelope,
        pub(crate) ascii_preview: Option<String>,
        pub(crate) inferred_channel: Option<String>,
        pub(crate) runtime_guarded: bool,
    }

    pub(crate) fn runtime_enabled() -> bool {
        false
    }

    pub(crate) fn build_static_plan() -> FeedBootstrapPlan {
        FeedBootstrapPlan {
            config: BootstrapConfig {
                discovery_url: BOOTSTRAP_DISCOVERY_URL,
                ws_url: WS_URL_HINT.to_string(),
                transport: TRANSPORT_PROTOBUF.to_string(),
                proto_asset: Some(format!("{BASE_URL}/assets/{PROTO_ASSET_HINT}")),
                channels: DEFAULT_CHANNELS
                    .iter()
                    .map(|channel| (*channel).to_string())
                    .collect(),
                runtime_feature_enabled: runtime_enabled(),
                notes: NOTES.iter().map(|note| (*note).to_string()).collect(),
            },
            bootstrap_detected: false,
            protobuf_assets_detected: false,
            frame_decoder_scaffolded: true,
            runtime_guarded: true,
        }
    }

    pub(crate) fn build_contract_manifest_from_html(html: &str) -> ContractManifest {
        let hints = extract_bootstrap_hints_from_html(html);
        let plan = build_plan_from_html(html);

        ContractManifest {
            discovery_url: BOOTSTRAP_DISCOVERY_URL,
            namespace: "sporthub",
            transport: plan.config.transport,
            ws_url: plan.config.ws_url,
            proto_asset: plan.config.proto_asset,
            script_assets: hints.script_assets,
            channels: plan.config.channels,
            protobuf_markers: hints.protobuf_markers,
            runtime_guarded: plan.runtime_guarded,
        }
    }

    pub(crate) fn build_subscription_intents(
        manifest: &ContractManifest,
    ) -> Vec<SubscriptionIntent> {
        manifest
            .channels
            .iter()
            .map(|channel| SubscriptionIntent {
                channel: channel.clone(),
                purpose: match channel.as_str() {
                    CHANNEL_PREMATCH => "prematch market snapshot discovery".to_string(),
                    CHANNEL_LIVE => "live market delta discovery".to_string(),
                    _ => "undocumented sporthub feed lane".to_string(),
                },
                subscribe_mode: format!("intent-only:{}", manifest.transport),
                runtime_guarded: manifest.runtime_guarded,
            })
            .collect()
    }

    pub(crate) fn build_plan_from_html(html: &str) -> FeedBootstrapPlan {
        let hints = extract_bootstrap_hints_from_html(html);
        let mut plan = build_static_plan();

        if let Some(ws_url) = capture_bootstrap_string(html, "wsUrl") {
            plan.config.ws_url = ws_url;
        } else if let Some(ws_url) = hints.ws_urls.first() {
            plan.config.ws_url = ws_url.clone();
        }

        if let Some(transport) = capture_bootstrap_string(html, "transport") {
            plan.config.transport = transport;
        }

        if let Some(proto_asset) = capture_bootstrap_string(html, "feedProto") {
            plan.config.proto_asset = Some(normalize_bootstrap_asset(&proto_asset));
        } else if let Some(asset) = hints
            .script_assets
            .iter()
            .find(|asset| asset.contains(PROTO_ASSET_HINT))
        {
            plan.config.proto_asset = Some(normalize_bootstrap_asset(asset));
        }

        let channels = capture_bootstrap_channels(html);
        if !channels.is_empty() {
            plan.config.channels = channels;
        }

        plan.bootstrap_detected = hints.has_sporthub_namespace;
        plan.protobuf_assets_detected = plan
            .config
            .proto_asset
            .as_deref()
            .is_some_and(|asset| asset.contains("proto") || asset.contains("pb"));

        plan
    }

    pub(crate) fn extract_bootstrap_hints_from_html(html: &str) -> BootstrapHints {
        let mut ws_urls = Vec::new();
        let mut script_assets = Vec::new();
        let mut protobuf_markers = Vec::new();
        let lower = html.to_lowercase();

        let ws_regex = regex::Regex::new(r#"wss?://[^"'\s<]+"#).expect("ws regex");
        for capture in ws_regex.find_iter(html) {
            let value = capture.as_str().trim().trim_end_matches([',', ';']);
            if value.to_lowercase().contains("sporthub")
                && !ws_urls.iter().any(|item| item == value)
            {
                ws_urls.push(value.to_string());
            }
        }

        let asset_regex =
            regex::Regex::new(r#"[^"'\s>]*(?:sporthub|protobuf)[^"'\s>]*\.(?:js|mjs|proto|pb)"#)
                .expect("asset regex");
        for capture in asset_regex.find_iter(html) {
            let value = capture.as_str().trim().trim_end_matches([',', ';']);
            if !script_assets.iter().any(|item| item == value) {
                script_assets.push(value.to_string());
            }
        }

        for marker in MARKERS {
            if lower.contains(marker) {
                protobuf_markers.push((*marker).to_string());
            }
        }

        BootstrapHints {
            ws_urls,
            script_assets,
            protobuf_markers,
            has_sporthub_namespace: lower.contains("sporthub"),
        }
    }

    pub(crate) fn inspect_ws_frame(frame: &[u8]) -> FrameEnvelope {
        let (length_delimited, prefix_len, message_len, payload) =
            if let Some((prefix_len, message_len)) = decode_varint_prefix(frame) {
                let available = frame.len().saturating_sub(prefix_len);
                if message_len <= available {
                    (
                        true,
                        prefix_len,
                        message_len,
                        &frame[prefix_len..prefix_len + message_len],
                    )
                } else {
                    (false, 0, frame.len(), frame)
                }
            } else {
                (false, 0, frame.len(), frame)
            };

        let payload_hex_preview = payload
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        FrameEnvelope {
            length_delimited,
            prefix_len,
            message_len,
            payload_hex_preview,
        }
    }

    pub(crate) fn classify_ws_frame(frame: &[u8]) -> FrameClassification {
        let envelope = inspect_ws_frame(frame);
        let payload = if envelope.length_delimited {
            &frame[envelope.prefix_len..envelope.prefix_len + envelope.message_len]
        } else {
            frame
        };
        let ascii_preview = std::str::from_utf8(payload)
            .ok()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(48).collect::<String>());
        let lowered_preview = ascii_preview.as_deref().map(str::to_lowercase);
        let inferred_channel = lowered_preview.as_deref().and_then(|preview| {
            if preview.contains(CHANNEL_PREMATCH) {
                Some(CHANNEL_PREMATCH.to_string())
            } else if preview.contains(CHANNEL_LIVE) {
                Some(CHANNEL_LIVE.to_string())
            } else {
                None
            }
        });

        let class = if frame.is_empty() {
            FrameClass::Empty
        } else if lowered_preview
            .as_deref()
            .is_some_and(|preview| preview.starts_with('{') || preview.starts_with('['))
        {
            FrameClass::JsonControl
        } else if lowered_preview.as_deref().is_some_and(|preview| {
            preview.contains("ping") || preview.contains("pong") || preview.contains("heartbeat")
        }) {
            FrameClass::TextHeartbeat
        } else if envelope.length_delimited {
            FrameClass::LengthDelimitedProtobuf
        } else {
            FrameClass::BinaryOpaque
        };

        FrameClassification {
            class,
            envelope,
            ascii_preview,
            inferred_channel,
            runtime_guarded: !runtime_enabled(),
        }
    }

    fn capture_bootstrap_string(html: &str, field: &str) -> Option<String> {
        let pattern = format!(r#"{field}\s*:\s*[\"']([^\"']+)[\"']"#);
        regex::Regex::new(&pattern)
            .ok()?
            .captures(html)
            .and_then(|captures| {
                captures
                    .get(1)
                    .map(|value| value.as_str().trim().to_string())
            })
    }

    fn capture_bootstrap_channels(html: &str) -> Vec<String> {
        let Some(captures) = regex::Regex::new(r#"channels\s*:\s*\[([^\]]+)\]"#)
            .expect("channels regex")
            .captures(html)
        else {
            return Vec::new();
        };

        captures[1]
            .split(',')
            .filter_map(|value| {
                let trimmed = value.trim().trim_matches('"').trim_matches('\'');
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .collect()
    }

    fn normalize_bootstrap_asset(asset: &str) -> String {
        if asset.starts_with("http://") || asset.starts_with("https://") {
            asset.to_string()
        } else if asset.starts_with('/') {
            format!("{BASE_URL}{asset}")
        } else {
            format!("{BASE_URL}/{asset}")
        }
    }

    fn decode_varint_prefix(bytes: &[u8]) -> Option<(usize, usize)> {
        let mut value = 0usize;
        let mut shift = 0usize;

        for (index, byte) in bytes.iter().copied().enumerate().take(10) {
            let chunk = usize::from(byte & 0x7f);
            value |= chunk.checked_shl(shift as u32)?;
            if byte & 0x80 == 0 {
                return Some((index + 1, value));
            }
            shift += 7;
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct BetboomParser {
    client: Arc<Client>,
}

#[derive(Clone, Copy, Debug)]
struct Probe {
    url: &'static str,
    sport: Sport,
    is_live: bool,
    prematch_filter: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProbeReport {
    url: &'static str,
    sport: Sport,
    is_live: bool,
    navigation_ok: bool,
    navigation_error: Option<String>,
    snapshots: usize,
    rendered_chars: usize,
    strategies: Vec<String>,
    events: usize,
    odds: usize,
    preview: Option<String>,
    rendered_probe: Option<String>,
    root_cause: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeDiagnosticSummary {
    status: &'static str,
    total_probes: usize,
    navigation_ok: usize,
    navigation_failed: usize,
    snapshot_nonzero: usize,
    rendered_char_nonzero: usize,
    rendered_probe_nonzero: usize,
    root_cause_nonzero: usize,
    root_cause_counts: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedSnapshot {
    strategy: String,
    text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RenderedTextAnalysis {
    line_count: usize,
    team_candidates: usize,
    market_labels: usize,
    price_lines: usize,
    market_price_pairs: usize,
    block_count: usize,
    explicit_boundaries: usize,
    implicit_boundaries: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TeamMisclassificationSummary {
    league_like: usize,
    status_like: usize,
    market_like: usize,
    counter_like: usize,
}

const PROBES: &[Probe] = &[
    Probe {
        url: "https://betboom.ru/sport/live/football",
        sport: Sport::Football,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/football",
        sport: Sport::Football,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/basketball",
        sport: Sport::Basketball,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/basketball",
        sport: Sport::Basketball,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/hockey",
        sport: Sport::Hockey,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/hockey",
        sport: Sport::Hockey,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/tennis",
        sport: Sport::Tennis,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/tennis",
        sport: Sport::Tennis,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/volleyball",
        sport: Sport::Volleyball,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/volleyball",
        sport: Sport::Volleyball,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/table-tennis",
        sport: Sport::TableTennis,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/table-tennis",
        sport: Sport::TableTennis,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/handball",
        sport: Sport::Handball,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/handball",
        sport: Sport::Handball,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
    Probe {
        url: "https://betboom.ru/sport/live/futsal",
        sport: Sport::Futsal,
        is_live: true,
        prematch_filter: None,
    },
    Probe {
        url: "https://betboom.ru/sport/futsal",
        sport: Sport::Futsal,
        is_live: false,
        prematch_filter: Some(PREMATCH_FILTER_TEXT),
    },
];

impl BetboomParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    fn readiness_snapshot() -> ParserReadiness {
        let scaffold = sporthub_helper::build_static_plan();
        ParserReadiness {
            stage: ParserReadinessStage::DiagnosticOnly,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "rendered_dom_diagnostics_available".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Rendered sport-page extraction remains available for diagnostics and regression fixtures.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "sporthub_bootstrap_constants_recorded".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!(
                        "Bootstrap hints for Sporthub discovery are recorded from {} with ws hint {} and channels {}.",
                        scaffold.config.discovery_url,
                        scaffold.config.ws_url,
                        scaffold.config.channels.join(", ")
                    ),
                },
                ParserDiagnosticCheck {
                    code: "protobuf_frame_scaffolding_present".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: format!(
                        "Lightweight bootstrap/frame inspection helpers are present for future {} work without activating a feed runtime.",
                        sporthub_helper::PROTO_ASSET_HINT
                    ),
                },
                ParserDiagnosticCheck {
                    code: "sporthub_contract_helpers_available".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "Structured Sporthub contract helpers cover manifest extraction, subscription intents, and frame classification without enabling runtime feed execution.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "sporthub_bootstrap_notes_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: scaffold.config.notes.join(" "),
                },
                ParserDiagnosticCheck {
                    code: "sporthub_runtime_feature_disabled".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "The unstable Sporthub websocket runtime path is not compiled into this parser crate, so production builds stay on the safe diagnostic-only path.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "sporthub_feed_unimplemented_guardrail".to_string(),
                    severity: DiagnosticSeverity::Fail,
                    message: "Sporthub websocket/protobuf feed execution is intentionally not implemented yet; live production scanning remains disabled until schema and runtime stability are verified.".to_string(),
                },
            ],
        }
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            probes = PROBES.len(),
            "BetBoom: collecting runtime data from rendered sport pages"
        );

        let (events, odds, reports) = self.fetch_via_headless().await?;
        if events.is_empty() && odds.is_empty() {
            let diagnostic = Self::format_empty_runtime_diagnostic(&reports);
            warn!(diagnostic = %diagnostic, "BetBoom: rendered runtime extraction returned no data");
            return Err(Self::boxed_error(diagnostic));
        }
        let live_count = events.iter().filter(|event| event.is_live).count();
        let prematch_count = events.len().saturating_sub(live_count);

        info!(
            total = events.len(),
            live = live_count,
            prematch = prematch_count,
            odds = odds.len(),
            "BetBoom: rendered runtime extraction finished"
        );

        Ok((events, odds))
    }

    async fn fetch_via_headless(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>, Vec<ProbeReport>), Box<dyn std::error::Error + Send + Sync>>
    {
        let probes = PROBES.to_vec();
        tokio::task::spawn_blocking(move || Self::fetch_headless_runtime_data_blocking(&probes))
            .await
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?
    }

    fn fetch_headless_runtime_data_blocking(
        probes: &[Probe],
    ) -> Result<(Vec<Event>, Vec<Odd>, Vec<ProbeReport>), Box<dyn std::error::Error + Send + Sync>>
    {
        let helper = HeadlessChromeHelper::new()?;
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen = HashSet::new();
        let mut reports = Vec::with_capacity(probes.len());

        for probe in probes {
            let tab = match helper.navigate_and_wait(probe.url, HEADLESS_WAIT_MS) {
                Ok(tab) => tab,
                Err(error) => {
                    warn!(url = probe.url, error = %error, "BetBoom: headless navigation failed");
                    reports.push(ProbeReport {
                        url: probe.url,
                        sport: probe.sport,
                        is_live: probe.is_live,
                        navigation_ok: false,
                        navigation_error: Some(error.to_string()),
                        snapshots: 0,
                        rendered_chars: 0,
                        strategies: Vec::new(),
                        events: 0,
                        odds: 0,
                        preview: None,
                        rendered_probe: None,
                        root_cause: Some("navigation_failed".to_string()),
                    });
                    continue;
                }
            };

            if let Some(filter_text) = probe.prematch_filter {
                let clicked = Self::click_visible_text(&tab, filter_text);
                debug!(
                    url = probe.url,
                    filter = filter_text,
                    clicked,
                    "BetBoom: prematch filter attempt"
                );
                std::thread::sleep(std::time::Duration::from_millis(FILTER_WAIT_MS));
            }

            let snapshots = Self::collect_text_snapshots(&tab);
            debug!(
                url = probe.url,
                snapshots = snapshots.len(),
                "BetBoom: text snapshots collected"
            );

            let mut probe_events = 0;
            let mut probe_odds = 0;
            let mut best_empty_root_cause = None;
            let mut best_empty_root_cause_score = None;
            let mut best_rendered_probe = None;
            let mut best_rendered_score = None;
            let snapshot_count = snapshots.len();
            let rendered_chars = snapshots.iter().map(|snapshot| snapshot.text.len()).sum();
            let preview = snapshots
                .iter()
                .map(|snapshot| Self::snapshot_preview(&snapshot.text))
                .find(|preview| !preview.is_empty());
            let strategies = snapshots
                .iter()
                .map(|snapshot| snapshot.strategy.to_string())
                .collect::<Vec<_>>();

            for snapshot in &snapshots {
                if let Some(analysis) = Self::analyze_rendered_text(&snapshot.text) {
                    let score = Self::rendered_probe_score(&analysis);
                    if best_rendered_score.is_none_or(|current| score > current) {
                        best_rendered_score = Some(score);
                        best_rendered_probe = Some(format!(
                            "{}:{}",
                            snapshot.strategy,
                            Self::format_rendered_analysis(&analysis)
                        ));
                    }
                    if let Some(diagnostic) = Self::diagnose_empty_rendered_text(&snapshot.text) {
                        if best_empty_root_cause_score.is_none_or(|current| score >= current) {
                            best_empty_root_cause_score = Some(score);
                            best_empty_root_cause =
                                Some(format!("{}:{}", snapshot.strategy, diagnostic));
                        }
                    }
                }
                let (events, odds) = Self::parse_rendered_text(&snapshot.text, *probe);
                for event in events {
                    if seen.insert(event.id.clone()) {
                        all_events.push(event);
                        probe_events += 1;
                    }
                }
                probe_odds += odds.len();
                all_odds.extend(odds);
            }

            reports.push(ProbeReport {
                url: probe.url,
                sport: probe.sport,
                is_live: probe.is_live,
                navigation_ok: true,
                navigation_error: None,
                snapshots: snapshot_count,
                rendered_chars,
                strategies,
                events: probe_events,
                odds: probe_odds,
                preview,
                rendered_probe: best_rendered_probe,
                root_cause: if probe_events == 0 && probe_odds == 0 {
                    if snapshot_count == 0 {
                        Some("no_rendered_snapshots".to_string())
                    } else {
                        best_empty_root_cause
                    }
                } else {
                    None
                },
            });
        }

        Ok((all_events, all_odds, reports))
    }

    fn format_empty_runtime_diagnostic(reports: &[ProbeReport]) -> String {
        if reports.is_empty() {
            return "BetBoom rendered runtime returned no events or odds; no probes were executed"
                .to_string();
        }

        let summary = Self::summarize_runtime_diagnostics(reports);

        let probe_details = reports
            .iter()
            .map(|report| {
                format!(
                    "{}:{}: nav={}, snapshots={}, chars={}, strategies={}, events={}, odds={}, preview={}, rendered_probe={}, nav_error={}, root_cause={}",
                    if report.is_live { "live" } else { "prematch" },
                    report.sport,
                    report.navigation_ok,
                    report.snapshots,
                    report.rendered_chars,
                    if report.strategies.is_empty() {
                        "-".to_string()
                    } else {
                        report.strategies.join("|")
                    },
                    report.events,
                    report.odds,
                    report.preview.as_deref().unwrap_or("-"),
                    report.rendered_probe.as_deref().unwrap_or("-"),
                    report.navigation_error.as_deref().unwrap_or("-"),
                    report.root_cause.as_deref().unwrap_or("-")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");

        format!(
            "BetBoom rendered runtime returned no events or odds across {} probes: status={}, nav_ok={}/{}, nav_failed={}, snapshots_nonzero={}/{}, rendered_chars_nonzero={}/{}, rendered_probe_nonzero={}/{}, root_cause_nonzero={}/{}, root_cause_counts={}, probes=[{}]",
            summary.total_probes,
            summary.status,
            summary.navigation_ok,
            summary.total_probes,
            summary.navigation_failed,
            summary.snapshot_nonzero,
            summary.total_probes,
            summary.rendered_char_nonzero,
            summary.total_probes,
            summary.rendered_probe_nonzero,
            summary.total_probes,
            summary.root_cause_nonzero,
            summary.total_probes,
            Self::format_root_cause_counts(&summary.root_cause_counts),
            probe_details
        )
    }

    fn summarize_runtime_diagnostics(reports: &[ProbeReport]) -> RuntimeDiagnosticSummary {
        let total_probes = reports.len();
        let navigation_ok = reports.iter().filter(|report| report.navigation_ok).count();
        let snapshot_nonzero = reports.iter().filter(|report| report.snapshots > 0).count();
        let rendered_char_nonzero = reports
            .iter()
            .filter(|report| report.rendered_chars > 0)
            .count();
        let rendered_probe_nonzero = reports
            .iter()
            .filter(|report| report.rendered_probe.is_some())
            .count();
        let root_cause_nonzero = reports
            .iter()
            .filter(|report| report.root_cause.is_some())
            .count();
        let navigation_failed = total_probes.saturating_sub(navigation_ok);

        let mut root_cause_counts = HashMap::new();
        for report in reports {
            if let Some(root_cause) = report.root_cause.as_deref() {
                *root_cause_counts
                    .entry(Self::normalize_root_cause(root_cause).to_string())
                    .or_insert(0usize) += 1;
            }
        }

        let mut root_cause_counts = root_cause_counts.into_iter().collect::<Vec<_>>();
        root_cause_counts.sort_by(|(left_reason, left_count), (right_reason, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_reason.cmp(right_reason))
        });

        let status = if navigation_ok == 0 {
            "navigation_blocked"
        } else if snapshot_nonzero == 0 {
            "no_rendered_snapshots"
        } else if rendered_probe_nonzero > 0 {
            "rendered_visible_but_parse_empty"
        } else if rendered_char_nonzero > 0 {
            "rendered_text_unstructured"
        } else {
            "rendered_signal_missing"
        };

        RuntimeDiagnosticSummary {
            status,
            total_probes,
            navigation_ok,
            navigation_failed,
            snapshot_nonzero,
            rendered_char_nonzero,
            rendered_probe_nonzero,
            root_cause_nonzero,
            root_cause_counts,
        }
    }

    fn normalize_root_cause(root_cause: &str) -> &str {
        let root_cause = root_cause
            .split_once(':')
            .map(|(_, suffix)| suffix)
            .unwrap_or(root_cause);

        root_cause
            .split_once('[')
            .map(|(prefix, _)| prefix)
            .unwrap_or(root_cause)
    }

    fn format_root_cause_counts(counts: &[(String, usize)]) -> String {
        if counts.is_empty() {
            return "-".to_string();
        }

        counts
            .iter()
            .map(|(reason, count)| format!("{reason}:{count}"))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn boxed_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            message.into(),
        ))
    }

    fn click_visible_text(tab: &headless_chrome::Tab, text: &str) -> bool {
        let escaped = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
        let script = format!(
            r#"(() => {{
                const targetText = {escaped};
                const nodes = Array.from(document.querySelectorAll('button, a, span, div'));
                const target = nodes.find((node) => {{
                    const value = (node.innerText || node.textContent || '').replace(/\s+/g, ' ').trim();
                    const visible = !!(node.offsetWidth || node.offsetHeight || node.getClientRects().length);
                    return visible && value === targetText;
                }});
                if (!target) return false;
                target.dispatchEvent(new MouseEvent('click', {{ bubbles: true, cancelable: true }}));
                return true;
            }})()"#
        );

        HeadlessChromeHelper::evaluate_json(tab, &script)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    fn collect_text_snapshots(tab: &headless_chrome::Tab) -> Vec<RenderedSnapshot> {
        let mut snapshots = Vec::new();

        Self::extend_rendered_snapshots(tab, &mut snapshots);

        for _ in 0..SCROLL_STEPS {
            let _ = tab.evaluate(
                "window.scrollBy(0, Math.max(window.innerHeight, 900));",
                false,
            );
            std::thread::sleep(std::time::Duration::from_millis(1_200));

            Self::extend_rendered_snapshots(tab, &mut snapshots);
        }

        snapshots
    }

    fn extend_rendered_snapshots(
        tab: &headless_chrome::Tab,
        snapshots: &mut Vec<RenderedSnapshot>,
    ) {
        let Some(payload) = HeadlessChromeHelper::evaluate_json(
            tab,
            r#"(() => {
                const normalize = (value) => (value || '').replace(/\u00a0/g, ' ').replace(/\s+/g, ' ').trim();
                const visible = (node) => !!(node && (node.offsetWidth || node.offsetHeight || node.getClientRects().length));
                const results = [];
                const seen = new Set();
                const push = (strategy, text) => {
                    const normalized = normalize(text);
                    if (normalized.length < 80 || seen.has(normalized)) {
                        return;
                    }
                    seen.add(normalized);
                    results.push({ strategy, text: normalized });
                };

                const selectors = [
                    ['sportbook_root', '.bb-uc'],
                    ['market_shell', '.bb-Vq'],
                    ['sportbook_class', '[class*="Sportbook"]'],
                    ['main', 'main'],
                    ['body', 'body'],
                ];

                for (const [strategy, selector] of selectors) {
                    const node = document.querySelector(selector);
                    if (visible(node)) {
                        push(strategy, node.innerText || node.textContent || '');
                    }
                }

                const cardSelectors = [
                    '[class*="Event"]',
                    '[class*="event"]',
                    '[class*="Match"]',
                    '[class*="match"]',
                    'main article',
                    'main section',
                ];
                const cardTexts = Array.from(document.querySelectorAll(cardSelectors.join(',')))
                    .filter(visible)
                    .map((node) => normalize(node.innerText || node.textContent || ''))
                    .filter((text) => text.length >= 24 && /(?:П1|П2|\bX\b|\b1\b|\b2\b)/.test(text))
                    .slice(0, 60);
                if (cardTexts.length) {
                    push('event_cards', cardTexts.join('\nЕщё\n'));
                }

                const structuredCards = cardTexts
                    .map((text) => text
                        .replace(/\s*\+\s*\d+(?=\s|$)/g, '')
                        .replace(/\s+(П1|П2|X|1|2)\s+/g, '\n$1\n')
                        .replace(/\s+(\d+[.,]\d+)\s+/g, '\n$1\n')
                    )
                    .filter((text) => text.length >= 24);
                if (structuredCards.length) {
                    push('structured_event_cards', structuredCards.join('\nЕщё\n'));
                }

                const interactiveTexts = Array.from(document.querySelectorAll('main a, main button, main [role="button"], main span, main div'))
                    .filter(visible)
                    .map((node) => normalize(node.innerText || node.textContent || ''))
                    .filter((text) => text.length >= 8)
                    .slice(0, 250);
                if (interactiveTexts.length) {
                    push('interactive_rollup', interactiveTexts.join('\n'));
                }

                return results;
            })()"#,
        ) else {
            return;
        };

        let Some(items) = payload.as_array() else {
            return;
        };

        for item in items {
            let Some(text) = item.get("text").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(strategy) = item.get("strategy").and_then(|value| value.as_str()) else {
                continue;
            };
            Self::push_snapshot(
                snapshots,
                RenderedSnapshot {
                    strategy: strategy.to_string(),
                    text: text.to_string(),
                },
            );
        }
    }

    fn push_snapshot(snapshots: &mut Vec<RenderedSnapshot>, snapshot: RenderedSnapshot) {
        let text = snapshot
            .text
            .lines()
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if text.len() < MIN_SNAPSHOT_TEXT_LEN || snapshots.iter().any(|item| item.text == text) {
            return;
        }

        snapshots.push(RenderedSnapshot {
            strategy: snapshot.strategy,
            text,
        });
    }

    fn snapshot_preview(text: &str) -> String {
        text.chars()
            .take(SNAPSHOT_PREVIEW_CHARS)
            .collect::<String>()
    }

    fn parse_rendered_text(text: &str, probe: Probe) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut current_league: Option<String> = None;

        for block in Self::split_rendered_blocks(text) {
            if let Some((event, mut event_odds, league)) =
                Self::parse_event_block(&block, current_league.as_deref(), probe, probe.url)
            {
                current_league = Some(league);
                events.push(event);
                odds.append(&mut event_odds);
            }
        }

        (events, odds)
    }

    fn split_rendered_blocks(text: &str) -> Vec<Vec<String>> {
        let lines = text
            .lines()
            .map(|raw_line| raw_line.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        let mut blocks = Vec::new();
        let mut block = Vec::new();
        let mut index = 0;

        while index < lines.len() {
            let line = &lines[index];
            if Self::is_block_boundary(line) {
                Self::push_block(&mut blocks, &mut block);
                index += 1;
                if index < lines.len() && lines[index].starts_with('+') {
                    index += 1;
                }
                continue;
            }

            if Self::looks_like_implicit_block_boundary(&block, line, lines.get(index + 1)) {
                Self::push_block(&mut blocks, &mut block);
            }

            block.push(line.clone());
            index += 1;
        }

        Self::push_block(&mut blocks, &mut block);
        blocks
    }

    fn push_block(blocks: &mut Vec<Vec<String>>, block: &mut Vec<String>) {
        if !block.is_empty() {
            blocks.push(std::mem::take(block));
        }
    }

    fn looks_like_implicit_block_boundary(
        current_block: &[String],
        line: &str,
        next_line: Option<&String>,
    ) -> bool {
        if current_block.is_empty() || !Self::block_looks_complete(current_block) {
            return false;
        }

        Self::looks_like_league(line)
            || (Self::is_team_candidate(line)
                && next_line.is_some_and(|next| {
                    Self::is_team_candidate(next)
                        || Self::is_status_line(next)
                        || Self::looks_like_league(next)
                }))
    }

    fn block_looks_complete(block: &[String]) -> bool {
        let analysis = Self::analyze_rendered_text_lines(block);
        analysis.team_candidates >= 2 && Self::count_market_price_pairs(block) >= 2
    }

    fn analyze_rendered_text_lines(lines: &[String]) -> RenderedTextAnalysis {
        RenderedTextAnalysis {
            line_count: lines.len(),
            team_candidates: lines
                .iter()
                .filter(|line| Self::is_team_candidate(line))
                .count(),
            market_labels: lines
                .iter()
                .filter(|line| Self::is_market_label(line))
                .count(),
            price_lines: lines
                .iter()
                .filter(|line| Self::parse_price(line).is_some())
                .count(),
            market_price_pairs: Self::count_market_price_pairs(lines),
            block_count: 1,
            explicit_boundaries: lines
                .iter()
                .filter(|line| Self::is_block_boundary(line))
                .count(),
            implicit_boundaries: 0,
        }
    }

    fn analyze_rendered_text(text: &str) -> Option<RenderedTextAnalysis> {
        let lines = text
            .lines()
            .map(|raw_line| raw_line.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return None;
        }

        let mut analysis = Self::analyze_rendered_text_lines(&lines);
        analysis.block_count = Self::split_rendered_blocks(text).len().max(1);
        analysis.implicit_boundaries = analysis
            .block_count
            .saturating_sub(1 + analysis.explicit_boundaries);
        Some(analysis)
    }

    fn format_rendered_analysis(analysis: &RenderedTextAnalysis) -> String {
        format!(
            "lines={},blocks={},teams={},markets={},prices={},pairs={},boundaries={},implicit={}",
            analysis.line_count,
            analysis.block_count,
            analysis.team_candidates,
            analysis.market_labels,
            analysis.price_lines,
            analysis.market_price_pairs,
            analysis.explicit_boundaries,
            analysis.implicit_boundaries
        )
    }

    fn rendered_probe_score(analysis: &RenderedTextAnalysis) -> usize {
        analysis.block_count * 100
            + analysis.market_price_pairs * 10
            + analysis.team_candidates * 5
            + analysis.market_labels * 3
            + analysis.price_lines
    }

    fn diagnose_empty_rendered_text(text: &str) -> Option<String> {
        let lines = text
            .lines()
            .map(|raw_line| raw_line.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return Some("empty_rendered_text".to_string());
        }

        let mut analysis = Self::analyze_rendered_text_lines(&lines);
        analysis.block_count = Self::split_rendered_blocks(text).len().max(1);
        analysis.implicit_boundaries = analysis
            .block_count
            .saturating_sub(1 + analysis.explicit_boundaries);
        let reason = if analysis.market_labels == 0 {
            "missing_market_labels"
        } else if analysis.team_candidates < 2 {
            "missing_team_pairs"
        } else if analysis.market_price_pairs < 2 {
            "missing_price_lines"
        } else if analysis.explicit_boundaries == 0 && analysis.implicit_boundaries == 0 {
            "single_flattened_block"
        } else {
            "rendered_parse_miss"
        };

        let misclassification = if reason == "missing_team_pairs" {
            Self::summarize_team_misclassification(&lines)
        } else {
            TeamMisclassificationSummary::default()
        };

        Some(format!(
            "{}{}[lines={},teams={},markets={},prices={},boundaries={},implicit={},misclassified={}]",
            reason,
            Self::format_misclassification_suffix(reason, &misclassification),
            analysis.line_count,
            analysis.team_candidates,
            analysis.market_labels,
            analysis.market_price_pairs,
            analysis.explicit_boundaries,
            analysis.implicit_boundaries,
            Self::format_misclassification_summary(&misclassification)
        ))
    }

    fn summarize_team_misclassification(lines: &[String]) -> TeamMisclassificationSummary {
        let mut summary = TeamMisclassificationSummary::default();
        let candidate_window = Self::team_slot_window(lines);

        for line in candidate_window {
            if !is_valid_team_name(line)
                || Self::parse_price(line).is_some()
                || regex::Regex::new(r"^[\d.,:+\-]+$")
                    .expect("numeric-ish regex")
                    .is_match(line)
                || line.starts_with('+')
            {
                continue;
            }

            if Self::is_team_candidate(line) {
                continue;
            }

            if Self::looks_like_league(line) {
                summary.league_like += 1;
            } else if Self::is_status_line(line) {
                summary.status_like += 1;
            } else if Self::is_market_label(line) {
                summary.market_like += 1;
            } else if Self::is_small_counter(line) {
                summary.counter_like += 1;
            }
        }

        summary
    }

    fn team_slot_window(lines: &[String]) -> &[String] {
        let start = lines
            .iter()
            .position(|line| !Self::is_structural_header_line(line))
            .unwrap_or(lines.len());
        let end = lines[start..]
            .iter()
            .position(|line| Self::is_market_label(line))
            .map(|offset| start + offset)
            .unwrap_or(lines.len());

        &lines[start..end]
    }

    fn format_misclassification_suffix(
        reason: &str,
        summary: &TeamMisclassificationSummary,
    ) -> String {
        if reason != "missing_team_pairs" {
            return String::new();
        }

        let kind = if summary.league_like >= 2 {
            Some("misclassified_as_league")
        } else if summary.status_like >= 2 {
            Some("misclassified_as_status")
        } else if summary.market_like >= 2 {
            Some("misclassified_as_market")
        } else if summary.counter_like >= 2 {
            Some("misclassified_as_counter")
        } else {
            None
        };

        kind.map(|kind| format!(":{kind}")).unwrap_or_default()
    }

    fn format_misclassification_summary(summary: &TeamMisclassificationSummary) -> String {
        format!(
            "league={},status={},market={},counter={}",
            summary.league_like, summary.status_like, summary.market_like, summary.counter_like
        )
    }

    fn count_market_price_pairs(lines: &[String]) -> usize {
        lines
            .windows(2)
            .filter(|pair| Self::is_market_label(&pair[0]) && Self::parse_price(&pair[1]).is_some())
            .count()
    }

    fn is_block_boundary(line: &str) -> bool {
        line == "Ещё" || line == "Еще"
    }

    fn parse_event_block(
        block: &[String],
        current_league: Option<&str>,
        probe: Probe,
        source_url: &str,
    ) -> Option<(Event, Vec<Odd>, String)> {
        if block.len() < 6 {
            return None;
        }

        let mut lines = block.to_vec();
        while matches!(
            lines.first().map(String::as_str),
            Some(
                "Футбол"
                    | "Баскетбол"
                    | "Хоккей"
                    | "Теннис"
                    | "Волейбол"
                    | "Настольный теннис"
                    | "Гандбол"
                    | "Мини-футбол"
                    | "Футзал"
                    | "Исход"
                    | "Тотал"
                    | "Фора"
            )
        ) {
            lines.remove(0);
        }

        if lines.len() < 5 || !lines.iter().any(|line| Self::is_market_label(line)) {
            return None;
        }

        let mut league = current_league.unwrap_or("Unknown").to_string();
        let mut start_index = 0;
        if Self::looks_like_league(&lines[0]) {
            league = lines[0].clone();
            start_index = 1;
            if lines
                .get(1)
                .is_some_and(|line| Self::is_small_counter(line))
            {
                start_index = 2;
            }
        }

        let status_index = lines
            .iter()
            .enumerate()
            .skip(start_index)
            .find(|(_, line)| Self::is_status_line(line))
            .map(|(index, _)| index)
            .or_else(|| {
                lines
                    .iter()
                    .enumerate()
                    .skip(start_index)
                    .find(|(_, line)| Self::is_market_label(line))
                    .map(|(index, _)| index)
            })?;

        let teams: Vec<String> = lines[start_index..status_index]
            .iter()
            .filter(|line| Self::is_team_candidate(line))
            .take(2)
            .cloned()
            .collect();
        if teams.len() < 2 {
            return None;
        }

        let home_team = teams[0].clone();
        let away_team = teams[1].clone();
        let event_id = format!(
            "betboom-{}-{}-{}-{}",
            if probe.is_live { "live" } else { "prematch" },
            Self::slugify(&league),
            Self::slugify(&home_team),
            Self::slugify(&away_team)
        );

        let event = Event {
            id: event_id.clone(),
            sport: probe.sport,
            league: league.clone(),
            home_team,
            away_team,
            start_time: None,
            is_live: probe.is_live,
            bookmaker_slug: "betboom".to_string(),
            raw_url: Some(source_url.to_string()),
            extra: HashMap::new(),
        };

        let now = Utc::now();
        let mut odds = Vec::new();
        let mut market_index = status_index;
        while market_index + 1 < lines.len() {
            if Self::is_market_label(&lines[market_index]) {
                if let Some(price) = Self::parse_price(&lines[market_index + 1]) {
                    let selection = lines[market_index].clone();
                    odds.push(Odd {
                        id: format!("{}-{}", event_id, Self::slugify(&selection)),
                        event_id: event_id.clone(),
                        bookmaker_slug: "betboom".to_string(),
                        market: "Main".to_string(),
                        selection: selection.clone(),
                        odds: price,
                        odds_type: Self::selection_to_odds_type(&selection),
                        line: None,
                        timestamp: now,
                    });
                    market_index += 2;
                    continue;
                }
            }
            market_index += 1;
        }

        if odds.len() < 2 {
            return None;
        }

        Some((event, odds, league))
    }

    fn looks_like_league(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::parse_price(line).is_none()
            && !regex::Regex::new(r"^[\d.,:+\-]+$")
                .expect("numeric-ish regex")
                .is_match(line)
            && !Self::is_status_line(line)
            && !Self::is_market_label(line)
            && !Self::is_small_counter(line)
            && !Self::looks_like_team_name(line)
            && (line.contains('.')
                || lower.contains("лига")
                || lower.contains("league")
                || lower.contains("liga")
                || lower.contains("кубок")
                || lower.contains("champ")
                || lower.contains("чемпион")
                || lower.contains("open"))
    }

    fn is_status_line(line: &str) -> bool {
        let lower = line.to_lowercase();
        lower.contains("сегодня")
            || lower.contains("завтра")
            || lower.contains("перерыв")
            || lower.contains("тайм")
            || lower.contains("мин")
            || lower.contains("сет")
            || lower.contains("четверть")
            || lower.contains("партия")
            || lower.contains("матч начнется")
            || regex::Regex::new(r"^\d{1,2}:\d{2}$")
                .expect("time regex")
                .is_match(line)
            || regex::Regex::new(r"^\d{1,2} [а-я]+ в \d{1,2}:\d{2}$")
                .expect("date regex")
                .is_match(&lower)
    }

    fn is_small_counter(line: &str) -> bool {
        !line.is_empty() && line.len() <= 3 && line.chars().all(|ch| ch.is_ascii_digit())
    }

    fn is_team_candidate(line: &str) -> bool {
        Self::looks_like_team_name(line)
            && !Self::is_structural_header_line(line)
            && !Self::is_market_label(line)
            && !Self::is_status_line(line)
            && !Self::is_small_counter(line)
            && !line.starts_with('+')
    }

    fn is_structural_header_line(line: &str) -> bool {
        matches!(
            line,
            "Футбол"
                | "Баскетбол"
                | "Хоккей"
                | "Теннис"
                | "Волейбол"
                | "Настольный теннис"
                | "Гандбол"
                | "Мини-футбол"
                | "Футзал"
                | "Исход"
                | "Тотал"
                | "Фора"
        )
    }

    fn looks_like_team_name(line: &str) -> bool {
        let lower = line.to_lowercase();
        is_valid_team_name(line)
            && Self::parse_price(line).is_none()
            && !regex::Regex::new(r"^[\d.,:+\-]+$")
                .expect("numeric-ish regex")
                .is_match(line)
            && !line.contains('.')
            && !lower.contains("лига")
            && !lower.contains("league")
            && !lower.contains("liga")
            && !lower.contains("чемпион")
            && !lower.contains("кубок")
    }

    fn is_market_label(line: &str) -> bool {
        matches!(line, "П1" | "П2" | "X" | "1" | "2")
            || matches!(line.to_lowercase().as_str(), "draw" | "ничья")
    }

    fn parse_price(line: &str) -> Option<f64> {
        line.replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|price| *price > 1.0)
    }

    fn slugify(value: &str) -> String {
        value
            .chars()
            .map(|ch| {
                if ch.is_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    fn selection_to_odds_type(selection: &str) -> OddsType {
        match selection.to_lowercase().as_str() {
            "1" | "п1" => OddsType::Home,
            "x" | "ничья" | "draw" => OddsType::Draw,
            "2" | "п2" => OddsType::Away,
            _ => OddsType::Custom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sporthub_helper, BetboomParser, Probe, ProbeReport, PREMATCH_FILTER_TEXT, PROBES};
    use shared::Sport;

    fn decode_hex(input: &str) -> Vec<u8> {
        let sanitized: String = input.chars().filter(|ch| !ch.is_whitespace()).collect();
        sanitized
            .as_bytes()
            .chunks(2)
            .map(|chunk| {
                let pair = std::str::from_utf8(chunk).expect("hex chunk utf8");
                u8::from_str_radix(pair, 16).expect("hex byte")
            })
            .collect()
    }

    #[test]
    fn parses_live_rendered_fixture() {
        let text = include_str!("../tests/fixtures/betboom_live_text_fixture.txt");
        let probe = Probe {
            url: "https://betboom.ru/sport/live/football",
            sport: Sport::Football,
            is_live: true,
            prematch_filter: None,
        };

        let (events, odds) = BetboomParser::parse_rendered_text(text, probe);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].league, "BudnesLiga 5x5. Лига 1");
        assert_eq!(events[0].home_team, "Штутгарт (люб)");
        assert_eq!(events[0].away_team, "Гамбург (люб)");
        assert!(events.iter().all(|event| event.is_live));
        assert_eq!(odds.len(), 9);
    }

    #[test]
    fn parses_prematch_rendered_fixture_with_league_carry() {
        let text = include_str!("../tests/fixtures/betboom_prematch_text_fixture.txt");
        let probe = Probe {
            url: "https://betboom.ru/sport/football",
            sport: Sport::Football,
            is_live: false,
            prematch_filter: Some(PREMATCH_FILTER_TEXT),
        };

        let (events, odds) = BetboomParser::parse_rendered_text(text, probe);
        assert!(events.len() >= 3);
        assert!(events.iter().all(|event| !event.is_live));
        assert!(events
            .iter()
            .all(|event| event.league == "Лига чемпионов УЕФА. 1/4"));
        assert!(events
            .iter()
            .any(|event| event.home_team == "Ливерпуль" && event.away_team == "ПСЖ"));
        assert!(odds.len() >= 9);
    }

    #[test]
    fn parses_trailing_rendered_block_without_terminal_more_marker() {
        let text = include_str!("../tests/fixtures/betboom_rendered_trailing_block_fixture.txt");
        let probe = Probe {
            url: "https://betboom.ru/sport/handball",
            sport: Sport::Handball,
            is_live: false,
            prematch_filter: Some(PREMATCH_FILTER_TEXT),
        };

        let (events, odds) = BetboomParser::parse_rendered_text(text, probe);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].league, "Россия. Суперлига");
        assert_eq!(events[0].home_team, "Пермские медведи");
        assert_eq!(events[0].away_team, "Чеховские медведи");
        assert_eq!(odds.len(), 3);
    }

    #[test]
    fn parses_flattened_cards_without_explicit_more_boundaries() {
        let text = include_str!("../tests/fixtures/betboom_flattened_cards_fixture.txt");
        let probe = Probe {
            url: "https://betboom.ru/sport/football",
            sport: Sport::Football,
            is_live: false,
            prematch_filter: Some(PREMATCH_FILTER_TEXT),
        };

        let (events, odds) = BetboomParser::parse_rendered_text(text, probe);
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| event.league == "Лига чемпионов УЕФА. 1/4"));
        assert!(events
            .iter()
            .any(|event| event.home_team == "Атлетико М" && event.away_team == "Барселона"));
        assert!(events
            .iter()
            .any(|event| event.home_team == "Ливерпуль" && event.away_team == "ПСЖ"));
        assert_eq!(odds.len(), 6);
    }

    #[test]
    fn diagnoses_flattened_single_block_empty_result_root_cause() {
        let diagnostic = BetboomParser::diagnose_empty_rendered_text(
            "Футбол\nИсход\nЛига 1\nАтлетико\nБарселона\nП1\n1.8\nX\n3.2\nП2\n4.1",
        )
        .expect("diagnostic");

        assert!(diagnostic.starts_with("single_flattened_block"));
        assert!(diagnostic.contains("markets=3"));
        assert!(diagnostic.contains("implicit=0"));
        assert!(diagnostic.contains("misclassified=league=0,status=0,market=0,counter=0"));
    }

    #[test]
    fn diagnoses_team_pair_loss_as_league_misclassification() {
        let diagnostic = BetboomParser::diagnose_empty_rendered_text(
            "Футбол\nИсход\nРоссия. Премьер-лига\nИспания. Ла Лига\nП1\n1.8\nX\n3.2\nП2\n4.1",
        )
        .expect("diagnostic");

        assert!(diagnostic.starts_with("missing_team_pairs:misclassified_as_league"));
        assert!(diagnostic.contains("misclassified=league=2,status=0,market=0,counter=0"));
    }

    #[test]
    fn diagnoses_team_pair_loss_as_status_misclassification() {
        let diagnostic = BetboomParser::diagnose_empty_rendered_text(
            "Теннис\nИсход\nСегодня\nЗавтра\nП1\n1.8\nП2\n2.1",
        )
        .expect("diagnostic");

        assert!(diagnostic.starts_with("missing_team_pairs:misclassified_as_status"));
        assert!(diagnostic.contains("misclassified=league=0,status=2,market=0,counter=0"));
    }

    #[test]
    fn surfaces_rendered_probe_summary_for_flattened_snapshot() {
        let analysis = BetboomParser::analyze_rendered_text(
            "Футбол\nИсход\nЛига 1\nАтлетико\nБарселона\nП1\n1.8\nX\n3.2\nП2\n4.1",
        )
        .expect("analysis");

        let summary = BetboomParser::format_rendered_analysis(&analysis);
        assert_eq!(
            summary,
            "lines=11,blocks=1,teams=2,markets=3,prices=3,pairs=3,boundaries=0,implicit=0"
        );
    }

    #[test]
    fn extracts_sporthub_bootstrap_hints_from_fixture() {
        let html = include_str!("../tests/fixtures/betboom_sporthub_bootstrap_fixture.html");

        let hints = sporthub_helper::extract_bootstrap_hints_from_html(html);
        assert!(hints.has_sporthub_namespace);
        assert!(hints
            .ws_urls
            .iter()
            .any(|url| url == sporthub_helper::WS_URL_HINT));
        assert!(hints
            .script_assets
            .iter()
            .any(|asset| asset.contains("sporthub-runtime.js")));
        assert!(hints
            .script_assets
            .iter()
            .any(|asset| asset.contains(sporthub_helper::PROTO_ASSET_HINT)));
        assert!(hints
            .protobuf_markers
            .iter()
            .any(|marker| marker == sporthub_helper::CHANNEL_PREMATCH));
    }

    #[test]
    fn builds_sporthub_bootstrap_plan_from_fixture() {
        let html = include_str!("../tests/fixtures/betboom_sporthub_bootstrap_fixture.html");

        let plan = sporthub_helper::build_plan_from_html(html);
        assert!(plan.bootstrap_detected);
        assert!(plan.protobuf_assets_detected);
        assert!(plan.frame_decoder_scaffolded);
        assert!(plan.runtime_guarded);
        assert_eq!(plan.config.ws_url, sporthub_helper::WS_URL_HINT);
        assert_eq!(plan.config.transport, sporthub_helper::TRANSPORT_PROTOBUF);
        assert_eq!(
            plan.config.proto_asset.as_deref(),
            Some("https://betboom.ru/assets/sporthub-feed.proto")
        );
        assert_eq!(
            plan.config.channels,
            vec![
                sporthub_helper::CHANNEL_PREMATCH.to_string(),
                sporthub_helper::CHANNEL_LIVE.to_string()
            ]
        );
        assert!(!plan.config.runtime_feature_enabled);
        assert!(!plan.config.notes.is_empty());
    }

    #[test]
    fn builds_structured_contract_manifest_and_subscription_intents() {
        let html = include_str!("../tests/fixtures/betboom_sporthub_bootstrap_fixture.html");

        let manifest = sporthub_helper::build_contract_manifest_from_html(html);
        let intents = sporthub_helper::build_subscription_intents(&manifest);

        assert_eq!(manifest.namespace, "sporthub");
        assert_eq!(manifest.ws_url, sporthub_helper::WS_URL_HINT);
        assert_eq!(manifest.transport, sporthub_helper::TRANSPORT_PROTOBUF);
        assert!(manifest
            .script_assets
            .iter()
            .any(|asset| asset.contains("sporthub-runtime.js")));
        assert_eq!(intents.len(), 2);
        assert_eq!(intents[0].channel, sporthub_helper::CHANNEL_PREMATCH);
        assert_eq!(intents[0].subscribe_mode, "intent-only:protobuf");
        assert!(intents.iter().all(|intent| intent.runtime_guarded));
    }

    #[test]
    fn inspects_length_delimited_sporthub_frame_fixture() {
        let bytes = decode_hex(include_str!(
            "../tests/fixtures/betboom_sporthub_frame_fixture.txt"
        ));

        let envelope = sporthub_helper::inspect_ws_frame(&bytes);
        assert!(envelope.length_delimited);
        assert_eq!(envelope.prefix_len, 1);
        assert_eq!(envelope.message_len, 8);
        assert_eq!(envelope.payload_hex_preview, "73706f7274687562");
    }

    #[test]
    fn classifies_sporthub_frames_without_runtime_feed() {
        let protobuf_frame = decode_hex(include_str!(
            "../tests/fixtures/betboom_sporthub_frame_fixture.txt"
        ));
        let control_frame =
            include_str!("../tests/fixtures/betboom_sporthub_control_frame_fixture.txt").as_bytes();

        let protobuf_classification = sporthub_helper::classify_ws_frame(&protobuf_frame);
        let control_classification = sporthub_helper::classify_ws_frame(control_frame);

        assert_eq!(
            protobuf_classification.class,
            sporthub_helper::FrameClass::LengthDelimitedProtobuf
        );
        assert_eq!(
            protobuf_classification.ascii_preview.as_deref(),
            Some("sporthub")
        );
        assert!(protobuf_classification.runtime_guarded);
        assert_eq!(
            control_classification.class,
            sporthub_helper::FrameClass::JsonControl
        );
        assert_eq!(
            control_classification.inferred_channel.as_deref(),
            Some(sporthub_helper::CHANNEL_PREMATCH)
        );
    }

    #[test]
    fn readiness_snapshot_keeps_feed_guarded() {
        let readiness = BetboomParser::readiness_snapshot();

        assert_eq!(
            readiness.stage,
            shared::ParserReadinessStage::DiagnosticOnly
        );
        assert!(!readiness.production_enabled);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "sporthub_contract_helpers_available"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "sporthub_bootstrap_notes_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "sporthub_runtime_feature_disabled"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "sporthub_feed_unimplemented_guardrail"));
    }

    #[test]
    fn exposes_supported_sport_probe_matrix() {
        let sports = PROBES
            .iter()
            .map(|probe| probe.sport)
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(PROBES.len(), 16);
        assert_eq!(sports.len(), 8);
        assert!(sports.contains(&Sport::Football));
        assert!(sports.contains(&Sport::Basketball));
        assert!(sports.contains(&Sport::Hockey));
        assert!(sports.contains(&Sport::Tennis));
        assert!(sports.contains(&Sport::Volleyball));
        assert!(sports.contains(&Sport::TableTennis));
        assert!(sports.contains(&Sport::Handball));
        assert!(sports.contains(&Sport::Futsal));
    }

    #[test]
    fn formats_explicit_empty_runtime_diagnostic() {
        let diagnostic = BetboomParser::format_empty_runtime_diagnostic(&[
            ProbeReport {
                url: "https://betboom.ru/sport/live/football",
                sport: Sport::Football,
                is_live: true,
                navigation_ok: true,
                navigation_error: None,
                snapshots: 2,
                rendered_chars: 420,
                strategies: vec!["sportbook_root".to_string(), "event_cards".to_string()],
                events: 0,
                odds: 0,
                preview: Some("Футбол П1 1.54 X 4.7 П2 4.0".to_string()),
                rendered_probe: Some(
                    "structured_event_cards:lines=8,blocks=1,teams=2,markets=3,prices=3,pairs=3,boundaries=0,implicit=0"
                        .to_string(),
                ),
                root_cause: Some(
                    "structured_event_cards:single_flattened_block[lines=8,teams=2,markets=3,prices=3,boundaries=0,implicit=0,misclassified=league=0,status=0,market=0,counter=0]"
                        .to_string(),
                ),
            },
            ProbeReport {
                url: "https://betboom.ru/sport/tennis",
                sport: Sport::Tennis,
                is_live: false,
                navigation_ok: false,
                navigation_error: Some("navigation timeout".to_string()),
                snapshots: 0,
                rendered_chars: 0,
                strategies: Vec::new(),
                events: 0,
                odds: 0,
                preview: None,
                rendered_probe: None,
                root_cause: Some("navigation_failed".to_string()),
            },
        ]);

        assert!(diagnostic
            .contains("BetBoom rendered runtime returned no events or odds across 2 probes"));
        assert!(diagnostic.contains("status=rendered_visible_but_parse_empty"));
        assert!(diagnostic.contains("nav_ok=1/2"));
        assert!(diagnostic.contains("nav_failed=1"));
        assert!(diagnostic.contains("snapshots_nonzero=1/2"));
        assert!(diagnostic.contains("rendered_chars_nonzero=1/2"));
        assert!(diagnostic.contains("rendered_probe_nonzero=1/2"));
        assert!(diagnostic.contains("root_cause_nonzero=2/2"));
        assert!(diagnostic.contains(
            "root_cause_counts=navigation_failed:1|single_flattened_block:1"
        ));
        assert!(diagnostic.contains("live:football: nav=true, snapshots=2, chars=420, strategies=sportbook_root|event_cards, events=0, odds=0"));
        assert!(diagnostic.contains("preview=Футбол П1 1.54 X 4.7 П2 4.0"));
        assert!(diagnostic.contains(
            "rendered_probe=structured_event_cards:lines=8,blocks=1,teams=2,markets=3,prices=3,pairs=3,boundaries=0,implicit=0"
        ));
        assert!(diagnostic.contains("root_cause=structured_event_cards:single_flattened_block"));
        assert!(diagnostic.contains("misclassified=league=0,status=0,market=0,counter=0"));
        assert!(diagnostic.contains(
            "prematch:tennis: nav=false, snapshots=0, chars=0, strategies=-, events=0, odds=0"
        ));
        assert!(diagnostic.contains("rendered_probe=-"));
        assert!(diagnostic.contains("nav_error=navigation timeout"));
        assert!(diagnostic.contains("root_cause=navigation_failed"));
    }

    #[test]
    fn summarizes_navigation_blocked_runtime_diagnostics() {
        let summary = BetboomParser::summarize_runtime_diagnostics(&[ProbeReport {
            url: "https://betboom.ru/sport/live/football",
            sport: Sport::Football,
            is_live: true,
            navigation_ok: false,
            navigation_error: Some("blocked".to_string()),
            snapshots: 0,
            rendered_chars: 0,
            strategies: Vec::new(),
            events: 0,
            odds: 0,
            preview: None,
            rendered_probe: None,
            root_cause: Some("navigation_failed".to_string()),
        }]);

        assert_eq!(summary.status, "navigation_blocked");
        assert_eq!(summary.navigation_ok, 0);
        assert_eq!(summary.navigation_failed, 1);
        assert_eq!(summary.snapshot_nonzero, 0);
        assert_eq!(summary.rendered_probe_nonzero, 0);
        assert_eq!(summary.root_cause_counts, vec![("navigation_failed".to_string(), 1)]);
    }
}

#[async_trait]
impl BookmakerParser for BetboomParser {
    fn name(&self) -> &str {
        "BetBoom"
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
        debug!(
            client_refs = Arc::strong_count(&self.client),
            "BetBoom: fetching events"
        );
        let (events, _) = self.fetch_runtime_data().await?;
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            client_refs = Arc::strong_count(&self.client),
            "BetBoom: fetching odds"
        );
        let (_, odds) = self.fetch_runtime_data().await?;
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let (events, odds) = self.fetch_runtime_data().await?;
        Ok(ParserResult::new(
            BOOKMAKER_SLUG,
            events,
            odds,
            started.elapsed().as_millis() as u64,
        ))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        USER_AGENT
    }
}
