use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub scanner: ScannerConfig,
    pub database: DatabaseConfig,
    pub telegram: TelegramConfig,
    pub proxies: ProxyConfig,
    pub bookmakers: BookmakersConfig,
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
    pub request_timeout_secs: u64,
    pub max_concurrent_requests: usize,
    pub cache_ttl_secs: u64,
    pub bloom_filter_capacity: usize,
    pub bloom_filter_error_rate: f64,
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
        Self {
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
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080i64)?
            .set_default("server.ws_max_connections", 100i64)?
            .set_default("server.cors_origins", vec!["*".to_string()])?
            .set_default("scanner.scan_interval_secs", 5i64)?
            .set_default("scanner.min_profit_percent", 1.0f64)?
            .set_default("scanner.max_profit_percent", 30.0f64)?
            .set_default("scanner.parallel_parsers", 4i64)?
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
        Ok(cfg.try_deserialize::<AppConfig>()?)
    }
}
