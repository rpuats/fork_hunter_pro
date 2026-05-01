//! Auto Bet - Fully automated bet placement

use super::{BetInstruction, BetResult, BetStatus, BettingError};
use crate::auth::SessionCookies;
use crate::performance::get_global_monitor;
use anyhow::Result;
use playwright::api::Browser;
use rust_decimal::Decimal;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Place bet in fully automatic mode
pub async fn place_auto_bet(
    instruction: &BetInstruction,
    session: &SessionCookies,
    browser: &Browser,
) -> Result<BetResult, BettingError> {
    let start = Instant::now();
    let context = browser
        .new_context(
            playwright::api::BrowserNewContextOptions::default()
                .user_agent(&session.user_agent)
        )
        .await
        .map_err(|e| BettingError::BrowserError(e.to_string()))?;

    // Add cookies
    for cookie in &session.cookies {
        let pw_cookie = playwright::api::Cookie {
            name: cookie.name.clone(),
            value: cookie.value.clone(),
            domain: cookie.domain.clone(),
            path: cookie.path.clone(),
            expires: cookie.expires.map(|e| e.timestamp() as f64),
            http_only: cookie.http_only,
            secure: cookie.secure,
            same_site: None,
        };
        // Context cookies would need to be set here
    }

    let page = context
        .new_page()
        .await
        .map_err(|e| BettingError::BrowserError(e.to_string()))?;

    // Step 1: Navigate to event
    let event_url = build_event_url(&instruction.bookmaker_id, &instruction.event_id);
    page.goto_builder(&event_url)
        .goto()
        .await
        .map_err(|e| BettingError::BrowserError(format!("Failed to navigate: {}", e)))?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Step 2: Find and click market
    let market_selector = find_market_selector(&instruction.bookmaker_id, &instruction.market, &instruction.selection)
        .await
        .map_err(|e| BettingError::BrowserError(e))?;

    page.click(&market_selector)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Failed to click market: {}", e)))?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 3: Fill stake
    let stake_selector = find_stake_input_selector(&instruction.bookmaker_id);
    let stake_input = page
        .wait_for_selector_with_timeout(&stake_selector, 5000)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Stake input not found: {}", e)))?;

    stake_input
        .fill(&instruction.stake.to_string())
        .await
        .map_err(|e| BettingError::BrowserError(e.to_string()))?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 4: Check current odds
    let current_odds = scrape_coupon_odds(&page, &instruction.bookmaker_id)
        .await
        .map_err(|e| BettingError::BrowserError(e))?;

    let odds_tolerance = Decimal::from_f64_retain(0.05).unwrap();
    let odds_diff = (current_odds - instruction.odds).abs() / instruction.odds;

    if odds_diff > odds_tolerance {
        return Err(BettingError::OddsChanged {
            expected: instruction.odds,
            actual: current_odds,
        });
    }

    // Step 5: Take screenshot before placing
    let screenshot = page
        .screenshot_builder()
        .screenshot()
        .await
        .ok();

    // Step 6: Place bet
    let place_selector = find_place_bet_selector(&instruction.bookmaker_id);
    page.click(&place_selector)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Failed to place bet: {}", e)))?;

    // Step 7: Wait for confirmation
    let result = timeout(
        Duration::from_secs(10),
        wait_for_bet_confirmation(&page, &instruction.bookmaker_id)
    ).await
    .map_err(|_| BettingError::Timeout { operation: "bet confirmation".to_string() })?
    .map_err(|e| BettingError::BrowserError(e))?;

    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;

    // Record performance metric
    if let Some(monitor) = get_global_monitor() {
        monitor.record("auto_bet", duration_ms, true).await;
    }

    Ok(BetResult {
        bet_id: instruction.id.clone(),
        status: BetStatus::Placed,
        external_bet_id: result.external_id,
        actual_odds: Some(current_odds),
        error: None,
        screenshot,
        placed_at: Some(chrono::Utc::now()),
    })
}

/// Build event URL for bookmaker
fn build_event_url(bookmaker_id: &str, event_id: &str) -> String {
    match bookmaker_id {
        "pari" => format!("https://www.pari.ru/live/football/{}" , event_id),
        "fonbet" => format!("https://www.fonbet.ru/live/{}" , event_id),
        "marathon" => format!("https://www.marathonbet.ru/su/live/{}" , event_id),
        _ => format!("https://www.google.com/search?q={}+{}" , bookmaker_id, event_id),
    }
}

/// Find market selector
async fn find_market_selector(
    bookmaker_id: &str,
    market: &str,
    selection: &str,
) -> Result<String, String> {
    let selector = match bookmaker_id {
        "pari" => match market {
            "1X2" => format!("[data-selection='{}']", selection),
            "total_over" => format!("[data-market='total'][data-selection='{}']", selection),
            "handicap_1" => format!("[data-market='handicap'][data-selection='{}']", selection),
            _ => format!("[data-testid='outcome-{}']", selection),
        },
        "fonbet" => format!("[data-type='{}']", selection.to_lowercase()),
        "marathon" => format!("[data-market='{}']", market),
        _ => format!("[data-selection='{}']", selection),
    };
    Ok(selector)
}

/// Find stake input selector
fn find_stake_input_selector(bookmaker_id: &str) -> String {
    match bookmaker_id {
        "pari" => ".betslip-stake input, [data-testid='stake-input']".to_string(),
        "fonbet" => ".coupon-stake input, .stake-input".to_string(),
        "marathon" => ".betslip-amount input, [data-testid='stake-input']".to_string(),
        _ => "input[type='number'], .stake-input".to_string(),
    }
}

/// Find place bet button selector
fn find_place_bet_selector(bookmaker_id: &str) -> String {
    match bookmaker_id {
        "pari" => ".place-bet-btn, [data-testid='place-bet']".to_string(),
        "fonbet" => ".place-bet, .coupon-submit".to_string(),
        "marathon" => ".place-bet, [data-testid='place-bet']".to_string(),
        _ => ".place-bet, .confirm-bet, button[type='submit']".to_string(),
    }
}

/// Scrape current odds from coupon
async fn scrape_coupon_odds(page: &playwright::api::Page, bookmaker_id: &str) -> Result<Decimal, String> {
    let selectors = match bookmaker_id {
        "pari" => vec![".coupon-odds", ".betslip-odds", "[data-testid='coupon-odds']"],
        "fonbet" => vec![".coupon-odds", ".odds-value"],
        "marathon" => vec![".betslip-odds", ".coupon-odds"],
        _ => vec![".odds", ".coupon-odds", ".current-odds"],
    };

    for selector in selectors {
        if let Ok(element) = page.wait_for_selector_with_timeout(selector, 2000).await {
            if let Ok(text) = element.text_content().await {
                let cleaned: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(odds) = cleaned.parse::<f64>() {
                    return Ok(Decimal::try_from(odds).unwrap_or(Decimal::ZERO));
                }
            }
        }
    }

    Err("Could not scrape odds".to_string())
}

/// Bet confirmation result
struct ConfirmationResult {
    external_id: Option<String>,
}

/// Wait for bet confirmation
async fn wait_for_bet_confirmation(
    page: &playwright::api::Page,
    bookmaker_id: &str,
) -> Result<ConfirmationResult, String> {
    let success_selectors = match bookmaker_id {
        "pari" => vec![".bet-success", ".bet-confirmed", "[data-testid='bet-success']"],
        "fonbet" => vec![".bet-placed", ".coupon-success"],
        "marathon" => vec![".bet-accepted", ".success-message"],
        _ => vec![".success", ".bet-placed", ".confirmed"],
    };

    let error_selectors = match bookmaker_id {
        "pari" => vec![".bet-error", ".error-message", ".rejected"],
        "fonbet" => vec![".coupon-error", ".bet-rejected"],
        "marathon" => vec![".bet-error", ".error"],
        _ => vec![".error", ".rejected", ".failed"],
    };

    // Poll for 10 seconds
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Check for success
        for selector in &success_selectors {
            if page.is_visible(selector).await.unwrap_or(false) {
                // Try to get bet ID
                let external_id = page
                    .eval_on_selector(selector, "el => el.dataset.betId || el.textContent")
                    .await
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()));

                return Ok(ConfirmationResult { external_id });
            }
        }

        // Check for error
        for selector in &error_selectors {
            if page.is_visible(selector).await.unwrap_or(false) {
                let error_text = page
                    .eval_on_selector(selector, "el => el.textContent")
                    .await
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "Unknown error".to_string());

                return Err(format!("Bet rejected: {}", error_text));
            }
        }
    }

    Err("Timeout waiting for confirmation".to_string())
}
