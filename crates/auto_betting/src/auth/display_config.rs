//! Display configuration - Bookmaker-specific settings after login
//! Configures UI for optimal fork betting experience

use anyhow::{Context, Result};
use std::collections::HashMap;

/// Per-bookmaker display configuration
#[derive(Debug, Clone)]
pub struct BookmakerDisplayConfig {
    pub bookmaker_id: String,
    /// Actions to perform after successful login
    pub post_login_actions: Vec<PostLoginAction>,
    /// Cookie banners to accept
    pub cookie_accept_selectors: Vec<String>,
    /// Settings to apply
    pub settings: HashMap<String, String>,
    /// Whether this bookmaker supports professional view
    pub has_pro_view: bool,
    /// Whether animations can be disabled
    pub can_disable_animations: bool,
    /// Default odds format
    pub default_odds_format: OddsFormat,
}

#[derive(Debug, Clone)]
pub enum PostLoginAction {
    /// Navigate to a URL
    Navigate(String),
    /// Click an element
    Click(String),
    /// Set localStorage value
    SetLocalStorage { key: String, value: String },
    /// Execute JavaScript
    ExecuteJs(String),
    /// Wait for element
    WaitFor(String, u64), // selector, timeout_ms
}

#[derive(Debug, Clone, PartialEq)]
pub enum OddsFormat {
    Decimal,
    Fractional,
    American,
}

/// Get display configuration for a bookmaker
pub fn get_display_config(bookmaker_id: &str) -> BookmakerDisplayConfig {
    let configs = init_configs();
    configs.get(bookmaker_id).cloned().unwrap_or_else(|| {
        BookmakerDisplayConfig {
            bookmaker_id: bookmaker_id.to_string(),
            post_login_actions: vec![],
            cookie_accept_selectors: vec![],
            settings: HashMap::new(),
            has_pro_view: false,
            can_disable_animations: false,
            default_odds_format: OddsFormat::Decimal,
        }
    })
}

/// Apply display configuration via Playwright
pub async fn apply_display_config(
    page: &playwright::api::Page,
    config: &BookmakerDisplayConfig,
) -> Result<()> {
    // Accept cookies
    for selector in &config.cookie_accept_selectors {
        match page.click(selector).await {
            Ok(_) => tracing::info!("Accepted cookies for {}", config.bookmaker_id),
            Err(_) => tracing::debug!("No cookie banner found for {}", config.bookmaker_id),
        }
    }

    // Apply post-login actions
    for action in &config.post_login_actions {
        match action {
            PostLoginAction::Navigate(url) => {
                page.goto_builder(url)
                    .goto()
                    .await
                    .with_context(|| format!("Failed to navigate to {}", url))?;
            }
            PostLoginAction::Click(selector) => {
                page.click(selector)
                    .await
                    .with_context(|| format!("Failed to click {}", selector))?;
            }
            PostLoginAction::SetLocalStorage { key, value } => {
                let js = format!(
                    "() => {{ localStorage.setItem('{}', '{}'); }}",
                    key.replace('\\', "\\\\").replace('\'', "\\'"),
                    value.replace('\\', "\\\\").replace('\'', "\\'")
                );
                page.evaluate(js).await?;
            }
            PostLoginAction::ExecuteJs(js) => {
                page.evaluate(js.clone()).await?;
            }
            PostLoginAction::WaitFor(selector, timeout_ms) => {
                page.wait_for_selector_with_timeout(selector, *timeout_ms)
                    .await
                    .with_context(|| format!("Timeout waiting for {}", selector))?;
            }
        }
        
        // Small delay between actions
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    tracing::info!("Applied display config for {}", config.bookmaker_id);
    Ok(())
}

/// Initialize all bookmaker configs
fn init_configs() -> HashMap<String, BookmakerDisplayConfig> {
    let mut configs = HashMap::new();

    // Pari
    configs.insert(
        "pari".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "pari".to_string(),
            post_login_actions: vec![
                PostLoginAction::Navigate("/settings/odds?type=decimal".to_string()),
                PostLoginAction::SetLocalStorage {
                    key: "animations".to_string(),
                    value: "false".to_string(),
                },
                PostLoginAction::Navigate("/".to_string()),
            ],
            cookie_accept_selectors: vec![
                ".cookie-accept".to_string(),
                ".close-banner".to_string(),
                "[data-testid='cookie-accept']".to_string(),
            ],
            settings: [
                ("odds_format".to_string(), "decimal".to_string()),
                ("language".to_string(), "ru".to_string()),
                ("timezone".to_string(), "Europe/Moscow".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: true,
            can_disable_animations: true,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    // Fonbet
    configs.insert(
        "fonbet".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "fonbet".to_string(),
            post_login_actions: vec![
                PostLoginAction::SetLocalStorage {
                    key: "animations".to_string(),
                    value: "false".to_string(),
                },
                PostLoginAction::SetLocalStorage {
                    key: "quick_mode".to_string(),
                    value: "true".to_string(),
                },
                PostLoginAction::ExecuteJs(
                    r#"() => { 
                        localStorage.setItem('lineView', 'professional');
                        localStorage.setItem('oddsFormat', 'decimal');
                    }"#
                    .to_string(),
                ),
            ],
            cookie_accept_selectors: vec![
                ".agree-cookies".to_string(),
                ".cookie-banner button".to_string(),
            ],
            settings: [
                ("line_view".to_string(), "professional".to_string()),
                ("quick_mode".to_string(), "true".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: true,
            can_disable_animations: true,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    // Marathon
    configs.insert(
        "marathon".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "marathon".to_string(),
            post_login_actions: vec![
                PostLoginAction::Click(".pro-view-toggle".to_string()),
                PostLoginAction::SetLocalStorage {
                    key: "odds_format".to_string(),
                    value: "decimal".to_string(),
                },
            ],
            cookie_accept_selectors: vec![
                ".accept-cookies".to_string(),
                ".cookie-consent-accept".to_string(),
            ],
            settings: [
                ("view_mode".to_string(), "professional".to_string()),
                ("odds_format".to_string(), "decimal".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: true,
            can_disable_animations: false,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    // Leon
    configs.insert(
        "leon".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "leon".to_string(),
            post_login_actions: vec![
                PostLoginAction::Navigate("/settings".to_string()),
                PostLoginAction::Click("[data-setting='odds_decimal']".to_string()),
                PostLoginAction::Navigate("/".to_string()),
            ],
            cookie_accept_selectors: vec![
                ".cookie-agree".to_string(),
                ".accept-cookies-btn".to_string(),
            ],
            settings: [
                ("odds_format".to_string(), "decimal".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: false,
            can_disable_animations: true,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    // Winline
    configs.insert(
        "winline".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "winline".to_string(),
            post_login_actions: vec![
                PostLoginAction::SetLocalStorage {
                    key: "oddsView".to_string(),
                    value: "decimal".to_string(),
                },
                PostLoginAction::SetLocalStorage {
                    key: "animation".to_string(),
                    value: "off".to_string(),
                },
            ],
            cookie_accept_selectors: vec![
                ".cookie-accept-btn".to_string(),
            ],
            settings: [
                ("odds_format".to_string(), "decimal".to_string()),
                ("animations".to_string(), "off".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: false,
            can_disable_animations: true,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    // Zenit
    configs.insert(
        "zenit".to_string(),
        BookmakerDisplayConfig {
            bookmaker_id: "zenit".to_string(),
            post_login_actions: vec![
                PostLoginAction::ExecuteJs(
                    r#"() => {
                        localStorage.setItem('line_format', 'decimal');
                        localStorage.setItem('view_mode', 'pro');
                    }"#
                    .to_string(),
                ),
            ],
            cookie_accept_selectors: vec![
                ".cookie-consent__accept".to_string(),
            ],
            settings: [
                ("line_format".to_string(), "decimal".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            has_pro_view: true,
            can_disable_animations: false,
            default_odds_format: OddsFormat::Decimal,
        },
    );

    configs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config_existing() {
        let config = get_display_config("pari");
        assert_eq!(config.bookmaker_id, "pari");
        assert!(config.has_pro_view);
        assert_eq!(config.default_odds_format, OddsFormat::Decimal);
    }

    #[test]
    fn test_get_config_unknown() {
        let config = get_display_config("unknown_bk");
        assert_eq!(config.bookmaker_id, "unknown_bk");
        assert!(!config.has_pro_view);
    }
}
