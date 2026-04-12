use headless_chrome::Browser;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Common headless Chrome utilities for SPA bookmaker parsers
pub struct HeadlessChromeHelper {
    browser: Browser,
}

impl HeadlessChromeHelper {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let browser = Browser::default()?;
        Ok(Self { browser })
    }

    /// Navigate to URL and wait for page load
    pub fn navigate_and_wait(
        &self,
        url: &str,
        wait_ms: u64,
    ) -> Result<Arc<headless_chrome::Tab>, Box<dyn std::error::Error + Send + Sync>> {
        let tab = self.browser.new_tab()?;

        debug!(url = url, "HeadlessChrome: navigating");
        tab.navigate_to(url)?.wait_until_navigated()?;

        // Wait additional time for lazy loading
        if wait_ms > 0 {
            std::thread::sleep(Duration::from_millis(wait_ms));
        }

        debug!(url = url, "HeadlessChrome: page loaded");
        Ok(tab)
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

    /// Scroll page to trigger lazy loading
    pub fn scroll_page(
        tab: &headless_chrome::Tab,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tab.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)", false)?;
        std::thread::sleep(Duration::from_secs(1));
        tab.evaluate("window.scrollTo(0, document.body.scrollHeight)", false)?;
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }

    /// Extract all text content from page
    pub fn get_page_text(tab: &headless_chrome::Tab) -> Option<String> {
        tab.evaluate("document.body.innerText", false)
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(String::from))
    }
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
