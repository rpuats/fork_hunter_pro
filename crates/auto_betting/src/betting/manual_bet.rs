//! Manual Bet - Preparation only, operator does the rest

use super::{BetInstruction, BetResult, BetStatus, BettingError};
use crate::auth::SessionCookies;
use anyhow::Result;
use playwright::api::Browser;
use std::time::Duration;

/// Manual bet - only prepare coupon, don't place
pub async fn prepare_manual_bet(
    instruction: &BetInstruction,
    session: &SessionCookies,
    browser: &Browser,
) -> Result<BetResult, BettingError> {
    let context = browser
        .new_context(
            playwright::api::BrowserNewContextOptions::default()
                .user_agent(&session.user_agent)
        )
        .await
        .map_err(|e| BettingError::BrowserError(e.to_string()))?;

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

    // Step 3: Fill stake in coupon
    fill_stake(&page, &instruction.bookmaker_id, &instruction.stake).await?;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Step 4: Take screenshot of prepared coupon
    let screenshot = page
        .screenshot_builder()
        .screenshot()
        .await
        .ok();

    // Return prepared result - operator must place manually
    Ok(BetResult {
        bet_id: instruction.id.clone(),
        status: BetStatus::Preparing,
        external_bet_id: None,
        actual_odds: None,
        error: None,
        screenshot,
        placed_at: None,
    })
}

/// Find market selector
async fn find_market_selector(
    bookmaker_id: &str,
    market: &str,
    selection: &str,
) -> Result<String, String> {
    let selector = match bookmaker_id {
        "pari" => format!("[data-selection='{}']", selection),
        "fonbet" => format!("[data-type='{}']", selection.to_lowercase()),
        "marathon" => format!("[data-market='{}']", market),
        _ => format!("[data-selection='{}']", selection),
    };
    Ok(selector)
}

/// Build event URL
fn build_event_url(bookmaker_id: &str, event_id: &str) -> String {
    match bookmaker_id {
        "pari" => format!("https://www.pari.ru/live/{}", event_id),
        "fonbet" => format!("https://www.fonbet.ru/live/{}", event_id),
        "marathon" => format!("https://www.marathonbet.ru/su/live/{}", event_id),
        _ => format!("https://www.google.com/search?q={}", event_id),
    }
}

/// Fill stake
async fn fill_stake(
    page: &playwright::api::Page,
    bookmaker_id: &str,
    stake: &rust_decimal::Decimal,
) -> Result<(), BettingError> {
    let selector = match bookmaker_id {
        "pari" => ".betslip-stake input, [data-testid='stake-input']",
        "fonbet" => ".coupon-stake input, .stake-input",
        "marathon" => ".betslip-amount input, [data-testid='stake-input']",
        _ => "input[type='number'], .stake-input",
    };

    let input = page
        .wait_for_selector_with_timeout(selector, 3000)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Stake input not found: {}", e)))?;

    input
        .fill(&stake.to_string())
        .await
        .map_err(|e| BettingError::BrowserError(e.to_string()))?;

    Ok(())
}
