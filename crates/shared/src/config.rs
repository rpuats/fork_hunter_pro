use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const WILDCARD_CORS_ORIGIN: &str = "*";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub profile: RuntimeProfile,
    pub features: FeatureFlags,
    pub server: ServerConfig,
    pub scanner: ScannerConfig,
    pub database: DatabaseConfig,
    pub telegram: TelegramConfig,
    pub proxies: ProxyConfig,
    pub bookmakers: BookmakersConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfile {
    #[default]
    Dev,
    Production,
}

impl RuntimeProfile {
    fn default_feature_flags(self) -> FeatureFlags {
        FeatureFlags {
            offline_synced_events_fallback: match self {
                Self::Dev => FeatureFlag::Enabled,
                Self::Production => FeatureFlag::Disabled,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureFlags {
    pub offline_synced_events_fallback: FeatureFlag,
}

impl FeatureFlags {
    pub fn offline_synced_events_fallback_enabled(&self) -> bool {
        self.offline_synced_events_fallback.is_enabled()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureFlag {
    Enabled,
    Disabled,
}

impl FeatureFlag {
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawAppConfig {
    #[serde(default)]
    profile: RuntimeProfile,
    #[serde(default)]
    features: RawFeatureFlags,
    server: ServerConfig,
    scanner: ScannerConfig,
    database: DatabaseConfig,
    telegram: TelegramConfig,
    proxies: ProxyConfig,
    bookmakers: BookmakersConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawFeatureFlags {
    offline_synced_events_fallback: Option<FeatureFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub ws_max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub scan_interval_secs: u64,
    pub min_profit_percent: f64,
    pub max_profit_percent: f64,
    pub parallel_parsers: usize,
    pub production_parallel_parsers: Option<usize>,
    pub parser_result_max_events: usize,
    pub production_parser_result_max_events: Option<usize>,
    pub parser_result_max_odds: usize,
    pub production_parser_result_max_odds: Option<usize>,
    pub request_timeout_secs: u64,
    pub max_concurrent_requests: usize,
    pub cache_ttl_secs: u64,
    pub bloom_filter_capacity: usize,
    pub bloom_filter_error_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParserResultCaps {
    pub max_events: usize,
    pub max_odds: usize,
}

impl ScannerConfig {
    pub fn parser_execution_parallelism(&self, profile: RuntimeProfile) -> usize {
        let dev_parallelism = self.parallel_parsers.max(1);

        match profile {
            RuntimeProfile::Dev => dev_parallelism,
            RuntimeProfile::Production => self
                .production_parallel_parsers
                .unwrap_or_else(|| dev_parallelism.min(2))
                .max(1),
        }
    }

    pub fn parser_execution_strict_mode(&self, profile: RuntimeProfile) -> bool {
        matches!(profile, RuntimeProfile::Production)
            && self.parser_execution_parallelism(profile) < self.parallel_parsers.max(1)
    }

    pub fn parser_result_caps(&self, profile: RuntimeProfile) -> ParserResultCaps {
        let dev_caps = ParserResultCaps {
            max_events: self.parser_result_max_events.max(1),
            max_odds: self.parser_result_max_odds.max(1),
        };

        match profile {
            RuntimeProfile::Dev => dev_caps,
            RuntimeProfile::Production => ParserResultCaps {
                max_events: self
                    .production_parser_result_max_events
                    .unwrap_or_else(|| dev_caps.max_events.min(10_000))
                    .max(1),
                max_odds: self
                    .production_parser_result_max_odds
                    .unwrap_or_else(|| dev_caps.max_odds.min(100_000))
                    .max(1),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub auto_migrate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub admin_chat_ids: Vec<i64>,
    pub notify_min_profit: f64,
    pub silent_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub proxies: Vec<String>,
    pub rotation_strategy: RotationStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationStrategy {
    RoundRobin,
    Random,
    Sticky,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakersConfig {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
    pub per_bookmaker_delay_ms: HashMap<String, u64>,
    pub per_bookmaker_timeout_secs: HashMap<String, u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let profile = RuntimeProfile::default();
        Self {
            profile,
            features: profile.default_feature_flags(),
            server: ServerConfig::default(),
            scanner: ScannerConfig::default(),
            database: DatabaseConfig::default(),
            telegram: TelegramConfig::default(),
            proxies: ProxyConfig::default(),
            bookmakers: BookmakersConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9090,
            cors_origins: vec!["*".to_string()],
            ws_max_connections: 100,
        }
    }
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            scan_interval_secs: 5,
            min_profit_percent: 0.1,
            max_profit_percent: 30.0,
            parallel_parsers: 4,
            production_parallel_parsers: None,
            parser_result_max_events: 20_000,
            production_parser_result_max_events: None,
            parser_result_max_odds: 200_000,
            production_parser_result_max_odds: None,
            request_timeout_secs: 30,
            max_concurrent_requests: 20,
            cache_ttl_secs: 60,
            bloom_filter_capacity: 100000,
            bloom_filter_error_rate: 0.01,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://data/fork_hunter.db".to_string(),
            max_connections: 5,
            auto_migrate: true,
        }
    }
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            admin_chat_ids: Vec::new(),
            notify_min_profit: 1.0,
            silent_mode: false,
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxies: Vec::new(),
            rotation_strategy: RotationStrategy::RoundRobin,
        }
    }
}

impl Default for BookmakersConfig {
    fn default() -> Self {
        Self {
            enabled: vec![
                "winline".into(),
                "pari".into(),
                "betcity".into(),
                "marathon".into(),
                "zenit".into(),
                "baltbet".into(),
                "bettery".into(),
            ],
            disabled: Vec::new(),
            per_bookmaker_delay_ms: HashMap::new(),
            per_bookmaker_timeout_secs: HashMap::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let mut builder = config::Config::builder()
            .set_default("profile", "dev")?
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080i64)?
            .set_default("server.ws_max_connections", 100i64)?
            .set_default("server.cors_origins", vec!["*".to_string()])?
            .set_default("scanner.scan_interval_secs", 5i64)?
            .set_default("scanner.min_profit_percent", 1.0f64)?
            .set_default("scanner.max_profit_percent", 30.0f64)?
            .set_default("scanner.parallel_parsers", 4i64)?
            .set_default("scanner.parser_result_max_events", 20000i64)?
            .set_default("scanner.parser_result_max_odds", 200000i64)?
            .set_default("scanner.request_timeout_secs", 30i64)?
            .set_default("scanner.max_concurrent_requests", 20i64)?
            .set_default("scanner.cache_ttl_secs", 60i64)?
            .set_default("scanner.bloom_filter_capacity", 10000i64)?
            .set_default("scanner.bloom_filter_error_rate", 0.01f64)?
            .set_default("database.url", "sqlite://data/fork_hunter.db")?
            .set_default("database.max_connections", 5i64)?
            .set_default("database.auto_migrate", true)?
            .set_default("telegram.enabled", false)?
            .set_default("telegram.token", "")?
            .set_default("telegram.bot_token", "")?
            .set_default("telegram.admin_chat_ids", Vec::<i64>::new())?
            .set_default("telegram.notify_min_profit", 1.0f64)?
            .set_default("telegram.silent_mode", false)?
            .set_default("proxies.enabled", false)?
            .set_default("proxies.proxies", Vec::<String>::new())?
            .set_default("proxies.rotation_strategy", "Random")?
            .set_default("bookmakers.enabled", Vec::<String>::new())?
            .set_default("bookmakers.disabled", Vec::<String>::new())?
            .set_default(
                "bookmakers.per_bookmaker_delay_ms",
                HashMap::<String, u64>::new(),
            )?
            .set_default(
                "bookmakers.per_bookmaker_timeout_secs",
                HashMap::<String, u64>::new(),
            )?;

        let config_path = PathBuf::from("config.yaml");
        if config_path.exists() {
            builder = builder.add_source(config::File::from(config_path).required(false));
        }

        builder = builder.add_source(config::Environment::with_prefix("FORK").separator("__"));

        let cfg = builder.build()?;
        Self::from_config(cfg)
    }

    fn from_config(cfg: config::Config) -> anyhow::Result<Self> {
        let raw = cfg.try_deserialize::<RawAppConfig>()?;
        let config = Self::from_raw(raw);
        config.validate()?;
        Ok(config)
    }

    fn from_raw(raw: RawAppConfig) -> Self {
        let mut features = raw.profile.default_feature_flags();
        if let Some(flag) = raw.features.offline_synced_events_fallback {
            features.offline_synced_events_fallback = flag;
        }

        Self {
            profile: raw.profile,
            features,
            server: raw.server,
            scanner: raw.scanner,
            database: raw.database,
            telegram: raw.telegram,
            proxies: raw.proxies,
            bookmakers: raw.bookmakers,
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.validate_server()?;
        self.validate_scanner()?;
        self.validate_database()?;
        self.validate_telegram()?;
        self.validate_proxies()?;
        self.validate_bookmakers()?;
        self.validate_profile_guardrails()?;
        Ok(())
    }

    fn validate_server(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.server.host.trim().is_empty(),
            "server.host must not be empty"
        );
        anyhow::ensure!(self.server.port > 0, "server.port must be greater than 0");
        anyhow::ensure!(
            !self.server.cors_origins.is_empty(),
            "server.cors_origins must contain at least one origin"
        );
        anyhow::ensure!(
            self.server
                .cors_origins
                .iter()
                .all(|origin| !origin.trim().is_empty()),
            "server.cors_origins must not contain empty values"
        );
        anyhow::ensure!(
            self.server.ws_max_connections > 0,
            "server.ws_max_connections must be greater than 0"
        );
        Ok(())
    }

    fn validate_scanner(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.scanner.scan_interval_secs > 0,
            "scanner.scan_interval_secs must be greater than 0"
        );
        anyhow::ensure!(
            self.scanner.min_profit_percent >= 0.0,
            "scanner.min_profit_percent must be non-negative"
        );
        anyhow::ensure!(
            self.scanner.max_profit_percent > self.scanner.min_profit_percent,
            "scanner.max_profit_percent must be greater than scanner.min_profit_percent"
        );
        anyhow::ensure!(
            self.scanner.parallel_parsers > 0,
            "scanner.parallel_parsers must be greater than 0"
        );
        if let Some(production_parallel_parsers) = self.scanner.production_parallel_parsers {
            anyhow::ensure!(
                production_parallel_parsers > 0,
                "scanner.production_parallel_parsers must be greater than 0"
            );
            anyhow::ensure!(
                production_parallel_parsers <= self.scanner.parallel_parsers,
                "scanner.production_parallel_parsers must be less than or equal to scanner.parallel_parsers"
            );
        }
        anyhow::ensure!(
            self.scanner.parser_result_max_events > 0,
            "scanner.parser_result_max_events must be greater than 0"
        );
        anyhow::ensure!(
            self.scanner.parser_result_max_odds > 0,
            "scanner.parser_result_max_odds must be greater than 0"
        );
        if let Some(production_parser_result_max_events) =
            self.scanner.production_parser_result_max_events
        {
            anyhow::ensure!(
                production_parser_result_max_events > 0,
                "scanner.production_parser_result_max_events must be greater than 0"
            );
            anyhow::ensure!(
                production_parser_result_max_events <= self.scanner.parser_result_max_events,
                "scanner.production_parser_result_max_events must be less than or equal to scanner.parser_result_max_events"
            );
        }
        if let Some(production_parser_result_max_odds) =
            self.scanner.production_parser_result_max_odds
        {
            anyhow::ensure!(
                production_parser_result_max_odds > 0,
                "scanner.production_parser_result_max_odds must be greater than 0"
            );
            anyhow::ensure!(
                production_parser_result_max_odds <= self.scanner.parser_result_max_odds,
                "scanner.production_parser_result_max_odds must be less than or equal to scanner.parser_result_max_odds"
            );
        }
        anyhow::ensure!(
            self.scanner.request_timeout_secs > 0,
            "scanner.request_timeout_secs must be greater than 0"
        );
        anyhow::ensure!(
            self.scanner.max_concurrent_requests > 0,
            "scanner.max_concurrent_requests must be greater than 0"
        );
        anyhow::ensure!(
            self.scanner.bloom_filter_capacity > 0,
            "scanner.bloom_filter_capacity must be greater than 0"
        );
        anyhow::ensure!(
            (0.0..1.0).contains(&self.scanner.bloom_filter_error_rate),
            "scanner.bloom_filter_error_rate must be between 0 and 1"
        );
        Ok(())
    }

    fn validate_database(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.database.url.trim().is_empty(),
            "database.url must not be empty"
        );
        anyhow::ensure!(
            self.database.max_connections > 0,
            "database.max_connections must be greater than 0"
        );
        Ok(())
    }

    fn validate_telegram(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.telegram.notify_min_profit >= 0.0,
            "telegram.notify_min_profit must be non-negative"
        );
        Ok(())
    }

    fn validate_proxies(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.proxies.enabled || !self.proxies.proxies.is_empty(),
            "proxies.enabled requires at least one configured proxy"
        );
        anyhow::ensure!(
            self.proxies
                .proxies
                .iter()
                .all(|proxy| !proxy.trim().is_empty()),
            "proxies.proxies must not contain empty values"
        );
        Ok(())
    }

    fn validate_bookmakers(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.bookmakers
                .enabled
                .iter()
                .all(|slug| !slug.trim().is_empty()),
            "bookmakers.enabled must not contain empty values"
        );
        anyhow::ensure!(
            self.bookmakers
                .disabled
                .iter()
                .all(|slug| !slug.trim().is_empty()),
            "bookmakers.disabled must not contain empty values"
        );

        let overlapping: Vec<&str> = self
            .bookmakers
            .enabled
            .iter()
            .map(String::as_str)
            .filter(|slug| {
                self.bookmakers
                    .disabled
                    .iter()
                    .any(|disabled| disabled == slug)
            })
            .collect();

        anyhow::ensure!(
            overlapping.is_empty(),
            "bookmakers.enabled and bookmakers.disabled overlap: {}",
            overlapping.join(", ")
        );

        for (slug, delay_ms) in &self.bookmakers.per_bookmaker_delay_ms {
            anyhow::ensure!(
                !slug.trim().is_empty(),
                "bookmakers.per_bookmaker_delay_ms contains an empty bookmaker slug"
            );
            anyhow::ensure!(
                *delay_ms > 0,
                "bookmakers.per_bookmaker_delay_ms.{slug} must be greater than 0"
            );
        }

        for (slug, timeout_secs) in &self.bookmakers.per_bookmaker_timeout_secs {
            anyhow::ensure!(
                !slug.trim().is_empty(),
                "bookmakers.per_bookmaker_timeout_secs contains an empty bookmaker slug"
            );
            anyhow::ensure!(
                *timeout_secs > 0,
                "bookmakers.per_bookmaker_timeout_secs.{slug} must be greater than 0"
            );
        }

        Ok(())
    }

    fn validate_profile_guardrails(&self) -> anyhow::Result<()> {
        if self.profile != RuntimeProfile::Production {
            return Ok(());
        }

        anyhow::ensure!(
            !self.features.offline_synced_events_fallback_enabled(),
            "features.offline_synced_events_fallback must stay disabled in production"
        );
        anyhow::ensure!(
            !self
                .server
                .cors_origins
                .iter()
                .any(|origin| origin.trim() == WILDCARD_CORS_ORIGIN),
            "server.cors_origins must not contain '*' in production"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, FeatureFlag, RuntimeProfile};

    fn base_config_builder() -> config::ConfigBuilder<config::builder::DefaultState> {
        config::Config::builder()
            .set_default("server.host", "0.0.0.0")
            .expect("server.host")
            .set_default("server.port", 8080i64)
            .expect("server.port")
            .set_default("server.ws_max_connections", 100i64)
            .expect("server.ws_max_connections")
            .set_default("server.cors_origins", vec!["*".to_string()])
            .expect("server.cors_origins")
            .set_default("scanner.scan_interval_secs", 5i64)
            .expect("scanner.scan_interval_secs")
            .set_default("scanner.min_profit_percent", 1.0f64)
            .expect("scanner.min_profit_percent")
            .set_default("scanner.max_profit_percent", 30.0f64)
            .expect("scanner.max_profit_percent")
            .set_default("scanner.parallel_parsers", 4i64)
            .expect("scanner.parallel_parsers")
            .set_default("scanner.parser_result_max_events", 20000i64)
            .expect("scanner.parser_result_max_events")
            .set_default("scanner.parser_result_max_odds", 200000i64)
            .expect("scanner.parser_result_max_odds")
            .set_default("scanner.request_timeout_secs", 30i64)
            .expect("scanner.request_timeout_secs")
            .set_default("scanner.max_concurrent_requests", 20i64)
            .expect("scanner.max_concurrent_requests")
            .set_default("scanner.cache_ttl_secs", 60i64)
            .expect("scanner.cache_ttl_secs")
            .set_default("scanner.bloom_filter_capacity", 10000i64)
            .expect("scanner.bloom_filter_capacity")
            .set_default("scanner.bloom_filter_error_rate", 0.01f64)
            .expect("scanner.bloom_filter_error_rate")
            .set_default("database.url", "sqlite://data/fork_hunter.db")
            .expect("database.url")
            .set_default("database.max_connections", 5i64)
            .expect("database.max_connections")
            .set_default("database.auto_migrate", true)
            .expect("database.auto_migrate")
            .set_default("telegram.bot_token", "")
            .expect("telegram.bot_token")
            .set_default("telegram.admin_chat_ids", Vec::<i64>::new())
            .expect("telegram.admin_chat_ids")
            .set_default("telegram.notify_min_profit", 1.0f64)
            .expect("telegram.notify_min_profit")
            .set_default("telegram.silent_mode", false)
            .expect("telegram.silent_mode")
            .set_default("proxies.enabled", false)
            .expect("proxies.enabled")
            .set_default("proxies.proxies", Vec::<String>::new())
            .expect("proxies.proxies")
            .set_default("proxies.rotation_strategy", "Random")
            .expect("proxies.rotation_strategy")
            .set_default("bookmakers.enabled", Vec::<String>::new())
            .expect("bookmakers.enabled")
            .set_default("bookmakers.disabled", Vec::<String>::new())
            .expect("bookmakers.disabled")
            .set_default(
                "bookmakers.per_bookmaker_delay_ms",
                HashMap::<String, u64>::new(),
            )
            .expect("bookmakers.per_bookmaker_delay_ms")
            .set_default(
                "bookmakers.per_bookmaker_timeout_secs",
                HashMap::<String, u64>::new(),
            )
            .expect("bookmakers.per_bookmaker_timeout_secs")
    }

    use std::collections::HashMap;

    #[test]
    fn defaults_to_dev_profile_with_synced_fallback_enabled() {
        let config = AppConfig::from_config(base_config_builder().build().expect("config build"))
            .expect("config parse");

        assert_eq!(config.profile, RuntimeProfile::Dev);
        assert_eq!(
            config.features.offline_synced_events_fallback,
            FeatureFlag::Enabled
        );
        assert!(config.features.offline_synced_events_fallback_enabled());
    }

    #[test]
    fn production_profile_disables_synced_fallback_by_default() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("profile", "production")
                .expect("profile override")
                .set_override("server.cors_origins", vec!["https://ops.example"])
                .expect("cors override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(config.profile, RuntimeProfile::Production);
        assert_eq!(
            config.features.offline_synced_events_fallback,
            FeatureFlag::Disabled
        );
        assert!(!config.features.offline_synced_events_fallback_enabled());
    }

    #[test]
    fn explicit_feature_override_wins_over_dev_profile_default() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("profile", "dev")
                .expect("profile override")
                .set_override("features.offline_synced_events_fallback", "disabled")
                .expect("feature override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(config.profile, RuntimeProfile::Dev);
        assert_eq!(
            config.features.offline_synced_events_fallback,
            FeatureFlag::Disabled
        );
        assert!(!config.features.offline_synced_events_fallback_enabled());
    }

    #[test]
    fn rejects_zero_scan_interval() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.scan_interval_secs", 0i64)
                .expect("scan_interval override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err
            .to_string()
            .contains("scanner.scan_interval_secs must be greater than 0"));
    }

    #[test]
    fn rejects_invalid_profit_range() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.min_profit_percent", 5.0f64)
                .expect("min profit override")
                .set_override("scanner.max_profit_percent", 5.0f64)
                .expect("max profit override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err.to_string().contains(
            "scanner.max_profit_percent must be greater than scanner.min_profit_percent"
        ));
    }

    #[test]
    fn production_parallelism_defaults_to_stricter_limit() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.parallel_parsers", 6i64)
                .expect("parallel_parsers override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(
            config
                .scanner
                .parser_execution_parallelism(RuntimeProfile::Dev),
            6
        );
        assert_eq!(
            config
                .scanner
                .parser_execution_parallelism(RuntimeProfile::Production),
            2
        );
        assert!(config
            .scanner
            .parser_execution_strict_mode(RuntimeProfile::Production));
    }

    #[test]
    fn explicit_production_parallelism_override_is_used() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.parallel_parsers", 5i64)
                .expect("parallel_parsers override")
                .set_override("scanner.production_parallel_parsers", 3i64)
                .expect("production_parallel_parsers override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(
            config
                .scanner
                .parser_execution_parallelism(RuntimeProfile::Production),
            3
        );
        assert!(config
            .scanner
            .parser_execution_strict_mode(RuntimeProfile::Production));
    }

    #[test]
    fn production_parser_result_caps_default_to_stricter_limits() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.parser_result_max_events", 18000i64)
                .expect("parser_result_max_events override")
                .set_override("scanner.parser_result_max_odds", 160000i64)
                .expect("parser_result_max_odds override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(
            config
                .scanner
                .parser_result_caps(RuntimeProfile::Dev)
                .max_events,
            18_000
        );
        assert_eq!(
            config
                .scanner
                .parser_result_caps(RuntimeProfile::Dev)
                .max_odds,
            160_000
        );
        assert_eq!(
            config
                .scanner
                .parser_result_caps(RuntimeProfile::Production)
                .max_events,
            10_000
        );
        assert_eq!(
            config
                .scanner
                .parser_result_caps(RuntimeProfile::Production)
                .max_odds,
            100_000
        );
    }

    #[test]
    fn rejects_looser_production_parser_result_caps() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.parser_result_max_events", 10i64)
                .expect("parser_result_max_events override")
                .set_override("scanner.production_parser_result_max_events", 11i64)
                .expect("production_parser_result_max_events override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err.to_string().contains(
            "scanner.production_parser_result_max_events must be less than or equal to scanner.parser_result_max_events"
        ));
    }

    #[test]
    fn rejects_looser_production_parallelism_override() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("scanner.parallel_parsers", 2i64)
                .expect("parallel_parsers override")
                .set_override("scanner.production_parallel_parsers", 3i64)
                .expect("production_parallel_parsers override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err.to_string().contains(
            "scanner.production_parallel_parsers must be less than or equal to scanner.parallel_parsers"
        ));
    }

    #[test]
    fn rejects_enabled_proxies_without_proxy_list() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("proxies.enabled", true)
                .expect("proxies.enabled override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err
            .to_string()
            .contains("proxies.enabled requires at least one configured proxy"));
    }

    #[test]
    fn rejects_overlapping_bookmaker_lists() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("bookmakers.enabled", vec!["pari"])
                .expect("bookmakers.enabled override")
                .set_override("bookmakers.disabled", vec!["pari"])
                .expect("bookmakers.disabled override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err
            .to_string()
            .contains("bookmakers.enabled and bookmakers.disabled overlap: pari"));
    }

    #[test]
    fn rejects_wildcard_cors_in_production() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("profile", "production")
                .expect("profile override")
                .set_override("server.cors_origins", vec!["*"])
                .expect("cors override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err
            .to_string()
            .contains("server.cors_origins must not contain '*' in production"));
    }

    #[test]
    fn rejects_synced_fallback_override_in_production() {
        let err = AppConfig::from_config(
            base_config_builder()
                .set_override("profile", "production")
                .expect("profile override")
                .set_override("features.offline_synced_events_fallback", "enabled")
                .expect("feature override")
                .set_override("server.cors_origins", vec!["https://ops.example"])
                .expect("cors override")
                .build()
                .expect("config build"),
        )
        .expect_err("config should fail validation");

        assert!(err
            .to_string()
            .contains("features.offline_synced_events_fallback must stay disabled in production"));
    }

    #[test]
    fn allows_locked_down_production_profile() {
        let config = AppConfig::from_config(
            base_config_builder()
                .set_override("profile", "production")
                .expect("profile override")
                .set_override("server.cors_origins", vec!["https://ops.example"])
                .expect("cors override")
                .build()
                .expect("config build"),
        )
        .expect("config parse");

        assert_eq!(config.profile, RuntimeProfile::Production);
        assert!(!config.features.offline_synced_events_fallback_enabled());
        assert_eq!(config.server.cors_origins, vec!["https://ops.example"]);
    }
}
