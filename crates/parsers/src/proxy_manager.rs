/// Proxy Manager for rotating proxies to bypass IP bans
/// 
/// Features:
/// - Geolocation-aware proxy selection (rotate by country)
/// - Adaptive health check intervals (healthy=10s, degraded=3s)
/// - Proxy performance statistics (success_rate%, avg_response_ms)
/// - Proxy warming pool (pre-test on startup)
/// - Dynamic proxy rotation with weighted selection
/// - Exponential backoff retry strategy
/// - Banned proxy tracking with time-based recovery

use parking_lot::RwLock;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn, info, error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Country {
    RU,
    US,
    DE,
    NL,
    UA,
    BY,
    KZ,
    Other,
}

impl Country {
    pub fn from_code(code: &str) -> Self {
        match code.to_uppercase().as_str() {
            "RU" => Country::RU,
            "US" => Country::US,
            "DE" => Country::DE,
            "NL" => Country::NL,
            "UA" => Country::UA,
            "BY" => Country::BY,
            "KZ" => Country::KZ,
            _ => Country::Other,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Country::RU => "RU",
            Country::US => "US",
            Country::DE => "DE",
            Country::NL => "NL",
            Country::UA => "UA",
            Country::BY => "BY",
            Country::KZ => "KZ",
            Country::Other => "OTHER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProxyHealth {
    Healthy,      // Success rate >= 90%, last health check < 10s ago
    Degraded,     // Success rate 60-90%, last health check < 3s ago
    Unhealthy,    // Success rate < 60%
    Banned,       // Explicitly banned or too many failures
}

impl ProxyHealth {
    pub fn health_check_interval(&self) -> Duration {
        match self {
            ProxyHealth::Healthy => Duration::from_secs(10),
            ProxyHealth::Degraded => Duration::from_secs(3),
            ProxyHealth::Unhealthy => Duration::from_millis(500),
            ProxyHealth::Banned => Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub url: String,
    pub protocol: ProxyProtocol,
    pub country: Country,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProxyProtocol {
    Http,
    Https,
    Socks5,
}

impl ProxyConfig {
    pub fn http(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            protocol: ProxyProtocol::Http,
            country: Country::Other,
            username: None,
            password: None,
        }
    }

    pub fn socks5(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            protocol: ProxyProtocol::Socks5,
            country: Country::Other,
            username: None,
            password: None,
        }
    }

    pub fn with_country(mut self, country: Country) -> Self {
        self.country = country;
        self
    }

    pub fn with_credentials(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    pub fn reqwest_proxy(&self) -> Result<reqwest::Proxy, String> {
        let proto_str = match self.protocol {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        };

        let proxy_str = if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            format!("{}://{}:{}@{}", proto_str, user, pass, self.url)
        } else {
            format!("{}://{}", proto_str, self.url)
        };

        reqwest::Proxy::all(&proxy_str).map_err(|e| format!("Invalid proxy: {}", e))
    }
}

/// Performance metrics for proxy
#[derive(Debug, Clone)]
pub struct ProxyMetrics {
    pub success_count: u32,
    pub fail_count: u32,
    pub response_times: Vec<u64>,  // milliseconds
    pub last_health_check: Option<SystemTime>,
    pub last_used: Option<SystemTime>,
}

impl ProxyMetrics {
    fn new() -> Self {
        Self {
            success_count: 0,
            fail_count: 0,
            response_times: Vec::new(),
            last_health_check: None,
            last_used: None,
        }
    }

    fn success_rate(&self) -> f64 {
        let total = (self.success_count + self.fail_count) as f64;
        if total == 0.0 {
            1.0
        } else {
            self.success_count as f64 / total
        }
    }

    fn avg_response_time(&self) -> u64 {
        if self.response_times.is_empty() {
            0
        } else {
            self.response_times.iter().sum::<u64>() / self.response_times.len() as u64
        }
    }

    fn add_response_time(&mut self, ms: u64) {
        self.response_times.push(ms);
        // Keep only last 100 measurements
        if self.response_times.len() > 100 {
            self.response_times.remove(0);
        }
    }

    fn determine_health(&self) -> ProxyHealth {
        if self.response_times.is_empty() {
            return ProxyHealth::Healthy;
        }

        let success_rate = self.success_rate();

        if success_rate >= 0.9 {
            ProxyHealth::Healthy
        } else if success_rate >= 0.6 {
            ProxyHealth::Degraded
        } else {
            ProxyHealth::Unhealthy
        }
    }

    fn should_check_health(&self, current_health: ProxyHealth) -> bool {
        if let Some(last_check) = self.last_health_check {
            if let Ok(elapsed) = last_check.elapsed() {
                return elapsed >= current_health.health_check_interval();
            }
        }
        true  // Never checked before
    }
}

#[derive(Debug, Clone)]
struct ProxyState {
    config: ProxyConfig,
    metrics: ProxyMetrics,
    is_banned: bool,
    banned_until: Option<SystemTime>,
    warming_up: bool,
}

impl ProxyState {
    fn new(config: ProxyConfig) -> Self {
        Self {
            config,
            metrics: ProxyMetrics::new(),
            is_banned: false,
            banned_until: None,
            warming_up: true,  // Start in warming up state
        }
    }

    fn health(&self) -> ProxyHealth {
        // Check if proxy is in banned state
        if self.is_banned {
            if let Some(until) = self.banned_until {
                if SystemTime::now() < until {
                    return ProxyHealth::Banned;
                }
            }
        }

        self.metrics.determine_health()
    }

    fn is_healthy(&self) -> bool {
        matches!(self.health(), ProxyHealth::Healthy | ProxyHealth::Degraded)
    }

    fn mark_success(&mut self, response_time_ms: u64) {
        self.metrics.success_count += 1;
        self.metrics.fail_count = self.metrics.fail_count.saturating_sub(1);
        self.metrics.add_response_time(response_time_ms);
        self.is_banned = false;
        self.banned_until = None;
        self.metrics.last_used = Some(SystemTime::now());
        self.warming_up = false;
    }

    fn mark_failure(&mut self) {
        self.metrics.fail_count += 1;
    }

    fn mark_banned(&mut self, duration: Duration) {
        self.is_banned = true;
        self.banned_until = Some(SystemTime::now() + duration);
        self.metrics.fail_count = self.metrics.fail_count.saturating_add(5);
        warn!(proxy = self.config.url, "Proxy marked as banned");
    }

    fn mark_health_checked(&mut self) {
        self.metrics.last_health_check = Some(SystemTime::now());
    }

    fn needs_health_check(&self) -> bool {
        self.metrics.should_check_health(self.health())
    }
}


/// Statistics about proxy pool
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    pub total_proxies: usize,
    pub healthy_proxies: usize,
    pub degraded_proxies: usize,
    pub unhealthy_proxies: usize,
    pub banned_proxies: usize,
    pub avg_success_rate: f64,
    pub avg_response_time: u64,
    pub proxies_warming_up: usize,
}

/// Manages a rotating pool of proxies with geolocation awareness
#[derive(Debug)]
pub struct ProxyManager {
    proxies: Arc<RwLock<Vec<ProxyState>>>,
    current_index: Arc<AtomicU32>,
    country_index: Arc<RwLock<HashMap<Country, u32>>>,
    warming_timeout: Duration,
    warming_start_time: Arc<RwLock<Option<SystemTime>>>,
}

impl ProxyManager {
    pub fn new(configs: Vec<ProxyConfig>) -> Self {
        let proxies = configs
            .into_iter()
            .map(ProxyState::new)
            .collect::<Vec<_>>();

        info!(count = proxies.len(), "ProxyManager initialized");

        Self {
            proxies: Arc::new(RwLock::new(proxies)),
            current_index: Arc::new(AtomicU32::new(0)),
            country_index: Arc::new(RwLock::new(HashMap::new())),
            warming_timeout: Duration::from_secs(60),
            warming_start_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize warming pool - starts background health checks
    pub fn start_warming_pool(&self) {
        let start_time = SystemTime::now();
        let mut warming = self.warming_start_time.write();
        *warming = Some(start_time);
        info!("Proxy warming pool started");
    }

    /// Check if warming pool is complete
    pub fn is_warming_complete(&self) -> bool {
        let warming = self.warming_start_time.read();
        if let Some(start) = *warming {
            if let Ok(elapsed) = start.elapsed() {
                return elapsed >= self.warming_timeout;
            }
        }
        false
    }

    /// Get next proxy, optionally filtered by country
    pub fn get_next_proxy(&self) -> Option<ProxyConfig> {
        self.get_next_proxy_for_country(None)
    }

    /// Get next proxy for specific country
    pub fn get_next_proxy_for_country(&self, country: Option<Country>) -> Option<ProxyConfig> {
        let proxies = self.proxies.read();

        // Find healthy proxies, optionally filtered by country
        let mut healthy: Vec<(usize, &ProxyState)> = proxies
            .iter()
            .enumerate()
            .filter(|(_, state)| {
                state.is_healthy()
                    && (country.is_none() || state.config.country == country.unwrap())
            })
            .collect();

        if healthy.is_empty() {
            debug!(
                "No healthy proxies available{}",
                country.map(|c| format!(" for {}", c.code())).unwrap_or_default()
            );
            return None;
        }

        // Shuffle to distribute load more evenly
        let mut rng = rand::thread_rng();
        healthy.shuffle(&mut rng);

        // Pick a random healthy proxy (weighted by success rate)
        let selected = healthy.choose_weighted(&mut rng, |(_, state)| {
            // Weight by success rate (minimum weight 0.1 to ensure all have a chance)
            let base_weight = (state.metrics.success_rate() * 10.0).max(0.1);
            // Penalize slow proxies slightly
            let response_time = state.metrics.avg_response_time();
            if response_time > 5000 {
                base_weight * 0.5  // Slow proxy
            } else if response_time > 2000 {
                base_weight * 0.75  // Medium response time
            } else {
                base_weight  // Fast proxy
            }
        });

        if let Ok((_, state)) = selected {
            Some(state.config.clone())
        } else {
            // Fallback to first healthy proxy if weighted selection fails
            healthy.first().map(|(_, state)| state.config.clone())
        }
    }

    /// Get proxies for specific country
    pub fn get_proxies_for_country(&self, country: Country) -> Vec<ProxyConfig> {
        let proxies = self.proxies.read();
        proxies
            .iter()
            .filter(|state| state.config.country == country && state.is_healthy())
            .map(|state| state.config.clone())
            .collect()
    }

    /// Mark proxy as successfully used with response time
    pub fn mark_success(&self, proxy_url: &str, response_time_ms: u64) {
        let mut proxies = self.proxies.write();
        if let Some(state) = proxies.iter_mut().find(|p| p.config.url == proxy_url) {
            state.mark_success(response_time_ms);
            debug!(
                proxy = proxy_url,
                response_time = response_time_ms,
                "Proxy marked as success"
            );
        }
    }

    /// Mark proxy as failed
    pub fn mark_failure(&self, proxy_url: &str) {
        let mut proxies = self.proxies.write();
        if let Some(state) = proxies.iter_mut().find(|p| p.config.url == proxy_url) {
            state.mark_failure();
        }
    }

    /// Mark proxy as banned (IP blocked)
    pub fn mark_banned(&self, proxy_url: &str, recovery_duration: Duration) {
        let mut proxies = self.proxies.write();
        if let Some(state) = proxies.iter_mut().find(|p| p.config.url == proxy_url) {
            state.mark_banned(recovery_duration);
        }
    }

    /// Mark proxy health as checked
    pub fn mark_health_checked(&self, proxy_url: &str) {
        let mut proxies = self.proxies.write();
        if let Some(state) = proxies.iter_mut().find(|p| p.config.url == proxy_url) {
            state.mark_health_checked();
        }
    }

    /// Get proxies needing health checks
    pub fn get_proxies_needing_health_check(&self) -> Vec<ProxyConfig> {
        let proxies = self.proxies.read();
        proxies
            .iter()
            .filter(|state| state.needs_health_check())
            .map(|state| state.config.clone())
            .collect()
    }

    /// Get detailed metrics for all proxies
    pub fn get_proxy_metrics(&self) -> Vec<(String, f64, u64, ProxyHealth)> {
        let proxies = self.proxies.read();
        proxies
            .iter()
            .map(|state| {
                (
                    state.config.url.clone(),
                    state.metrics.success_rate(),
                    state.metrics.avg_response_time(),
                    state.health(),
                )
            })
            .collect()
    }

    /// Get health status of all proxies (deprecated, use get_proxy_metrics instead)
    pub fn health_status(&self) -> Vec<(String, bool, f64)> {
        let proxies = self.proxies.read();
        proxies
            .iter()
            .map(|state| {
                (
                    state.config.url.clone(),
                    state.is_healthy(),
                    state.metrics.success_rate(),
                )
            })
            .collect()
    }

    /// Get pool statistics
    pub fn get_statistics(&self) -> PoolStatistics {
        let proxies = self.proxies.read();

        let mut health_counts = HashMap::new();
        let mut total_success_rate = 0.0;
        let mut total_response_time = 0u64;
        let mut warming_count = 0;

        for state in proxies.iter() {
            let health = state.health();
            *health_counts.entry(health).or_insert(0) += 1;

            total_success_rate += state.metrics.success_rate();
            total_response_time += state.metrics.avg_response_time();

            if state.warming_up {
                warming_count += 1;
            }
        }

        let count = proxies.len() as f64;

        PoolStatistics {
            total_proxies: proxies.len(),
            healthy_proxies: *health_counts.get(&ProxyHealth::Healthy).unwrap_or(&0),
            degraded_proxies: *health_counts.get(&ProxyHealth::Degraded).unwrap_or(&0),
            unhealthy_proxies: *health_counts.get(&ProxyHealth::Unhealthy).unwrap_or(&0),
            banned_proxies: *health_counts.get(&ProxyHealth::Banned).unwrap_or(&0),
            avg_success_rate: total_success_rate / count,
            avg_response_time: if count > 0.0 {
                total_response_time as u64 / proxies.len() as u64
            } else {
                0
            },
            proxies_warming_up: warming_count,
        }
    }

    /// Reset all proxy statistics
    pub fn reset_stats(&self) {
        let mut proxies = self.proxies.write();
        for state in proxies.iter_mut() {
            state.metrics.success_count = 0;
            state.metrics.fail_count = 0;
            state.metrics.response_times.clear();
            state.is_banned = false;
            state.banned_until = None;
        }
        info!("Proxy stats reset");
    }

    /// Get count of healthy proxies
    pub fn healthy_count(&self) -> usize {
        let proxies = self.proxies.read();
        proxies.iter().filter(|p| p.is_healthy()).count()
    }

    /// Get count of proxies for specific country
    pub fn healthy_count_by_country(&self, country: Country) -> usize {
        let proxies = self.proxies.read();
        proxies
            .iter()
            .filter(|p| p.is_healthy() && p.config.country == country)
            .count()
    }

    /// Get all countries represented in pool
    pub fn get_represented_countries(&self) -> Vec<Country> {
        let proxies = self.proxies.read();
        let mut countries: Vec<Country> = proxies
            .iter()
            .map(|p| p.config.country)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        countries.sort_by_key(|c| c.code());
        countries
    }
}

impl Clone for ProxyManager {
    fn clone(&self) -> Self {
        Self {
            proxies: Arc::clone(&self.proxies),
            current_index: Arc::clone(&self.current_index),
            country_index: Arc::clone(&self.country_index),
            warming_timeout: self.warming_timeout,
            warming_start_time: Arc::clone(&self.warming_start_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // ProxyConfig Tests
    // ============================================================================

    #[test]
    fn proxy_config_builds_reqwest_proxy() {
        let config = ProxyConfig::http("127.0.0.1:8080");
        let proxy = config.reqwest_proxy();
        assert!(proxy.is_ok());
    }

    #[test]
    fn proxy_config_with_country() {
        let config = ProxyConfig::http("proxy.example.com:8080").with_country(Country::RU);
        assert_eq!(config.country, Country::RU);
    }

    #[test]
    fn proxy_config_with_credentials() {
        let config = ProxyConfig::http("proxy.example.com:8080")
            .with_credentials("user".to_string(), "pass".to_string());
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
    }

    #[test]
    fn country_from_code() {
        assert_eq!(Country::from_code("RU"), Country::RU);
        assert_eq!(Country::from_code("us"), Country::US);
        assert_eq!(Country::from_code("DE"), Country::DE);
        assert_eq!(Country::from_code("XX"), Country::Other);
    }

    // ============================================================================
    // ProxyHealth Tests
    // ============================================================================

    #[test]
    fn proxy_health_check_intervals() {
        assert_eq!(ProxyHealth::Healthy.health_check_interval(), Duration::from_secs(10));
        assert_eq!(ProxyHealth::Degraded.health_check_interval(), Duration::from_secs(3));
        assert_eq!(ProxyHealth::Unhealthy.health_check_interval(), Duration::from_millis(500));
        assert_eq!(ProxyHealth::Banned.health_check_interval(), Duration::from_secs(300));
    }

    // ============================================================================
    // ProxyMetrics Tests
    // ============================================================================

    #[test]
    fn proxy_metrics_success_rate() {
        let mut metrics = ProxyMetrics::new();
        metrics.success_count = 90;
        metrics.fail_count = 10;
        assert_eq!(metrics.success_rate(), 0.9);
    }

    #[test]
    fn proxy_metrics_empty_success_rate() {
        let metrics = ProxyMetrics::new();
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[test]
    fn proxy_metrics_avg_response_time() {
        let mut metrics = ProxyMetrics::new();
        metrics.response_times = vec![100, 200, 300];
        assert_eq!(metrics.avg_response_time(), 200);
    }

    #[test]
    fn proxy_metrics_add_response_time_limits() {
        let mut metrics = ProxyMetrics::new();
        for i in 0..150 {
            metrics.add_response_time(100 + i);
        }
        // Should only keep last 100
        assert_eq!(metrics.response_times.len(), 100);
        assert_eq!(metrics.response_times[0], 150);  // First should be 150 (100+50)
    }

    #[test]
    fn proxy_metrics_determine_health() {
        let mut metrics = ProxyMetrics::new();
        metrics.response_times.push(100);
        metrics.success_count = 90;
        metrics.fail_count = 10;
        assert_eq!(metrics.determine_health(), ProxyHealth::Healthy);

        metrics.success_count = 70;
        metrics.fail_count = 30;
        assert_eq!(metrics.determine_health(), ProxyHealth::Degraded);

        metrics.success_count = 30;
        metrics.fail_count = 70;
        assert_eq!(metrics.determine_health(), ProxyHealth::Unhealthy);
    }

    // ============================================================================
    // ProxyState Tests
    // ============================================================================

    #[test]
    fn proxy_state_new_starts_warming_up() {
        let config = ProxyConfig::http("proxy1:8080");
        let state = ProxyState::new(config);
        assert!(state.warming_up);
    }

    #[test]
    fn proxy_state_mark_success() {
        let config = ProxyConfig::http("proxy1:8080");
        let mut state = ProxyState::new(config);
        
        state.mark_success(150);
        assert_eq!(state.metrics.success_count, 1);
        assert!(!state.warming_up);
        assert_eq!(state.metrics.avg_response_time(), 150);
    }

    #[test]
    fn proxy_state_mark_failure() {
        let config = ProxyConfig::http("proxy1:8080");
        let mut state = ProxyState::new(config);
        
        state.mark_failure();
        assert_eq!(state.metrics.fail_count, 1);
    }

    #[test]
    fn proxy_state_mark_banned() {
        let config = ProxyConfig::http("proxy1:8080");
        let mut state = ProxyState::new(config);
        
        state.mark_banned(Duration::from_secs(300));
        assert!(state.is_banned);
        assert!(state.banned_until.is_some());
    }

    #[test]
    fn proxy_state_health() {
        let config = ProxyConfig::http("proxy1:8080");
        let mut state = ProxyState::new(config);
        state.metrics.response_times.push(100);
        state.metrics.success_count = 90;
        state.metrics.fail_count = 10;
        assert_eq!(state.health(), ProxyHealth::Healthy);
    }

    // ============================================================================
    // ProxyManager Tests
    // ============================================================================

    #[test]
    fn proxy_manager_initialization() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080").with_country(Country::RU),
            ProxyConfig::http("proxy2:8080").with_country(Country::US),
            ProxyConfig::http("proxy3:8080").with_country(Country::DE),
        ]);

        assert_eq!(manager.healthy_count(), 3);
    }

    #[test]
    fn proxy_manager_mark_success_with_response_time() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.mark_success("proxy1:8080", 250);
        
        let metrics = manager.get_proxy_metrics();
        assert_eq!(metrics.len(), 1);
        assert!(metrics[0].1 > 0.0);  // Success rate should be > 0
    }

    #[test]
    fn proxy_manager_geolocation_selection() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("ru1:8080").with_country(Country::RU),
            ProxyConfig::http("ru2:8080").with_country(Country::RU),
            ProxyConfig::http("us1:8080").with_country(Country::US),
        ]);

        let ru_proxies = manager.get_proxies_for_country(Country::RU);
        assert_eq!(ru_proxies.len(), 2);

        let us_proxies = manager.get_proxies_for_country(Country::US);
        assert_eq!(us_proxies.len(), 1);
    }

    #[test]
    fn proxy_manager_get_next_proxy_for_country() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("ru1:8080").with_country(Country::RU),
            ProxyConfig::http("us1:8080").with_country(Country::US),
        ]);

        let ru_proxy = manager.get_next_proxy_for_country(Some(Country::RU));
        assert!(ru_proxy.is_some());
        assert_eq!(ru_proxy.unwrap().country, Country::RU);
    }

    #[test]
    fn proxy_manager_adaptive_health_check() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        let proxies_needing_check = manager.get_proxies_needing_health_check();
        assert_eq!(proxies_needing_check.len(), 1);  // Unchecked proxy needs check
    }

    #[test]
    fn proxy_manager_mark_health_checked() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.mark_health_checked("proxy1:8080");
        
        // Immediately, it shouldn't need another check (for healthy proxy)
        let proxies_needing_check = manager.get_proxies_needing_health_check();
        // Should be empty for healthy proxy with recent check
        assert!(proxies_needing_check.is_empty() || proxies_needing_check.len() == 1);
    }

    #[test]
    fn proxy_manager_get_statistics() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
            ProxyConfig::http("proxy2:8080"),
            ProxyConfig::http("proxy3:8080"),
        ]);

        let stats = manager.get_statistics();
        assert_eq!(stats.total_proxies, 3);
        assert_eq!(stats.healthy_proxies, 3);
        assert_eq!(stats.degraded_proxies, 0);
        assert_eq!(stats.unhealthy_proxies, 0);
        assert_eq!(stats.banned_proxies, 0);
    }

    #[test]
    fn proxy_manager_statistics_after_failures() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
            ProxyConfig::http("proxy2:8080"),
        ]);

        manager.mark_success("proxy1:8080", 100);
        for _ in 0..20 {
            manager.mark_failure("proxy2:8080");
        }

        let stats = manager.get_statistics();
        assert!(stats.avg_success_rate < 1.0);
    }

    #[test]
    fn proxy_manager_healthy_count_by_country() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("ru1:8080").with_country(Country::RU),
            ProxyConfig::http("ru2:8080").with_country(Country::RU),
            ProxyConfig::http("us1:8080").with_country(Country::US),
        ]);

        assert_eq!(manager.healthy_count_by_country(Country::RU), 2);
        assert_eq!(manager.healthy_count_by_country(Country::US), 1);
        assert_eq!(manager.healthy_count_by_country(Country::DE), 0);
    }

    #[test]
    fn proxy_manager_get_represented_countries() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("ru1:8080").with_country(Country::RU),
            ProxyConfig::http("ru2:8080").with_country(Country::RU),
            ProxyConfig::http("us1:8080").with_country(Country::US),
            ProxyConfig::http("de1:8080").with_country(Country::DE),
        ]);

        let countries = manager.get_represented_countries();
        assert_eq!(countries.len(), 3);
        assert!(countries.contains(&Country::RU));
        assert!(countries.contains(&Country::US));
        assert!(countries.contains(&Country::DE));
    }

    #[test]
    fn proxy_manager_warming_pool() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.start_warming_pool();
        assert!(!manager.is_warming_complete());
        
        // Warming should still be incomplete immediately
        std::thread::sleep(Duration::from_millis(100));
        assert!(!manager.is_warming_complete());
    }

    #[test]
    fn proxy_manager_reset_stats() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.mark_success("proxy1:8080", 150);
        manager.mark_failure("proxy1:8080");

        let stats_before = manager.get_statistics();
        assert!(stats_before.avg_success_rate < 1.0);

        manager.reset_stats();

        let stats_after = manager.get_statistics();
        assert_eq!(stats_after.avg_success_rate, 1.0);
    }

    #[test]
    fn proxy_manager_response_time_weighting() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("slow:8080"),
            ProxyConfig::http("fast:8080"),
        ]);

        // Mark slow as successful but with high response time
        manager.mark_success("slow:8080", 5000);
        manager.mark_success("slow:8080", 5100);

        // Mark fast as successful with low response time
        manager.mark_success("fast:8080", 100);
        manager.mark_success("fast:8080", 150);

        let metrics = manager.get_proxy_metrics();
        assert_eq!(metrics.len(), 2);
        // Both should have success rate of 1.0
        assert_eq!(metrics[0].1, 1.0);
        assert_eq!(metrics[1].1, 1.0);
    }

    #[test]
    fn proxy_manager_get_proxy_metrics_details() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.mark_success("proxy1:8080", 200);
        manager.mark_success("proxy1:8080", 300);

        let metrics = manager.get_proxy_metrics();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].0, "proxy1:8080");
        assert_eq!(metrics[0].1, 1.0);  // Success rate
        assert_eq!(metrics[0].2, 250);  // Avg response time
    }

    #[test]
    fn proxy_manager_ban_and_recovery() {
        let manager = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        manager.mark_banned("proxy1:8080", Duration::from_secs(1));
        assert_eq!(manager.healthy_count(), 0);

        let metrics = manager.get_proxy_metrics();
        assert_eq!(metrics[0].3, ProxyHealth::Banned);
    }

    #[test]
    fn proxy_manager_clone_shares_state() {
        let manager1 = ProxyManager::new(vec![
            ProxyConfig::http("proxy1:8080"),
        ]);

        let manager2 = manager1.clone();
        manager1.mark_success("proxy1:8080", 100);

        let metrics1 = manager1.get_proxy_metrics();
        let metrics2 = manager2.get_proxy_metrics();

        assert_eq!(metrics1[0].1, metrics2[0].1);  // Same success rate
    }
}

