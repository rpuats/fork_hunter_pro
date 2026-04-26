use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use reqwest::Url;
use serde_json::{json, Value};
use shared::odds::OddsType;
use shared::{DiagnosticSeverity, ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage};
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const EVENTS_LIST_URL: &str = "https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList";
const FILTER_URL: &str = "https://lds-api-sites.ligastavok.ru/rest/events/v2/filter";
const TOURNAMENT_TREE_URL: &str =
    "https://lds-api-sites.ligastavok.ru/rest/events/v8/tournamentTree";
const BASE_URL: &str = "https://www.ligastavok.ru";
const ROOT_REFERER: &str = "https://www.ligastavok.ru/";
const BOOKMAKER_SLUG: &str = "ligastavok";
const PAGE_LIMIT: u32 = 200;
const REQUEST_TIMEOUT_SECS: u64 = 20;
const COOKIE_ENV_VAR: &str = "LIGASTAVOK_COOKIE_FILE";
const COOKIE_HEADER_ENV_VAR: &str = "LIGASTAVOK_COOKIE_HEADER";
const STORAGE_STATE_ENV_VAR: &str = "LIGASTAVOK_STORAGE_STATE_FILE";
const HEADER_PROFILE_ENV_VAR: &str = "LIGASTAVOK_HEADER_PROFILE_FILE";
const BOOTSTRAP_FILE_ENV_VAR: &str = "LIGASTAVOK_BOOTSTRAP_FILE";
const ACCEPT_LANGUAGE_ENV_VAR: &str = "LIGASTAVOK_ACCEPT_LANGUAGE";
const DEFAULT_ACCEPT_LANGUAGE: &str = "ru-RU,ru;q=0.9,en;q=0.8";

/// Liga Stavok HTTP-first scaffold.
/// Uses the discovered POST `eventsList` + `tournamentTree` flow, but remains disabled by default
/// until QRATOR/session bootstrap is stable enough for unattended production traffic.
#[derive(Debug, Clone)]
pub struct LigaStavokParser {
    client: Arc<Client>,
    endpoints: Vec<Endpoint>,
    bootstrap: SessionBootstrap,
}

#[derive(Debug, Clone, Copy)]
struct Endpoint {
    referer: &'static str,
    route_hint: &'static str,
    namespace: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SportCatalogEntry {
    sport_id: u32,
    sport_name: String,
    total: usize,
    total_live: usize,
    filter_live_total: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FilterCatalogEntry {
    sport_id: u32,
    sport_name: Option<String>,
    total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionBootstrap {
    cookie_jar: Vec<BootstrapCookie>,
    cookie_header: Option<String>,
    accept_language: String,
    origin: String,
    referer: String,
    api_accept_language: Option<String>,
    api_origin: Option<String>,
    api_referer: Option<String>,
    browser_verified_api_probe_status: Option<u16>,
    direct_probe_status: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionBootstrapBlocker {
    Ready,
    ProtectionOnlyUnverifiedApi,
    ProtectionOnly,
    HeaderOnly,
    BootstrapUnavailable,
}

impl SessionBootstrapBlocker {
    fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::ProtectionOnlyUnverifiedApi => "protection_only_unverified_api",
            Self::ProtectionOnly => "protection_only",
            Self::HeaderOnly => "header_only",
            Self::BootstrapUnavailable => "bootstrap_unavailable",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StorageStateBootstrap {
    cookies: Vec<BootstrapCookie>,
    cookie_header: Option<String>,
    accept_language: Option<String>,
    origin: Option<String>,
    referer: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct HeaderProfile {
    accept_language: Option<String>,
    origin: Option<String>,
    referer: Option<String>,
    api_accept_language: Option<String>,
    api_origin: Option<String>,
    api_referer: Option<String>,
    browser_verified_api_probe_status: Option<u16>,
    direct_probe_status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BootstrapCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    expires: Option<i64>,
    secure: bool,
    host_only: bool,
}

impl LigaStavokParser {
    pub fn new(client: Arc<Client>) -> Self {
        let bootstrap = Self::load_session_bootstrap();
        Self {
            client,
            endpoints: vec![
                Endpoint {
                    referer: "https://www.ligastavok.ru/line/football",
                    route_hint: "line",
                    namespace: "prematch",
                },
                Endpoint {
                    referer: "https://www.ligastavok.ru/live/football",
                    route_hint: "live",
                    namespace: "live",
                },
            ],
            bootstrap,
        }
    }

    fn load_session_bootstrap() -> SessionBootstrap {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let header_profile = Self::load_header_profile(&manifest_dir).unwrap_or_default();
        let storage_state = Self::load_storage_state(&manifest_dir).unwrap_or_default();
        let cookie_header = Self::merge_cookie_headers(
            std::env::var(COOKIE_HEADER_ENV_VAR)
                .ok()
                .and_then(|value| Self::normalize_cookie_header(&value)),
            storage_state.cookie_header.clone(),
        );

        let accept_language = std::env::var(ACCEPT_LANGUAGE_ENV_VAR)
            .ok()
            .and_then(|value| Self::normalize_header_value(&value))
            .or(header_profile.accept_language)
            .or(storage_state.accept_language)
            .unwrap_or_else(|| DEFAULT_ACCEPT_LANGUAGE.to_string());
        let origin = header_profile
            .origin
            .or(storage_state.origin)
            .unwrap_or_else(|| BASE_URL.to_string());
        let referer = header_profile
            .referer
            .or(storage_state.referer)
            .unwrap_or_else(|| ROOT_REFERER.to_string());

        SessionBootstrap {
            cookie_jar: storage_state.cookies,
            cookie_header,
            accept_language,
            origin,
            referer,
            api_accept_language: header_profile.api_accept_language,
            api_origin: header_profile.api_origin,
            api_referer: header_profile.api_referer,
            browser_verified_api_probe_status: header_profile.browser_verified_api_probe_status,
            direct_probe_status: header_profile.direct_probe_status,
        }
    }

    fn load_header_profile(manifest_dir: &Path) -> Option<HeaderProfile> {
        let mut candidates = Vec::new();

        if let Ok(path) = std::env::var(BOOTSTRAP_FILE_ENV_VAR) {
            candidates.push(PathBuf::from(path));
        }
        if let Ok(path) = std::env::var(HEADER_PROFILE_ENV_VAR) {
            candidates.push(PathBuf::from(path));
        }

        candidates.push(manifest_dir.join("../../ligastavok_bootstrap.json"));
        candidates.push(manifest_dir.join("../../ligastavok_header_profile.json"));
        candidates.push(
            manifest_dir
                .join("../../tools/discovery_output/ligastavok/ligastavok_discovery_latest.json"),
        );
        candidates.push(PathBuf::from("ligastavok_bootstrap.json"));
        candidates.push(PathBuf::from("ligastavok_header_profile.json"));
        candidates.push(PathBuf::from(
            "tools/discovery_output/ligastavok/ligastavok_discovery_latest.json",
        ));

        candidates.into_iter().find_map(|path| {
            let contents = fs::read_to_string(path).ok()?;
            let value: Value = serde_json::from_str(&contents).ok()?;
            Self::extract_header_profile(&value)
        })
    }

    fn load_storage_state(manifest_dir: &Path) -> Option<StorageStateBootstrap> {
        let mut candidates = Vec::new();

        if let Ok(path) = std::env::var(BOOTSTRAP_FILE_ENV_VAR) {
            candidates.push(PathBuf::from(path));
        }
        if let Ok(path) = std::env::var(STORAGE_STATE_ENV_VAR) {
            candidates.push(PathBuf::from(path));
        }
        if let Ok(path) = std::env::var(COOKIE_ENV_VAR) {
            candidates.push(PathBuf::from(path));
        }

        candidates.push(manifest_dir.join("../../ligastavok_bootstrap.json"));
        candidates.push(manifest_dir.join("../../ligastavok_storage_state.json"));
        candidates.push(manifest_dir.join("../../ligastavok_cookies.json"));
        candidates.push(
            manifest_dir
                .join("../../tools/discovery_output/ligastavok/ligastavok_discovery_latest.json"),
        );
        candidates.push(PathBuf::from("ligastavok_bootstrap.json"));
        candidates.push(PathBuf::from("ligastavok_storage_state.json"));
        candidates.push(PathBuf::from("ligastavok_cookies.json"));
        candidates.push(PathBuf::from(
            "tools/discovery_output/ligastavok/ligastavok_discovery_latest.json",
        ));

        candidates.into_iter().find_map(|path| {
            let contents = fs::read_to_string(path).ok()?;
            let value: Value = serde_json::from_str(&contents).ok()?;
            Self::extract_storage_state_bootstrap(&value)
        })
    }

    fn normalize_header_value(value: &str) -> Option<String> {
        let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    fn normalize_cookie_header(value: &str) -> Option<String> {
        let normalized = value
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty() && part.contains('='))
            .collect::<Vec<_>>()
            .join("; ");

        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    fn merge_cookie_headers(primary: Option<String>, secondary: Option<String>) -> Option<String> {
        let mut pairs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for header in [primary, secondary].into_iter().flatten() {
            for part in header
                .split(';')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                let Some((name, value)) = part.split_once('=') else {
                    continue;
                };
                let name = name.trim();
                let value = value.trim();
                if name.is_empty() || value.is_empty() || !seen.insert(name.to_ascii_lowercase()) {
                    continue;
                }
                pairs.push(format!("{name}={value}"));
            }
        }

        if pairs.is_empty() {
            None
        } else {
            Some(pairs.join("; "))
        }
    }

    fn normalize_origin(value: &str) -> Option<String> {
        let normalized = value.trim().trim_end_matches('/');
        if normalized.is_empty() {
            return None;
        }

        let host = normalized.to_ascii_lowercase();
        if host == "https://ligastavok.ru"
            || host == "https://www.ligastavok.ru"
            || host.ends_with(".ligastavok.ru")
        {
            Some(normalized.to_string())
        } else {
            None
        }
    }

    fn normalize_referer(value: &str) -> Option<String> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return None;
        }

        let url = Url::parse(normalized).ok()?;
        let origin = format!("{}://{}", url.scheme(), url.host_str()?);
        if Self::normalize_origin(&origin).is_none() {
            return None;
        }

        Some(normalized.to_string())
    }

    fn cookie_header_from_path(path: &Path) -> Option<String> {
        let contents = fs::read_to_string(path).ok()?;
        let value: Value = serde_json::from_str(&contents).ok()?;
        let cookies = Self::extract_cookie_values(&value)?;

        Self::build_cookie_header(&cookies)
    }

    fn build_cookie_header(cookies: &[Value]) -> Option<String> {
        let jar = Self::parse_bootstrap_cookies(cookies);
        Self::build_cookie_header_for_url(&jar, ROOT_REFERER)
    }

    fn extract_cookie_values(value: &Value) -> Option<Vec<Value>> {
        match value {
            Value::Array(items) => Some(items.clone()),
            Value::Object(map) => map
                .get("cookies")
                .and_then(|cookies| cookies.as_array())
                .cloned()
                .or_else(|| {
                    map.get("storageState")
                        .and_then(|state| state.get("cookies"))
                        .and_then(|cookies| cookies.as_array())
                        .cloned()
                }),
            _ => None,
        }
    }

    fn extract_storage_state_bootstrap(value: &Value) -> Option<StorageStateBootstrap> {
        let cookies = Self::extract_cookie_values(value).unwrap_or_default();
        let parsed_cookies = Self::parse_bootstrap_cookies(&cookies);
        let cookie_header = Self::merge_cookie_headers(
            Self::extract_first_str(
                value,
                &[
                    &["cookieHeader"],
                    &["storageState", "cookieHeader"],
                    &["storage_state", "cookieHeader"],
                    &["storage_state", "cookie_header"],
                ],
            )
            .and_then(|value| Self::normalize_cookie_header(&value)),
            Self::build_cookie_header_for_url(&parsed_cookies, ROOT_REFERER),
        );
        let origin =
            Self::extract_storage_origin(value).or_else(|| Self::extract_manifest_origin(value));
        let accept_language = Self::extract_storage_accept_language(value)
            .or_else(|| Self::extract_manifest_accept_language(value));
        let referer = Self::extract_manifest_referer(value).or_else(|| {
            origin
                .as_deref()
                .and_then(|origin| Self::normalize_referer(&format!("{origin}/")))
        });

        if cookie_header.is_none() && origin.is_none() && accept_language.is_none() {
            None
        } else {
            Some(StorageStateBootstrap {
                cookies: parsed_cookies,
                cookie_header,
                accept_language,
                origin,
                referer,
            })
        }
    }

    fn extract_storage_origin(value: &Value) -> Option<String> {
        let origins = value
            .get("origins")
            .or_else(|| {
                value
                    .get("storageState")
                    .and_then(|state| state.get("origins"))
            })
            .and_then(|origins| origins.as_array())?;

        origins
            .iter()
            .filter_map(|origin| origin.get("origin").and_then(|value| value.as_str()))
            .find_map(Self::normalize_origin)
    }

    fn extract_manifest_origin(value: &Value) -> Option<String> {
        Self::extract_first_str(
            value,
            &[
                &["origin"],
                &["storageState", "origin"],
                &["storage_state", "origin"],
                &["headerProfile", "origin"],
                &["header_profile", "origin"],
                &["final_url"],
                &["finalUrl"],
            ],
        )
        .and_then(|raw| {
            Self::normalize_origin(&raw).or_else(|| {
                Self::normalize_referer(&raw).and_then(|referer| {
                    let url = Url::parse(&referer).ok()?;
                    let host = url.host_str()?;
                    Self::normalize_origin(&format!("{}://{}", url.scheme(), host))
                })
            })
        })
    }

    fn extract_storage_accept_language(value: &Value) -> Option<String> {
        let origins = value
            .get("origins")
            .or_else(|| {
                value
                    .get("storageState")
                    .and_then(|state| state.get("origins"))
            })
            .and_then(|origins| origins.as_array())?;

        for origin in origins {
            let Some(origin_url) = origin.get("origin").and_then(|value| value.as_str()) else {
                continue;
            };
            if Self::normalize_origin(origin_url).is_none() {
                continue;
            }

            let Some(local_storage) = origin
                .get("localStorage")
                .and_then(|value| value.as_array())
            else {
                continue;
            };

            for item in local_storage {
                let Some(name) = item.get("name").and_then(|value| value.as_str()) else {
                    continue;
                };
                if !matches!(
                    name,
                    "i18nextLng"
                        | "locale"
                        | "lang"
                        | "language"
                        | "accept-language"
                        | "acceptLanguage"
                ) {
                    continue;
                }

                let Some(raw_value) = item.get("value").and_then(|value| value.as_str()) else {
                    continue;
                };
                let normalized = raw_value.trim().trim_matches('"').replace('_', "-");
                if normalized.is_empty() {
                    continue;
                }
                if normalized.contains(',') || normalized.contains(';') {
                    return Self::normalize_header_value(&normalized);
                }

                let primary = normalized
                    .split('-')
                    .next()
                    .unwrap_or("ru")
                    .to_ascii_lowercase();
                return Some(format!("{normalized},{primary};q=0.9,en;q=0.8"));
            }
        }

        None
    }

    fn extract_manifest_accept_language(value: &Value) -> Option<String> {
        Self::extract_first_str(
            value,
            &[
                &["accept_language"],
                &["acceptLanguage"],
                &["storageState", "acceptLanguage"],
                &["storage_state", "acceptLanguage"],
                &["storage_state", "accept_language"],
                &["headerProfile", "accept_language"],
                &["headerProfile", "acceptLanguage"],
                &["header_profile", "accept_language"],
                &["extraHTTPHeaders", "Accept-Language"],
                &["extraHTTPHeaders", "accept-language"],
            ],
        )
        .and_then(|value| Self::normalize_header_value(&value))
    }

    fn extract_manifest_referer(value: &Value) -> Option<String> {
        Self::extract_first_str(
            value,
            &[
                &["referer"],
                &["referrer"],
                &["storageState", "referer"],
                &["storage_state", "referer"],
                &["headerProfile", "referer"],
                &["header_profile", "referer"],
                &["final_url"],
                &["finalUrl"],
            ],
        )
        .and_then(|value| Self::normalize_referer(&value))
    }

    fn extract_first_str(value: &Value, paths: &[&[&str]]) -> Option<String> {
        paths.iter().find_map(|path| {
            let mut current = value;
            for key in *path {
                current = current.get(*key)?;
            }
            current.as_str().map(|value| value.to_string())
        })
    }

    fn extract_header_profile(value: &Value) -> Option<HeaderProfile> {
        let root = value
            .get("headerProfile")
            .or_else(|| value.get("header_profile"))
            .unwrap_or(value);
        let headers = root.get("headers").unwrap_or(root);
        let extra_headers = root
            .get("extraHTTPHeaders")
            .or_else(|| value.get("extraHTTPHeaders"))
            .unwrap_or(headers);

        let accept_language = extra_headers
            .get("accept-language")
            .or_else(|| extra_headers.get("Accept-Language"))
            .or_else(|| headers.get("acceptLanguage"))
            .or_else(|| headers.get("accept_language"))
            .or_else(|| headers.get("locale"))
            .and_then(|value| value.as_str())
            .and_then(Self::normalize_header_value);
        let origin = extra_headers
            .get("origin")
            .or_else(|| extra_headers.get("Origin"))
            .or_else(|| headers.get("origin"))
            .or_else(|| headers.get("baseUrl"))
            .and_then(|value| value.as_str())
            .and_then(Self::normalize_origin);
        let referer = extra_headers
            .get("referer")
            .or_else(|| extra_headers.get("Referer"))
            .or_else(|| headers.get("referer"))
            .or_else(|| headers.get("referrer"))
            .and_then(|value| value.as_str())
            .and_then(Self::normalize_referer);
        let api_headers = root
            .get("api_headers")
            .or_else(|| root.get("apiHeaders"))
            .or_else(|| value.get("api_headers"))
            .or_else(|| value.get("apiHeaders"));
        let api_accept_language = api_headers
            .and_then(|headers| {
                headers
                    .get("accept-language")
                    .or_else(|| headers.get("Accept-Language"))
                    .and_then(|value| value.as_str())
            })
            .and_then(Self::normalize_header_value);
        let api_origin = api_headers
            .and_then(|headers| {
                headers
                    .get("origin")
                    .or_else(|| headers.get("Origin"))
                    .and_then(|value| value.as_str())
            })
            .and_then(Self::normalize_origin);
        let api_referer = api_headers
            .and_then(|headers| {
                headers
                    .get("referer")
                    .or_else(|| headers.get("Referer"))
                    .and_then(|value| value.as_str())
            })
            .and_then(Self::normalize_referer);
        let direct_probe_status = root
            .get("direct_probe_status")
            .or_else(|| root.get("directProbeStatus"))
            .or_else(|| value.get("direct_probe_status"))
            .or_else(|| value.get("directProbeStatus"))
            .or_else(|| {
                value
                    .get("status")
                    .and_then(|status| status.get("direct_probe_status"))
            })
            .or_else(|| {
                value
                    .get("status")
                    .and_then(|status| status.get("directProbeStatus"))
            })
            .and_then(|value| value.as_u64())
            .and_then(|value| u16::try_from(value).ok())
            .filter(|status| *status > 0);
        let browser_verified_api_probe_status = Self::extract_first_u16(
            value,
            &[
                &["headerProfile", "browser_verified_api_probe_status"],
                &["headerProfile", "browserVerifiedApiProbeStatus"],
                &["headerProfile", "browser_verified_api_probe", "status"],
                &["headerProfile", "browserVerifiedApiProbe", "status"],
                &["browser_verified_api_probe_status"],
                &["browserVerifiedApiProbeStatus"],
                &["browser_verified_api_probe", "status"],
                &["browserVerifiedApiProbe", "status"],
                &["runtimeBootstrap", "browser_verified_api_probe_status"],
                &["runtimeBootstrap", "browserVerifiedApiProbeStatus"],
                &["runtimeBootstrap", "browser_verified_api_probe", "status"],
                &["runtimeBootstrap", "browserVerifiedApiProbe", "status"],
                &["status", "browser_verified_api_probe_status"],
                &["status", "browserVerifiedApiProbeStatus"],
            ],
        );

        if accept_language.is_none()
            && origin.is_none()
            && referer.is_none()
            && api_accept_language.is_none()
            && api_origin.is_none()
            && api_referer.is_none()
            && browser_verified_api_probe_status.is_none()
            && direct_probe_status.is_none()
        {
            None
        } else {
            Some(HeaderProfile {
                accept_language,
                origin,
                referer,
                api_accept_language,
                api_origin,
                api_referer,
                browser_verified_api_probe_status,
                direct_probe_status,
            })
        }
    }

    fn extract_first_u16(value: &Value, paths: &[&[&str]]) -> Option<u16> {
        Self::extract_first_value(value, paths)
            .and_then(|value| value.as_u64())
            .and_then(|value| u16::try_from(value).ok())
            .filter(|status| *status > 0)
    }

    fn extract_first_value<'a>(value: &'a Value, paths: &[&[&str]]) -> Option<&'a Value> {
        paths.iter().find_map(|path| {
            let mut current = value;
            for segment in *path {
                current = current.get(*segment)?;
            }
            Some(current)
        })
    }

    fn parse_bootstrap_cookies(cookies: &[Value]) -> Vec<BootstrapCookie> {
        cookies
            .iter()
            .filter_map(Self::parse_bootstrap_cookie)
            .collect()
    }

    fn parse_bootstrap_cookie(cookie: &Value) -> Option<BootstrapCookie> {
        let name = cookie.get("name")?.as_str()?.trim();
        let value = cookie.get("value")?.as_str()?.trim();
        let domain = cookie
            .get("domain")
            .and_then(|value| value.as_str())
            .unwrap_or("www.ligastavok.ru")
            .trim()
            .to_ascii_lowercase();
        let path = cookie
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or("/")
            .trim();
        let expires = cookie
            .get("expires")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()));
        let secure = cookie
            .get("secure")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let host_only = cookie
            .get("hostOnly")
            .and_then(|value| value.as_bool())
            .unwrap_or_else(|| cookie.get("domain").is_none());

        if name.is_empty() || value.is_empty() || !Self::cookie_matches_target(&domain, path) {
            return None;
        }
        if expires.is_some_and(|value| value > 0 && value <= Utc::now().timestamp()) {
            return None;
        }

        Some(BootstrapCookie {
            name: name.to_string(),
            value: value.to_string(),
            domain,
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
            expires,
            secure,
            host_only,
        })
    }

    fn build_cookie_header_for_url(cookies: &[BootstrapCookie], url: &str) -> Option<String> {
        let url = Url::parse(url).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        let path = if url.path().is_empty() {
            "/"
        } else {
            url.path()
        };
        let is_https = url.scheme() == "https";

        let mut matched = cookies
            .iter()
            .filter(|cookie| Self::cookie_matches_url(cookie, &host, path, is_https))
            .collect::<Vec<_>>();
        matched.sort_by(|left, right| {
            right
                .path
                .len()
                .cmp(&left.path.len())
                .then_with(|| left.name.cmp(&right.name))
        });

        let mut seen = std::collections::HashSet::new();
        let joined = matched
            .into_iter()
            .filter(|cookie| seen.insert(cookie.name.to_ascii_lowercase()))
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; ");

        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    }

    fn cookie_matches_url(
        cookie: &BootstrapCookie,
        host: &str,
        path: &str,
        is_https: bool,
    ) -> bool {
        let domain = cookie.domain.trim().trim_start_matches('.');
        let domain_matches = if cookie.host_only {
            host == domain
        } else {
            host == domain || host.ends_with(&format!(".{domain}"))
        };
        let cookie_path = if cookie.path.is_empty() {
            "/"
        } else {
            &cookie.path
        };
        let path_matches = path == cookie_path
            || path.starts_with(cookie_path)
            || (cookie_path.ends_with('/') && path.starts_with(cookie_path.trim_end_matches('/')));

        domain_matches && path_matches && (!cookie.secure || is_https)
    }

    fn cookie_matches_target(domain: &str, path: &str) -> bool {
        let normalized_domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
        let normalized_path = path.trim();

        (normalized_domain.is_empty()
            || normalized_domain == "ligastavok.ru"
            || normalized_domain.ends_with(".ligastavok.ru"))
            && (normalized_path.is_empty()
                || normalized_path == "/"
                || normalized_path.starts_with('/'))
    }

    fn browser_headers(
        &self,
        endpoint: Endpoint,
        is_document: bool,
        target_url: &str,
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let accept_language = if is_document {
            &self.bootstrap.accept_language
        } else {
            self.bootstrap
                .api_accept_language
                .as_deref()
                .unwrap_or(&self.bootstrap.accept_language)
        };
        headers.insert(
            HeaderName::from_static("accept-language"),
            HeaderValue::from_str(accept_language)
                .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_ACCEPT_LANGUAGE)),
        );
        let referer = if is_document {
            endpoint.referer
        } else {
            self.bootstrap
                .api_referer
                .as_deref()
                .unwrap_or(&self.bootstrap.referer)
        };
        headers.insert(
            HeaderName::from_static("referer"),
            HeaderValue::from_str(referer)
                .unwrap_or_else(|_| HeaderValue::from_static(ROOT_REFERER)),
        );
        let origin = if is_document {
            &self.bootstrap.origin
        } else {
            self.bootstrap
                .api_origin
                .as_deref()
                .unwrap_or(&self.bootstrap.origin)
        };
        if let Ok(value) = HeaderValue::from_str(origin) {
            headers.insert(HeaderName::from_static("origin"), value);
        }
        headers.insert(
            HeaderName::from_static("sec-ch-ua"),
            HeaderValue::from_static("\"Chromium\";v=\"145\", \"Not:A-Brand\";v=\"99\""),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-mobile"),
            HeaderValue::from_static("?0"),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"Windows\""),
        );

        if is_document {
            headers.insert(
                HeaderName::from_static("upgrade-insecure-requests"),
                HeaderValue::from_static("1"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-dest"),
                HeaderValue::from_static("document"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-mode"),
                HeaderValue::from_static("navigate"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-site"),
                HeaderValue::from_static("same-origin"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-user"),
                HeaderValue::from_static("?1"),
            );
        } else {
            headers.insert(
                HeaderName::from_static("sec-fetch-dest"),
                HeaderValue::from_static("empty"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-mode"),
                HeaderValue::from_static("cors"),
            );
            headers.insert(
                HeaderName::from_static("sec-fetch-site"),
                HeaderValue::from_static("same-site"),
            );
            headers.insert(
                HeaderName::from_static("x-application-name"),
                HeaderValue::from_static("mobile"),
            );
        }

        let cookie_header = Self::merge_cookie_headers(
            self.bootstrap.cookie_header.clone(),
            Self::build_cookie_header_for_url(&self.bootstrap.cookie_jar, target_url),
        );
        if let Some(cookie_header) = cookie_header {
            if let Ok(value) = HeaderValue::from_str(&cookie_header) {
                headers.insert(HeaderName::from_static("cookie"), value);
            }
        }

        headers
    }

    fn has_cookie_bootstrap(&self) -> bool {
        self.bootstrap.cookie_header.is_some() || !self.bootstrap.cookie_jar.is_empty()
    }

    fn has_validated_session_bootstrap(&self) -> bool {
        self.bootstrap_cookie_names()
            .iter()
            .any(|name| !Self::is_protection_cookie_name(name))
    }

    fn bootstrap_cookie_names(&self) -> Vec<String> {
        let mut names = self
            .bootstrap
            .cookie_jar
            .iter()
            .map(|cookie| cookie.name.clone())
            .collect::<Vec<_>>();

        if let Some(header) = &self.bootstrap.cookie_header {
            for part in header
                .split(';')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                let Some((name, _)) = part.split_once('=') else {
                    continue;
                };
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    names.push(trimmed.to_string());
                }
            }
        }

        names.sort_by_key(|name| name.to_ascii_lowercase());
        names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        names
    }

    fn is_protection_cookie_name(name: &str) -> bool {
        let lower = name.trim().to_ascii_lowercase();
        lower.starts_with("qrator_")
            || lower.starts_with("__qrator")
            || lower.starts_with("qauth_")
            || lower.starts_with("qab")
    }

    fn has_header_bootstrap(&self) -> bool {
        self.bootstrap.accept_language != DEFAULT_ACCEPT_LANGUAGE
            || self.bootstrap.origin != BASE_URL
            || self.bootstrap.referer != ROOT_REFERER
            || self.bootstrap.api_accept_language.is_some()
            || self.bootstrap.api_origin.is_some()
            || self.bootstrap.api_referer.is_some()
            || self.bootstrap.browser_verified_api_probe_status.is_some()
            || self.bootstrap.direct_probe_status.is_some()
    }

    fn has_browser_verified_api_probe(&self) -> bool {
        self.bootstrap
            .browser_verified_api_probe_status
            .is_some_and(|status| (200..400).contains(&status))
    }

    fn has_direct_probe_success(&self) -> bool {
        self.bootstrap
            .direct_probe_status
            .is_some_and(|status| (200..400).contains(&status))
    }

    fn can_attempt_runtime_with_bootstrap(&self) -> bool {
        self.has_validated_session_bootstrap()
            || (self.has_cookie_bootstrap() && self.has_browser_verified_api_probe())
    }

    fn session_bootstrap_blocker(&self) -> SessionBootstrapBlocker {
        if self.has_validated_session_bootstrap() {
            SessionBootstrapBlocker::Ready
        } else if self.has_cookie_bootstrap() && !self.has_browser_verified_api_probe() {
            SessionBootstrapBlocker::ProtectionOnlyUnverifiedApi
        } else if self.has_cookie_bootstrap() {
            SessionBootstrapBlocker::ProtectionOnly
        } else if self.has_header_bootstrap() {
            SessionBootstrapBlocker::HeaderOnly
        } else {
            SessionBootstrapBlocker::BootstrapUnavailable
        }
    }

    fn session_bootstrap_summary(&self) -> String {
        let cookie_names = self.bootstrap_cookie_names();
        let non_protection_cookie_count = cookie_names
            .iter()
            .filter(|name| !Self::is_protection_cookie_name(name))
            .count();
        let protection_only = !cookie_names.is_empty() && non_protection_cookie_count == 0;
        let bootstrap_blocker = self.session_bootstrap_blocker();

        format!(
            "bootstrap_blocker={}; validated_session_bootstrap={}; cookie_names={}; cookie_count={}; non_protection_cookie_count={}; protection_only={}; has_manual_cookie_header={}; accept_language={}; origin={}; referer={}",
            bootstrap_blocker.label(),
            matches!(bootstrap_blocker, SessionBootstrapBlocker::Ready),
            if cookie_names.is_empty() {
                "-".to_string()
            } else {
                cookie_names.join("|")
            },
            cookie_names.len(),
            non_protection_cookie_count,
            protection_only,
            self.bootstrap.cookie_header.is_some(),
            self.bootstrap.accept_language,
            self.bootstrap.origin,
            self.bootstrap.referer,
        ) + &format!(
            "; api_accept_language={}; api_origin={}; api_referer={}; browser_verified_api_probe_status={}; browser_verified_api_probe={}; direct_probe_status={}; direct_probe_success={}",
            self.bootstrap
                .api_accept_language
                .as_deref()
                .unwrap_or("-"),
            self.bootstrap.api_origin.as_deref().unwrap_or("-"),
            self.bootstrap.api_referer.as_deref().unwrap_or("-"),
            self.bootstrap
                .browser_verified_api_probe_status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.has_browser_verified_api_probe(),
            self.bootstrap
                .direct_probe_status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.has_direct_probe_success(),
        )
    }

    fn build_events_payload(&self, sport_id: u32, namespace: &str, skip: u32) -> Value {
        json!({
            "gameId": [sport_id],
            "sportId": sport_id,
            "limit": PAGE_LIMIT,
            "skip": skip,
            "topEvents": false,
            "ts": Utc::now().timestamp_millis(),
            "widgetVideo": false,
            "filters": {},
            "lineType": "home",
            "method": "standard",
            "ns": [namespace],
            "proposedType": "MAINOFFER",
            "proposedTypes": ["MAINOFFER"],
        })
    }

    fn namespace_payload_candidates(namespace: &str) -> Vec<&str> {
        match namespace {
            "prematch" => vec!["prematch", "line"],
            "live" => vec!["live"],
            _ => vec![namespace],
        }
    }

    fn endpoint_for_namespace(&self, namespace: &str) -> Endpoint {
        self.endpoints
            .iter()
            .copied()
            .find(|endpoint| endpoint.namespace == namespace)
            .unwrap_or(self.endpoints[0])
    }

    async fn warm_up_session(
        &self,
        endpoint: Endpoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for url in [BASE_URL, endpoint.referer] {
            let response = self
                .client
                .get(url)
                .header("User-Agent", USER_AGENT)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
                )
                .headers(self.browser_headers(endpoint, true, url))
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .send()
                .await?;
            let body = response.text().await?;
            if !Self::looks_like_protection_page(&body) {
                return Ok(());
            }
        }

        Err(Self::boxed_error(format!(
            "QRATOR blocked warm-up navigation for {}",
            endpoint.referer
        )))
    }

    async fn fetch_json(
        &self,
        url: &str,
        endpoint: Endpoint,
        payload: &Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let req_id = Self::request_id();
        debug!(url, referer = endpoint.referer, route_hint = endpoint.route_hint, request_id = req_id, payload = %payload, "Liga Stavok: sending JSON probe");

        let request = self
            .client
            .post(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .headers(self.browser_headers(endpoint, false, url))
            .header("x-req-id", req_id)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS));

        let request = match payload {
            Value::Object(map) if map.is_empty() => request,
            _ => request.json(payload),
        };

        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(Self::boxed_error(format!(
                "{} returned status {} for {} ({})",
                url,
                response.status(),
                endpoint.route_hint,
                self.session_bootstrap_summary()
            )));
        }

        let body = response.text().await?;
        if Self::looks_like_protection_page(&body) {
            return Err(Self::boxed_error(format!(
                "QRATOR blocked {} for {} ({})",
                url,
                endpoint.route_hint,
                self.session_bootstrap_summary()
            )));
        }

        serde_json::from_str(&body).map_err(|error| {
            Self::boxed_error(format!("failed to parse {} response JSON: {}", url, error))
        })
    }

    fn looks_like_protection_page(body: &str) -> bool {
        let lower = body.to_lowercase();
        lower.contains("qauth_show_captcha")
            || lower.contains("доступ заблокирован системой защиты")
            || lower.contains("please, complete the captcha")
            || lower.contains("__qrator")
            || lower.contains("tag: qab")
    }

    fn boxed_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::other(message.into()))
    }

    fn branch_error(
        branch: &str,
        detail: impl Into<String>,
    ) -> Box<dyn std::error::Error + Send + Sync> {
        Self::boxed_error(format!("Liga Stavok {branch} branch: {}", detail.into()))
    }

    fn describe_filter_payload(json: &Value) -> String {
        match json.get("result") {
            Some(Value::Array(items)) if items.is_empty() => "result array is empty".to_string(),
            Some(Value::Array(items)) => format!(
                "result array has {} entries but none with positive totals",
                items.len()
            ),
            Some(other) => format!("result is {} instead of an array", Self::json_type(other)),
            None => {
                format!(
                    "result key is missing; top-level keys: {}",
                    Self::top_level_keys(json)
                )
            }
        }
    }

    fn describe_tournament_tree_payload(json: &Value) -> String {
        match json.get("result") {
            Some(Value::Array(items)) if items.is_empty() => "result array is empty".to_string(),
            Some(Value::Array(items)) => format!(
                "result array has {} entries but none with non-zero sport totals",
                items.len()
            ),
            Some(other) => format!("result is {} instead of an array", Self::json_type(other)),
            None => {
                format!(
                    "result key is missing; top-level keys: {}",
                    Self::top_level_keys(json)
                )
            }
        }
    }

    fn describe_events_list_payload(json: &Value) -> String {
        if let Some(items) = json
            .get("result")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.as_array())
        {
            let namespaces = items
                .iter()
                .filter_map(|item| item.get("ns").and_then(|value| value.as_str()))
                .take(3)
                .map(str::to_string)
                .collect::<Vec<_>>();

            return if items.is_empty() {
                "result.data is present but empty".to_string()
            } else if namespaces.is_empty() {
                format!("result.data has {} entries", items.len())
            } else {
                format!(
                    "result.data has {} entries with namespaces {}",
                    items.len(),
                    namespaces.join(",")
                )
            };
        }

        if let Some(items) = json.get("data").and_then(|value| value.as_array()) {
            return if items.is_empty() {
                "data is present but empty".to_string()
            } else {
                format!("data has {} entries", items.len())
            };
        }

        let result_shape = json.get("result").map(Self::json_type).unwrap_or("missing");
        format!(
            "schema does not expose result.data or data arrays (result={result_shape}; top-level keys: {})",
            Self::top_level_keys(json)
        )
    }

    fn json_type(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    fn top_level_keys(json: &Value) -> String {
        json.as_object()
            .map(|map| {
                let mut keys = map.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                if keys.is_empty() {
                    "<none>".to_string()
                } else {
                    keys.join(",")
                }
            })
            .unwrap_or_else(|| Self::json_type(json).to_string())
    }

    async fn fetch_sport_catalog(
        &self,
    ) -> Result<Vec<SportCatalogEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self.endpoint_for_namespace("prematch");
        if let Err(error) = self.warm_up_session(endpoint).await {
            if self.can_attempt_runtime_with_bootstrap() {
                warn!(
                    %error,
                    session = %self.session_bootstrap_summary(),
                    "Liga Stavok warm-up blocked, continuing with bootstrap-backed API probe"
                );
            } else {
                return Err(Self::branch_error(
                    "preflight",
                    format!(
                        "warm-up navigation refused before API bootstrap: {error}; {}",
                        self.session_bootstrap_summary()
                    ),
                ));
            }
        } else if !self.can_attempt_runtime_with_bootstrap() {
            info!(
                session = %self.session_bootstrap_summary(),
                "Liga Stavok warm-up succeeded without validated session bootstrap; proceeding on browser headers only"
            );
        }

        let filter_catalog = match self.fetch_filter_catalog(endpoint).await {
            Ok(entries) => entries,
            Err(error) => {
                warn!(%error, "Liga Stavok filter preflight failed, continuing with tournament tree only");
                Vec::new()
            }
        };
        let json = self
            .fetch_json(TOURNAMENT_TREE_URL, endpoint, &json!({}))
            .await?;
        let sports =
            Self::merge_filter_catalog(Self::parse_tournament_tree(&json), &filter_catalog);
        if sports.is_empty() {
            let filter_note = if filter_catalog.is_empty() {
                "filter preflight yielded no live-sport hints".to_string()
            } else {
                format!(
                    "filter preflight yielded {} live-sport hints",
                    filter_catalog.len()
                )
            };
            return Err(Self::branch_error(
                "tournamentTree",
                format!(
                    "{}; {filter_note}",
                    Self::describe_tournament_tree_payload(&json)
                ),
            ));
        }
        Ok(sports)
    }

    async fn fetch_filter_catalog(
        &self,
        endpoint: Endpoint,
    ) -> Result<Vec<FilterCatalogEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let json = self.fetch_json(FILTER_URL, endpoint, &json!({})).await?;
        let entries = Self::parse_filter_catalog(&json);
        if entries.is_empty() {
            return Err(Self::branch_error(
                "filter",
                format!(
                    "preflight returned no usable sport totals: {}",
                    Self::describe_filter_payload(&json)
                ),
            ));
        }
        Ok(entries)
    }

    async fn fetch_sport_namespace(
        &self,
        sport: &SportCatalogEntry,
        namespace: &str,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let expected_total = match namespace {
            "live" => sport.filter_live_total.unwrap_or(sport.total_live),
            _ => sport.total.saturating_sub(sport.total_live),
        };

        if expected_total == 0 {
            return Ok((Vec::new(), Vec::new()));
        }

        let endpoint = self.endpoint_for_namespace(namespace);
        let mut last_error = None;

        for payload_namespace in Self::namespace_payload_candidates(namespace) {
            let mut events = Vec::new();
            let mut odds = Vec::new();
            let mut candidate_error = None;

            for skip in (0..expected_total).step_by(PAGE_LIMIT as usize) {
                let payload =
                    self.build_events_payload(sport.sport_id, payload_namespace, skip as u32);
                let json = match self.fetch_json(EVENTS_LIST_URL, endpoint, &payload).await {
                    Ok(json) => json,
                    Err(error) => {
                        candidate_error = Some(error);
                        break;
                    }
                };
                let (batch_events, batch_odds) = Self::parse_response(&json, endpoint.route_hint);

                if batch_events.is_empty() {
                    if skip == 0 {
                        candidate_error = Some(Self::branch_error(
                            "eventsList",
                            format!(
                                "sport {} ({}) namespace {} via payload ns {} returned no parsable events: {}",
                                sport.sport_name,
                                sport.sport_id,
                                namespace,
                                payload_namespace,
                                Self::describe_events_list_payload(&json)
                            ),
                        ));
                    }
                    break;
                }

                let batch_len = batch_events.len();
                events.extend(batch_events);
                odds.extend(batch_odds);

                if batch_len < PAGE_LIMIT as usize {
                    break;
                }
            }

            if !events.is_empty() {
                if payload_namespace != namespace {
                    info!(
                        sport_id = sport.sport_id,
                        sport_name = %sport.sport_name,
                        namespace,
                        payload_namespace,
                        events = events.len(),
                        "Liga Stavok namespace fallback succeeded"
                    );
                }
                return Ok((events, odds));
            }

            last_error = candidate_error;
        }

        Err(last_error.unwrap_or_else(|| {
            Self::branch_error(
                "eventsList",
                format!(
                    "sport {} ({}) namespace {} produced no events across payload namespaces {}",
                    sport.sport_name,
                    sport.sport_id,
                    namespace,
                    Self::namespace_payload_candidates(namespace).join(",")
                ),
            )
        }))
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let mut events_by_id = HashMap::new();
        let mut odds_by_id = HashMap::new();
        let mut errors = Vec::new();

        let sports = self.fetch_sport_catalog().await.map_err(|error| {
            Self::branch_error("runtime", format!("catalog bootstrap failed: {error}"))
        })?;
        for sport in &sports {
            for namespace in ["prematch", "live"] {
                match self.fetch_sport_namespace(sport, namespace).await {
                    Ok((endpoint_events, endpoint_odds)) => {
                        for event in endpoint_events {
                            events_by_id.entry(event.id.clone()).or_insert(event);
                        }

                        for odd in endpoint_odds {
                            odds_by_id.entry(odd.id.clone()).or_insert(odd);
                        }
                    }
                    Err(error) => {
                        warn!(sport_id = sport.sport_id, sport_name = %sport.sport_name, namespace, %error, "Liga Stavok sport fetch failed");
                        errors.push(format!(
                            "sport={} id={} namespace={} reason={}",
                            sport.sport_name, sport.sport_id, namespace, error
                        ));
                    }
                }
            }
        }

        if events_by_id.is_empty() {
            if errors.is_empty() {
                return Err(Self::branch_error(
                    "runtime",
                    format!(
                        "all branches completed without parser-visible events after catalog bootstrap across {} sports",
                        sports.len()
                    ),
                ));
            }
            return Err(Self::branch_error(
                "runtime",
                format!(
                    "all sport branches refused runtime extraction after catalog bootstrap: {}",
                    errors.join("; ")
                ),
            ));
        }

        Ok((
            events_by_id.into_values().collect(),
            odds_by_id.into_values().collect(),
        ))
    }

    fn request_id() -> String {
        format!("ls-{:016x}", rand::thread_rng().gen::<u64>())
    }

    fn readiness_snapshot() -> ParserReadiness {
        ParserReadiness {
            stage: ParserReadinessStage::DiagnosticOnly,
            production_enabled: false,
            self_check_available: true,
            checks: vec![
                ParserDiagnosticCheck {
                    code: "events_list_pagination_configured".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!("Sport-scoped POST pagination is configured for {EVENTS_LIST_URL}."),
                },
                ParserDiagnosticCheck {
                    code: "filter_preflight_configured".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: format!("Lightweight live-sport preflight is configured via {FILTER_URL} before tournamentTree/eventsList.").to_string(),
                },
                ParserDiagnosticCheck {
                    code: "preflight_branch_diagnostics_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Warm-up refusal paths explicitly report whether cookie/storage bootstrap is available before any API probe fallback is attempted.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "filter_branch_diagnostics_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Filter preflight failures distinguish empty result sets, schema drift, and transport/protection errors without promoting the branch to a bypass mechanism.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "tournament_tree_catalog_configured".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: format!("Sport discovery is configured via {TOURNAMENT_TREE_URL}.").to_string(),
                },
                ParserDiagnosticCheck {
                    code: "tournament_tree_branch_diagnostics_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Tournament-tree failures now report payload-shape vs zero-total catalog outcomes and whether filter preflight contributed any live-sport hints.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "schema_parser_present".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "eventsList payload parser extracts events, markets, and totals from the production schema.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "events_list_branch_diagnostics_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "eventsList refusal messages include payload namespace, requested namespace, and schema/empty-data hints for safer runtime validation.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "cookie_bootstrap_supported".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Optional cookie bootstrap from LIGASTAVOK_COOKIE_HEADER, LIGASTAVOK_COOKIE_FILE, or ligastavok_cookies.json is supported for protected environments.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "storage_state_bootstrap_supported".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Playwright-style storage state bootstrap is parsed from LIGASTAVOK_BOOTSTRAP_FILE, LIGASTAVOK_STORAGE_STATE_FILE, ligastavok_bootstrap.json, ligastavok_storage_state.json, or the latest discovery artifact for cookies, locale, and origin alignment.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "browser_header_bootstrap_supported".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Warm-up and API probes send stable navigation/CORS headers with origin and Accept-Language derived from env, header-profile, or storage-state bootstrap.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "session_bootstrap_validation_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Warm-up fallback proceeds only when bootstrap carries at least one non-protection cookie; protection-only QRATOR cookies are surfaced as an explicit blocker instead of a silent bypass.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "session_bootstrap_blocker_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Runtime bootstrap diagnostics now classify ready, protection_only, header_only, and bootstrap_unavailable states so QRATOR/session gaps are surfaced as blockers instead of silent fallbacks.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "browser_verified_api_probe_required_for_protection_only".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Protection-only QRATOR cookies unlock runtime only when discovery captured a real browser-observed sportsbook API response; direct fetch probes are tracked separately and do not count as browser verification.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "runtime_refusal_reasons_recorded".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Runtime failures aggregate branch-specific refusal reasons so diagnostics can distinguish catalog bootstrap issues from per-sport eventsList exhaustion.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "qrator_unattended_bootstrap_unverified".to_string(),
                    severity: DiagnosticSeverity::Fail,
                    message: "Unattended QRATOR bypass is not verified on the current infra, so the parser remains out of production scan.".to_string(),
                },
                ParserDiagnosticCheck {
                    code: "production_guardrail".to_string(),
                    severity: DiagnosticSeverity::Info,
                    message: "Factory registration stays disabled until runtime diagnostics pass with stable protection bootstrap.".to_string(),
                },
            ],
        }
    }

    fn parse_tournament_tree(json: &Value) -> Vec<SportCatalogEntry> {
        json.get("result")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let sport_id = item.get("gameId").and_then(|value| value.as_u64())? as u32;
                let sport_name = item
                    .get("gameTitle")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .trim()
                    .to_string();
                let total = item
                    .get("total")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0) as usize;
                let total_live = item
                    .get("totalLive")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0) as usize;

                if sport_name.is_empty() || total == 0 {
                    return None;
                }

                Some(SportCatalogEntry {
                    sport_id,
                    sport_name,
                    total,
                    total_live,
                    filter_live_total: None,
                })
            })
            .collect()
    }

    fn parse_filter_catalog(json: &Value) -> Vec<FilterCatalogEntry> {
        json.get("result")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let sport_id = item.get("_id").and_then(|value| value.as_u64())? as u32;
                let sport_name = item
                    .get("title")
                    .or_else(|| item.get("name"))
                    .or_else(|| item.get("gameTitle"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                let total = item
                    .get("total")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0) as usize;

                if total == 0 {
                    return None;
                }

                Some(FilterCatalogEntry {
                    sport_id,
                    sport_name,
                    total,
                })
            })
            .collect()
    }

    fn merge_filter_catalog(
        mut sports: Vec<SportCatalogEntry>,
        filter_catalog: &[FilterCatalogEntry],
    ) -> Vec<SportCatalogEntry> {
        let filter_by_sport = filter_catalog
            .iter()
            .map(|entry| (entry.sport_id, entry))
            .collect::<HashMap<_, _>>();

        for sport in &mut sports {
            sport.filter_live_total = filter_by_sport
                .get(&sport.sport_id)
                .map(|entry| entry.total);
        }

        for entry in filter_catalog {
            if sports.iter().any(|sport| sport.sport_id == entry.sport_id) {
                continue;
            }

            sports.push(SportCatalogEntry {
                sport_id: entry.sport_id,
                sport_name: entry
                    .sport_name
                    .clone()
                    .unwrap_or_else(|| format!("sport-{}", entry.sport_id)),
                total: entry.total,
                total_live: entry.total,
                filter_live_total: Some(entry.total),
            });
        }

        sports
    }

    fn parse_response(json: &serde_json::Value, route_hint: &str) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        let Some(items) = json
            .get("result")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.as_array())
            .or_else(|| json.get("data").and_then(|value| value.as_array()))
        else {
            debug!(
                route_hint,
                "Liga Stavok: response schema not recognized yet"
            );
            return (events, odds);
        };

        for item in items {
            let Some((event_id, home, away)) = Self::extract_teams(item) else {
                continue;
            };

            let event_key = format!("{BOOKMAKER_SLUG}-{event_id}");
            let is_live = Self::extract_is_live(item, route_hint);

            let mut extra = HashMap::new();
            if let Some(ns) = item.get("ns").and_then(|value| value.as_str()) {
                extra.insert(
                    "namespace".to_string(),
                    serde_json::Value::String(ns.to_string()),
                );
            }
            if let Some(ext_id) = item
                .get("ids")
                .and_then(|value| value.get("extId"))
                .cloned()
            {
                extra.insert("ext_id".to_string(), ext_id);
            }

            events.push(Event {
                id: event_key.clone(),
                sport: Self::extract_sport(item),
                league: Self::extract_league(item),
                home_team: home,
                away_team: away,
                start_time: Self::extract_start_time(item),
                is_live,
                bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                raw_url: Self::extract_raw_url(item, is_live),
                extra,
            });

            Self::append_markets(&mut odds, &event_key, item, now);
        }

        (events, odds)
    }

    fn extract_teams(item: &serde_json::Value) -> Option<(String, String, String)> {
        let event_id = item.get("id")?.to_string().trim_matches('"').to_string();
        let event = item.get("event")?;

        if let Some(competitors) = event.get("competitors").and_then(|value| value.as_array()) {
            if competitors.len() >= 2 {
                let home = competitors
                    .first()?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .trim()
                    .to_string();
                let away = competitors
                    .get(1)?
                    .get("name")
                    .and_then(|value| value.as_str())?
                    .trim()
                    .to_string();
                if !home.is_empty() && !away.is_empty() {
                    return Some((event_id, home, away));
                }
            }
        }

        let home = event
            .get("team1")
            .and_then(|value| value.as_str())?
            .trim()
            .to_string();
        let away = event
            .get("team2")
            .and_then(|value| value.as_str())?
            .trim()
            .to_string();

        if home.is_empty() || away.is_empty() {
            return None;
        }

        Some((event_id, home, away))
    }

    fn extract_league(item: &serde_json::Value) -> String {
        let event = item.get("event");
        let tournament = event
            .and_then(|value| value.get("tournamentTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        let category = event
            .and_then(|value| value.get("categoryTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        let topic = event
            .and_then(|value| value.get("topicTitle"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();

        if !category.is_empty() && !tournament.is_empty() && category != tournament {
            return format!("{category}. {tournament}");
        }
        if !topic.is_empty() {
            return topic.to_string();
        }
        if !tournament.is_empty() {
            return tournament.to_string();
        }
        if !category.is_empty() {
            return category.to_string();
        }

        "Unknown".to_string()
    }

    fn extract_sport(item: &serde_json::Value) -> Sport {
        let raw = item
            .get("gameTitle")
            .and_then(|value| value.as_str())
            .or_else(|| item.get("title").and_then(|value| value.as_str()))
            .or_else(|| item.get("gameName").and_then(|value| value.as_str()))
            .unwrap_or("football");
        Sport::from_str(raw)
    }

    fn extract_is_live(item: &serde_json::Value, route_hint: &str) -> bool {
        match item.get("ns").and_then(|value| value.as_str()) {
            Some("live") => true,
            Some("line") | Some("prematch") => false,
            Some(other) => other.contains("live"),
            None => route_hint == "live",
        }
    }

    fn extract_start_time(item: &serde_json::Value) -> Option<DateTime<Utc>> {
        item.get("gameTs")
            .and_then(|value| value.as_i64())
            .and_then(|timestamp| Utc.timestamp_millis_opt(timestamp).single())
    }

    fn extract_raw_url(item: &serde_json::Value, is_live: bool) -> Option<String> {
        let game = item
            .get("gameSeoName")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())?;
        let section = if is_live { "live" } else { "line" };
        Some(format!("{BASE_URL}/{section}/{game}"))
    }

    fn append_markets(
        odds: &mut Vec<Odd>,
        event_id: &str,
        item: &serde_json::Value,
        now: chrono::DateTime<Utc>,
    ) {
        let Some(markets) = item.get("markets").and_then(|value| value.as_object()) else {
            return;
        };
        let Some(outcomes) = item.get("outcomes").and_then(|value| value.as_object()) else {
            return;
        };

        let parts = item.get("parts").and_then(|value| value.as_object());
        let mut outcomes_by_market: HashMap<String, Vec<&serde_json::Value>> = HashMap::new();

        for outcome in outcomes.values() {
            let Some(market_id) = outcome.get("marketId").and_then(|value| value.as_str()) else {
                continue;
            };
            outcomes_by_market
                .entry(market_id.to_string())
                .or_default()
                .push(outcome);
        }

        for (market_key, market_value) in markets {
            let Some(market) = market_value.as_object() else {
                continue;
            };
            if Self::is_locked_or_corrupted(market_value) {
                continue;
            }

            let Some(market_name) = market.get("title").and_then(|value| value.as_str()) else {
                continue;
            };
            let market_type = market
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let market_id = market
                .get("id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let market_label = Self::format_market_name(
                market_name,
                market.get("partId").and_then(|value| value.as_str()),
                parts,
            );

            let Some(linked_outcomes) = outcomes_by_market.get(market_key) else {
                continue;
            };

            for outcome in linked_outcomes {
                if Self::is_locked_or_corrupted(outcome) {
                    continue;
                }

                let Some(price) = outcome.get("value").and_then(Self::parse_f64) else {
                    continue;
                };
                if price <= 1.0 {
                    continue;
                }

                let selection = Self::format_selection(outcome);
                let line = outcome
                    .get("adValue")
                    .and_then(Self::parse_f64)
                    .or_else(|| outcome.get("line").and_then(Self::parse_f64));
                let outcome_id = outcome
                    .get("id")
                    .and_then(|value| value.as_i64())
                    .unwrap_or_default();

                odds.push(Odd {
                    id: format!("{event_id}-{market_id}-{outcome_id}"),
                    event_id: event_id.to_string(),
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    market: market_label.clone(),
                    selection: selection.clone(),
                    odds: price,
                    odds_type: Self::selection_to_odds_type(&selection, market_name, market_type),
                    line,
                    timestamp: now,
                });
            }
        }
    }

    fn format_market_name(
        market_name: &str,
        part_id: Option<&str>,
        parts: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> String {
        let part_title = part_id
            .and_then(|id| parts.and_then(|value| value.get(id)))
            .and_then(|value| value.get("title"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();

        if part_title.is_empty() || part_title == "Основное время" {
            return market_name.to_string();
        }

        format!("{part_title} / {market_name}")
    }

    fn format_selection(outcome: &serde_json::Value) -> String {
        let ad_title = outcome
            .get("adTitle")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        if !ad_title.is_empty() {
            return ad_title.to_string();
        }

        outcome
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .trim()
            .to_string()
    }

    fn parse_f64(value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(number) => number.as_f64(),
            serde_json::Value::String(text) => {
                let normalized = text.trim().replace(',', ".");
                normalized.parse::<f64>().ok()
            }
            _ => None,
        }
    }

    fn is_locked_or_corrupted(value: &serde_json::Value) -> bool {
        value
            .get("locked")
            .and_then(|field| field.as_bool())
            .unwrap_or(false)
            || value
                .get("corrupted")
                .and_then(|field| field.as_bool())
                .unwrap_or(false)
    }

    fn selection_to_odds_type(selection: &str, market_name: &str, market_type: &str) -> OddsType {
        let selection = selection.to_lowercase();
        let market_name = market_name.to_lowercase();
        let market_type = market_type.to_lowercase();

        if market_type == "ttl" || market_name.contains("тотал") || market_name.contains("total")
        {
            if selection.contains("бол")
                || selection.contains("больше")
                || selection.contains("over")
                || selection.contains("tb")
            {
                return OddsType::Over;
            }
            if selection.contains("мен")
                || selection.contains("меньше")
                || selection.contains("under")
                || selection.contains("tm")
                || selection.contains("less")
            {
                return OddsType::Under;
            }
        }

        if market_type == "han" || market_name.contains("фора") || market_name.contains("handicap")
        {
            return OddsType::Handicap;
        }

        match selection.as_str() {
            "1" | "п1" | "home" => OddsType::Home,
            "x" | "ничья" | "draw" => OddsType::Draw,
            "2" | "п2" | "away" => OddsType::Away,
            _ => OddsType::Custom,
        }
    }
}

#[async_trait]
impl BookmakerParser for LigaStavokParser {
    fn name(&self) -> &str {
        "Liga Stavok"
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
        info!(count = events.len(), "Liga Stavok events fetched");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        info!(count = odds.len(), "Liga Stavok odds fetched");
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
            "Liga Stavok fetch complete"
        );
        Ok(ParserResult::new(BOOKMAKER_SLUG, events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        USER_AGENT
    }
}

#[cfg(test)]
mod tests {
    use super::LigaStavokParser;

    #[test]
    fn parses_events_list_shape() {
        let payload = serde_json::json!({
            "result": {
                "data": [
                    {
                        "id": 22957768,
                        "gameTitle": "Футбол",
                        "gameSeoName": "football",
                        "gameTs": 1775365200000_i64,
                        "ns": "live",
                        "ids": { "extId": 685893 },
                        "event": {
                            "team1": "СКА Хабаровск",
                            "team2": "Волга Ульяновск",
                            "categoryTitle": "Россия",
                            "tournamentTitle": "Первая лига",
                            "competitors": [
                                { "name": "ФК СКА-Хабаровск" },
                                { "name": "Волга Ульяновск" }
                            ]
                        },
                        "parts": {
                            "main": { "title": "Основное время" }
                        },
                        "markets": {
                            "_759248971": {
                                "id": 759248971,
                                "title": "Победитель",
                                "type": "WIN",
                                "partId": "main",
                                "locked": false,
                                "corrupted": false
                            },
                            "_759248982": {
                                "id": 759248982,
                                "title": "Тотал",
                                "type": "TTL",
                                "partId": "main",
                                "locked": false,
                                "corrupted": false
                            }
                        },
                        "outcomes": {
                            "_1": {
                                "id": 1,
                                "marketId": "_759248971",
                                "title": "1",
                                "value": 3.6,
                                "locked": false,
                                "corrupted": false
                            },
                            "_x": {
                                "id": 2,
                                "marketId": "_759248971",
                                "title": "X",
                                "value": 1.57,
                                "locked": false,
                                "corrupted": false
                            },
                            "_2": {
                                "id": 3,
                                "marketId": "_759248971",
                                "title": "2",
                                "value": 5.5,
                                "locked": false,
                                "corrupted": false
                            },
                            "_over": {
                                "id": 4,
                                "marketId": "_759248982",
                                "title": "Бол",
                                "adValue": "2.50",
                                "value": 2.04,
                                "locked": false,
                                "corrupted": false
                            },
                            "_under": {
                                "id": 5,
                                "marketId": "_759248982",
                                "title": "Мен",
                                "adValue": "2.50",
                                "value": 1.66,
                                "locked": false,
                                "corrupted": false
                            }
                        }
                    }
                ]
            }
        });

        let (events, odds) = LigaStavokParser::parse_response(&payload, "live");

        assert_eq!(events.len(), 1);
        assert_eq!(odds.len(), 5);
        assert_eq!(events[0].league, "Россия. Первая лига");
        assert!(events[0].is_live);
        assert!(odds
            .iter()
            .any(|odd| odd.market == "Победитель" && odd.selection == "1"));
        assert!(odds
            .iter()
            .any(|odd| odd.market == "Тотал" && odd.line == Some(2.5)));
    }

    #[test]
    fn exposes_rollout_readiness_diagnostics() {
        let readiness = LigaStavokParser::readiness_snapshot();

        assert_eq!(
            readiness.stage,
            shared::ParserReadinessStage::DiagnosticOnly
        );
        assert!(!readiness.production_enabled);
        assert!(readiness.self_check_available);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "qrator_unattended_bootstrap_unverified"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "preflight_branch_diagnostics_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "filter_branch_diagnostics_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "tournament_tree_branch_diagnostics_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "events_list_branch_diagnostics_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "session_bootstrap_validation_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "session_bootstrap_blocker_recorded"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "browser_verified_api_probe_required_for_protection_only"));
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.code == "runtime_refusal_reasons_recorded"));
    }

    #[test]
    fn describes_branch_payload_failures_explicitly() {
        let filter_message = LigaStavokParser::describe_filter_payload(&serde_json::json!({
            "result": []
        }));
        let tournament_message =
            LigaStavokParser::describe_tournament_tree_payload(&serde_json::json!({
                "result": [{ "gameId": 33, "gameTitle": "Футбол", "total": 0 }]
            }));
        let events_message = LigaStavokParser::describe_events_list_payload(&serde_json::json!({
            "result": { "meta": { "skip": 0 } }
        }));

        assert_eq!(filter_message, "result array is empty");
        assert_eq!(
            tournament_message,
            "result array has 1 entries but none with non-zero sport totals"
        );
        assert!(events_message.contains("schema does not expose result.data or data arrays"));
        assert!(events_message.contains("result=object"));
    }

    #[test]
    fn describes_events_list_namespaces_when_payload_is_present() {
        let message = LigaStavokParser::describe_events_list_payload(&serde_json::json!({
            "result": {
                "data": [
                    { "id": 1, "ns": "live" },
                    { "id": 2, "ns": "live" },
                    { "id": 3, "ns": "prematch" }
                ]
            }
        }));

        assert_eq!(
            message,
            "result.data has 3 entries with namespaces live,live,prematch"
        );
    }

    #[test]
    fn builds_sport_scoped_payload() {
        let parser = LigaStavokParser::new(std::sync::Arc::new(reqwest::Client::new()));
        let payload = parser.build_events_payload(33, "prematch", 400);

        assert_eq!(payload["sportId"], 33);
        assert_eq!(payload["gameId"][0], 33);
        assert_eq!(payload["ns"][0], "prematch");
        assert_eq!(payload["limit"], 200);
        assert_eq!(payload["skip"], 400);
        assert_eq!(payload["lineType"], "home");
        assert_eq!(payload["method"], "standard");
        assert_eq!(payload["proposedType"], "MAINOFFER");
    }

    #[test]
    fn parses_tournament_tree_totals() {
        let payload = serde_json::json!({
            "result": [
                {
                    "gameId": 33,
                    "gameTitle": "Футбол",
                    "total": 1490,
                    "totalLive": 29
                },
                {
                    "gameId": 25,
                    "gameTitle": "Баскетбол",
                    "total": 170,
                    "totalLive": 23
                }
            ]
        });

        let sports = LigaStavokParser::parse_tournament_tree(&payload);
        assert_eq!(sports.len(), 2);
        assert_eq!(sports[0].sport_id, 33);
        assert_eq!(sports[0].sport_name, "Футбол");
        assert_eq!(sports[0].total, 1490);
        assert_eq!(sports[0].total_live, 29);
        assert_eq!(sports[0].filter_live_total, None);
        assert_eq!(sports[1].sport_id, 25);
    }

    #[test]
    fn parses_filter_preflight_totals() {
        let payload = serde_json::json!({
            "result": [
                { "_id": 33, "title": "Футбол", "total": 29, "lmt": 23 },
                { "_id": 25, "total": 23, "lmt": 23 },
                { "_id": 31, "total": 0, "lmt": 0 }
            ]
        });

        let sports = LigaStavokParser::parse_filter_catalog(&payload);
        assert_eq!(sports.len(), 2);
        assert_eq!(sports[0].sport_id, 33);
        assert_eq!(sports[0].sport_name.as_deref(), Some("Футбол"));
        assert_eq!(sports[0].total, 29);
        assert_eq!(sports[1].sport_id, 25);
        assert_eq!(sports[1].sport_name, None);
        assert_eq!(sports[1].total, 23);
    }

    #[test]
    fn merges_filter_preflight_into_tournament_tree_catalog() {
        let sports = vec![
            super::SportCatalogEntry {
                sport_id: 33,
                sport_name: "Футбол".to_string(),
                total: 1490,
                total_live: 29,
                filter_live_total: None,
            },
            super::SportCatalogEntry {
                sport_id: 25,
                sport_name: "Баскетбол".to_string(),
                total: 170,
                total_live: 23,
                filter_live_total: None,
            },
        ];
        let filter = vec![super::FilterCatalogEntry {
            sport_id: 25,
            sport_name: Some("Баскетбол".to_string()),
            total: 21,
        }];

        let merged = LigaStavokParser::merge_filter_catalog(sports, &filter);

        assert_eq!(merged[0].filter_live_total, None);
        assert_eq!(merged[1].filter_live_total, Some(21));
        assert_eq!(merged[1].total_live, 23);
    }

    #[test]
    fn adds_live_only_sport_from_filter_catalog_when_tree_misses_it() {
        let sports = vec![super::SportCatalogEntry {
            sport_id: 33,
            sport_name: "Футбол".to_string(),
            total: 1490,
            total_live: 29,
            filter_live_total: None,
        }];
        let filter = vec![super::FilterCatalogEntry {
            sport_id: 25,
            sport_name: Some("Баскетбол".to_string()),
            total: 21,
        }];

        let merged = LigaStavokParser::merge_filter_catalog(sports, &filter);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].sport_id, 25);
        assert_eq!(merged[1].sport_name, "Баскетбол");
        assert_eq!(merged[1].total, 21);
        assert_eq!(merged[1].total_live, 21);
        assert_eq!(merged[1].filter_live_total, Some(21));
    }

    #[test]
    fn provides_safe_namespace_fallback_for_prematch_events_list() {
        assert_eq!(
            LigaStavokParser::namespace_payload_candidates("prematch"),
            vec!["prematch", "line"]
        );
        assert_eq!(
            LigaStavokParser::namespace_payload_candidates("live"),
            vec!["live"]
        );
    }

    #[test]
    fn supports_storage_state_cookie_shape() {
        let payload = serde_json::json!({
            "cookies": [
                {
                    "name": "qrator_jsr",
                    "value": "abc",
                    "domain": ".ligastavok.ru",
                    "path": "/"
                }
            ]
        });

        let cookies = LigaStavokParser::extract_cookie_values(&payload).expect("cookies");
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0]["name"], "qrator_jsr");
    }

    #[test]
    fn supports_nested_storage_state_cookie_shape() {
        let payload = serde_json::json!({
            "storageState": {
                "cookies": [
                    {
                        "name": "sessionid",
                        "value": "safe",
                        "domain": ".ligastavok.ru",
                        "path": "/"
                    }
                ]
            }
        });

        let cookies = LigaStavokParser::extract_cookie_values(&payload).expect("cookies");
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0]["name"], "sessionid");
    }

    #[test]
    fn parses_storage_state_bootstrap_profile() {
        let payload = serde_json::json!({
            "cookies": [
                {
                    "name": "qrator_jsr",
                    "value": "abc",
                    "domain": ".ligastavok.ru",
                    "path": "/"
                },
                {
                    "name": "other",
                    "value": "skip",
                    "domain": ".example.com",
                    "path": "/"
                }
            ],
            "origins": [
                {
                    "origin": "https://www.ligastavok.ru",
                    "localStorage": [
                        { "name": "i18nextLng", "value": "ru-RU" }
                    ]
                }
            ]
        });

        let bootstrap =
            LigaStavokParser::extract_storage_state_bootstrap(&payload).expect("bootstrap");

        assert_eq!(bootstrap.cookie_header.as_deref(), Some("qrator_jsr=abc"));
        assert_eq!(
            bootstrap.accept_language.as_deref(),
            Some("ru-RU,ru;q=0.9,en;q=0.8")
        );
        assert_eq!(
            bootstrap.origin.as_deref(),
            Some("https://www.ligastavok.ru")
        );
        assert_eq!(
            bootstrap.referer.as_deref(),
            Some("https://www.ligastavok.ru/")
        );
    }

    #[test]
    fn extracts_safe_header_profile() {
        let payload = serde_json::json!({
            "extraHTTPHeaders": {
                "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.8",
                "Origin": "https://www.ligastavok.ru/",
                "Referer": "https://www.ligastavok.ru/live/football"
            }
        });

        let profile = LigaStavokParser::extract_header_profile(&payload).expect("profile");
        assert_eq!(
            profile.accept_language.as_deref(),
            Some("ru-RU,ru;q=0.9,en;q=0.8")
        );
        assert_eq!(profile.origin.as_deref(), Some("https://www.ligastavok.ru"));
        assert_eq!(
            profile.referer.as_deref(),
            Some("https://www.ligastavok.ru/live/football")
        );
    }

    #[test]
    fn extracts_header_profile_from_bootstrap_bundle_shape() {
        let payload = serde_json::json!({
            "headerProfile": {
                "accept_language": "ru-RU,ru;q=0.9,en;q=0.8",
                "origin": "https://www.ligastavok.ru",
                "referer": "https://www.ligastavok.ru/live/football"
            }
        });

        let profile = LigaStavokParser::extract_header_profile(&payload).expect("profile");
        assert_eq!(
            profile.accept_language.as_deref(),
            Some("ru-RU,ru;q=0.9,en;q=0.8")
        );
        assert_eq!(profile.origin.as_deref(), Some("https://www.ligastavok.ru"));
        assert_eq!(
            profile.referer.as_deref(),
            Some("https://www.ligastavok.ru/live/football")
        );
    }

    #[test]
    fn extracts_api_header_profile_and_probe_status() {
        let payload = serde_json::json!({
            "headerProfile": {
                "accept_language": "ru-RU,ru;q=0.9,en;q=0.8",
                "origin": "https://www.ligastavok.ru",
                "referer": "https://www.ligastavok.ru/live/football",
                "browser_verified_api_probe": {
                    "endpoint_kind": "events_list",
                    "status": 204
                },
                "direct_probe_status": 200,
                "api_headers": {
                    "Accept-Language": "ru-RU",
                    "Origin": "https://www.ligastavok.ru",
                    "Referer": "https://www.ligastavok.ru/",
                    "X-Application-Name": "mobile"
                }
            }
        });

        let profile = LigaStavokParser::extract_header_profile(&payload).expect("profile");

        assert_eq!(profile.api_accept_language.as_deref(), Some("ru-RU"));
        assert_eq!(
            profile.api_origin.as_deref(),
            Some("https://www.ligastavok.ru")
        );
        assert_eq!(
            profile.api_referer.as_deref(),
            Some("https://www.ligastavok.ru/")
        );
        assert_eq!(profile.browser_verified_api_probe_status, Some(204));
        assert_eq!(profile.direct_probe_status, Some(200));
    }

    #[test]
    fn extracts_browser_verified_probe_status_from_runtime_bootstrap_shape() {
        let payload = serde_json::json!({
            "runtimeBootstrap": {
                "browser_verified_api_probe": {
                    "endpoint_kind": "filter",
                    "status": 202
                },
                "direct_probe_status": 0
            }
        });

        let profile = LigaStavokParser::extract_header_profile(&payload).expect("profile");

        assert_eq!(profile.browser_verified_api_probe_status, Some(202));
        assert_eq!(profile.direct_probe_status, None);
    }

    #[test]
    fn parses_bootstrap_bundle_storage_state_profile() {
        let payload = serde_json::json!({
            "cookieHeader": "qrator_jsr=abc; sessionid=live",
            "final_url": "https://www.ligastavok.ru/live/football",
            "storageState": {
                "cookies": [
                    {
                        "name": "qrator_jsr",
                        "value": "abc",
                        "domain": ".ligastavok.ru",
                        "path": "/"
                    },
                    {
                        "name": "sessionid",
                        "value": "live",
                        "domain": ".ligastavok.ru",
                        "path": "/"
                    }
                ]
            },
            "headerProfile": {
                "accept_language": "ru-RU,ru;q=0.9,en;q=0.8",
                "origin": "https://www.ligastavok.ru"
            }
        });

        let bootstrap =
            LigaStavokParser::extract_storage_state_bootstrap(&payload).expect("bootstrap");

        assert_eq!(
            bootstrap.cookie_header.as_deref(),
            Some("qrator_jsr=abc; sessionid=live")
        );
        assert_eq!(
            bootstrap.accept_language.as_deref(),
            Some("ru-RU,ru;q=0.9,en;q=0.8")
        );
        assert_eq!(
            bootstrap.origin.as_deref(),
            Some("https://www.ligastavok.ru")
        );
        assert_eq!(
            bootstrap.referer.as_deref(),
            Some("https://www.ligastavok.ru/live/football")
        );
    }

    #[test]
    fn parses_raw_discovery_artifact_storage_state_profile() {
        let payload = serde_json::json!({
            "cookies": [
                {
                    "name": "sessionid",
                    "value": "live",
                    "domain": ".ligastavok.ru",
                    "path": "/"
                }
            ],
            "final_url": "https://www.ligastavok.ru/line/football"
        });

        let bootstrap =
            LigaStavokParser::extract_storage_state_bootstrap(&payload).expect("bootstrap");

        assert_eq!(bootstrap.cookie_header.as_deref(), Some("sessionid=live"));
        assert_eq!(
            bootstrap.origin.as_deref(),
            Some("https://www.ligastavok.ru")
        );
        assert_eq!(
            bootstrap.referer.as_deref(),
            Some("https://www.ligastavok.ru/line/football")
        );
    }

    #[test]
    fn drops_expired_cookies_from_cookie_header() {
        let payload = serde_json::json!([
            {
                "name": "expired",
                "value": "1",
                "domain": ".ligastavok.ru",
                "path": "/",
                "expires": 1
            },
            {
                "name": "alive",
                "value": "2",
                "domain": ".ligastavok.ru",
                "path": "/"
            }
        ]);

        let cookies = LigaStavokParser::extract_cookie_values(&payload).expect("cookies");
        let header = LigaStavokParser::build_cookie_header(&cookies).expect("header");
        assert_eq!(header, "alive=2");
    }

    #[test]
    fn filters_cookie_domains_to_ligastavok() {
        assert!(LigaStavokParser::cookie_matches_target(
            ".ligastavok.ru",
            "/"
        ));
        assert!(LigaStavokParser::cookie_matches_target(
            "lds-api-sites.ligastavok.ru",
            "/rest"
        ));
        assert!(!LigaStavokParser::cookie_matches_target("example.com", "/"));
    }

    #[test]
    fn builds_cookie_header_for_matching_url_only() {
        let payload = serde_json::json!([
            {
                "name": "root",
                "value": "1",
                "domain": ".ligastavok.ru",
                "path": "/"
            },
            {
                "name": "api_only",
                "value": "2",
                "domain": "lds-api-sites.ligastavok.ru",
                "path": "/rest"
            },
            {
                "name": "host_only",
                "value": "3",
                "domain": "www.ligastavok.ru",
                "path": "/",
                "hostOnly": true
            }
        ]);

        let cookies = LigaStavokParser::extract_cookie_values(&payload).expect("cookies");
        let jar = LigaStavokParser::parse_bootstrap_cookies(&cookies);

        let api_header = LigaStavokParser::build_cookie_header_for_url(
            &jar,
            "https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList",
        )
        .expect("api header");
        let page_header = LigaStavokParser::build_cookie_header_for_url(
            &jar,
            "https://www.ligastavok.ru/live/football",
        )
        .expect("page header");

        assert_eq!(api_header, "api_only=2; root=1");
        assert_eq!(page_header, "host_only=3; root=1");
    }

    #[test]
    fn merges_manual_cookie_header_without_overwriting_existing_names() {
        let merged = LigaStavokParser::merge_cookie_headers(
            Some("session=manual; locale=ru".to_string()),
            Some("session=jar; qrator_jsr=abc".to_string()),
        )
        .expect("merged");

        assert_eq!(merged, "session=manual; locale=ru; qrator_jsr=abc");
    }

    #[test]
    fn session_bootstrap_validation_rejects_protection_only_cookies() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![super::BootstrapCookie {
                    name: "qrator_jsr".to_string(),
                    value: "abc".to_string(),
                    domain: ".ligastavok.ru".to_string(),
                    path: "/".to_string(),
                    expires: None,
                    secure: true,
                    host_only: false,
                }],
                cookie_header: Some("qrator_jsr=abc".to_string()),
                accept_language: super::DEFAULT_ACCEPT_LANGUAGE.to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: None,
                api_origin: None,
                api_referer: None,
                browser_verified_api_probe_status: None,
                direct_probe_status: None,
            },
        };

        assert!(parser.has_cookie_bootstrap());
        assert!(!parser.has_validated_session_bootstrap());
        assert_eq!(
            parser.session_bootstrap_blocker().label(),
            "protection_only_unverified_api"
        );
        assert!(parser
            .session_bootstrap_summary()
            .contains("bootstrap_blocker=protection_only_unverified_api"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("validated_session_bootstrap=false"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("protection_only=true"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("cookie_names=qrator_jsr"));
    }

    #[test]
    fn session_bootstrap_validation_marks_header_only_bootstrap() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![],
                cookie_header: None,
                accept_language: "ru-RU,ru;q=0.9,en;q=0.7".to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: None,
                api_origin: None,
                api_referer: None,
                browser_verified_api_probe_status: None,
                direct_probe_status: None,
            },
        };

        assert_eq!(parser.session_bootstrap_blocker().label(), "header_only");
        assert!(!parser.has_validated_session_bootstrap());
        assert!(parser
            .session_bootstrap_summary()
            .contains("bootstrap_blocker=header_only"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("validated_session_bootstrap=false"));
    }

    #[test]
    fn session_bootstrap_validation_marks_missing_bootstrap() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![],
                cookie_header: None,
                accept_language: super::DEFAULT_ACCEPT_LANGUAGE.to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: None,
                api_origin: None,
                api_referer: None,
                browser_verified_api_probe_status: None,
                direct_probe_status: None,
            },
        };

        assert_eq!(
            parser.session_bootstrap_blocker().label(),
            "bootstrap_unavailable"
        );
        assert!(!parser.has_validated_session_bootstrap());
        assert!(parser
            .session_bootstrap_summary()
            .contains("bootstrap_blocker=bootstrap_unavailable"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("validated_session_bootstrap=false"));
    }

    #[test]
    fn session_bootstrap_validation_accepts_non_protection_cookie() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![],
                cookie_header: Some("qrator_jsr=abc; sessionid=live".to_string()),
                accept_language: super::DEFAULT_ACCEPT_LANGUAGE.to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: None,
                api_origin: None,
                api_referer: None,
                browser_verified_api_probe_status: None,
                direct_probe_status: None,
            },
        };

        assert!(parser.has_validated_session_bootstrap());
        assert_eq!(parser.session_bootstrap_blocker().label(), "ready");
        assert!(parser
            .session_bootstrap_summary()
            .contains("bootstrap_blocker=ready"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("validated_session_bootstrap=true"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("non_protection_cookie_count=1"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("cookie_names=qrator_jsr|sessionid"));
    }

    #[test]
    fn direct_probe_does_not_count_as_browser_verified_runtime_proof() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![super::BootstrapCookie {
                    name: "qrator_jsr".to_string(),
                    value: "abc".to_string(),
                    domain: ".ligastavok.ru".to_string(),
                    path: "/".to_string(),
                    expires: None,
                    secure: true,
                    host_only: false,
                }],
                cookie_header: Some("qrator_jsr=abc".to_string()),
                accept_language: super::DEFAULT_ACCEPT_LANGUAGE.to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: Some("ru-RU".to_string()),
                api_origin: Some(super::BASE_URL.to_string()),
                api_referer: Some(super::ROOT_REFERER.to_string()),
                browser_verified_api_probe_status: None,
                direct_probe_status: Some(200),
            },
        };

        assert!(!parser.has_validated_session_bootstrap());
        assert!(!parser.has_browser_verified_api_probe());
        assert!(parser.has_direct_probe_success());
        assert!(!parser.can_attempt_runtime_with_bootstrap());
        assert_eq!(
            parser.session_bootstrap_blocker().label(),
            "protection_only_unverified_api"
        );
        assert!(parser
            .session_bootstrap_summary()
            .contains("browser_verified_api_probe=false"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("direct_probe_success=true"));
    }

    #[test]
    fn browser_verified_probe_allows_runtime_attempt_with_protection_cookies() {
        let parser = LigaStavokParser {
            client: std::sync::Arc::new(reqwest::Client::new()),
            endpoints: vec![],
            bootstrap: super::SessionBootstrap {
                cookie_jar: vec![super::BootstrapCookie {
                    name: "qrator_jsr".to_string(),
                    value: "abc".to_string(),
                    domain: ".ligastavok.ru".to_string(),
                    path: "/".to_string(),
                    expires: None,
                    secure: true,
                    host_only: false,
                }],
                cookie_header: Some("qrator_jsr=abc".to_string()),
                accept_language: super::DEFAULT_ACCEPT_LANGUAGE.to_string(),
                origin: super::BASE_URL.to_string(),
                referer: super::ROOT_REFERER.to_string(),
                api_accept_language: Some("ru-RU".to_string()),
                api_origin: Some(super::BASE_URL.to_string()),
                api_referer: Some(super::ROOT_REFERER.to_string()),
                browser_verified_api_probe_status: Some(204),
                direct_probe_status: Some(200),
            },
        };

        assert!(!parser.has_validated_session_bootstrap());
        assert!(parser.has_browser_verified_api_probe());
        assert!(parser.can_attempt_runtime_with_bootstrap());
        assert!(parser
            .session_bootstrap_summary()
            .contains("browser_verified_api_probe=true"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("browser_verified_api_probe_status=204"));
        assert!(parser
            .session_bootstrap_summary()
            .contains("direct_probe_status=200"));
    }

    #[test]
    fn extracts_probe_status_from_discovery_status_shape() {
        let payload = serde_json::json!({
            "status": {
                "bootstrap_blocker": "protection_only_unverified_api",
                "browser_verified_api_probe_status": 204,
                "direct_probe_status": 200,
                "can_attempt_runtime_with_bootstrap": true
            }
        });

        let profile = LigaStavokParser::extract_header_profile(&payload).expect("profile");

        assert_eq!(profile.browser_verified_api_probe_status, Some(204));
        assert_eq!(profile.direct_probe_status, Some(200));
    }
}
