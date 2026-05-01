//! Semi-Auto Bet - Automated preparation, operator confirmation

use super::{BetInstruction, BetResult, BetStatus, BettingError};
use crate::auth::SessionCookies;
use anyhow::Result;
use playwright::api::Browser;
use rust_decimal::Decimal;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Semi-auto bet with operator confirmation
pub async fn place_semi_auto_bet(
    instruction: &BetInstruction,
    session: &SessionCookies,
    browser: &Browser,
    operator_tx: mpsc::Sender<OperatorEvent>,
    operator_rx: &mut mpsc::Receiver<OperatorResponse>,
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

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 4: Check current odds
    let current_odds = scrape_coupon_odds(&page, &instruction.bookmaker_id)
        .await
        .map_err(|e| BettingError::BrowserError(e))?;

    // Step 5: Take screenshot of prepared coupon
    let coupon_element = page
        .wait_for_selector_with_timeout(".bet-coupon, .betslip, .coupon", 3000)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Coupon not found: {}", e)))?;

    let screenshot = coupon_element
        .screenshot()
        .await
        .ok();

    let coupon_html = coupon_element
        .inner_html()
        .await
        .unwrap_or_default();

    // Step 6: Send to operator for confirmation
    operator_tx
        .send(OperatorEvent::BetAwaitingConfirmation {
            bet_id: instruction.id.clone(),
            fork_id: instruction.fork_id,
            bookmaker: instruction.bookmaker_id.clone(),
            event: instruction.event_name.clone(),
            market: instruction.market.clone(),
            selection: instruction.selection.clone(),
            odds: current_odds,
            stake: instruction.stake,
            expected_odds: instruction.odds,
            screenshot: screenshot.clone(),
            coupon_html,
            expires_at: instruction.expires_at,
        })
        .await
        .map_err(|_| BettingError::BrowserError("Failed to send to operator".to_string()))?;

    // Step 7: Wait for operator response (60 seconds)
    let response = timeout(Duration::from_secs(60), operator_rx.recv())
        .await
        .map_err(|_| BettingError::Timeout { operation: "operator confirmation".to_string() })?
        .ok_or_else(|| BettingError::OperatorCancelled)?;

    match response {
        OperatorResponse::Confirm { adjusted_stake } => {
            // If stake was adjusted, update it
            if let Some(new_stake) = adjusted_stake {
                fill_stake(&page, &instruction.bookmaker_id, &new_stake).await?;
                tokio::time::sleep(Duration::from_millis(300)).await;
            }

            // Place the bet
            place_bet_click(&page, &instruction.bookmaker_id).await?;

            // Wait for confirmation
            let result = timeout(
                Duration::from_secs(10),
                wait_for_confirmation(&page, &instruction.bookmaker_id)
            ).await
            .map_err(|_| BettingError::Timeout { operation: "bet confirmation".to_string() })?
            .map_err(|e| BettingError::BrowserError(e))?;

            Ok(BetResult {
                bet_id: instruction.id.clone(),
                status: BetStatus::Placed,
                external_bet_id: result.external_id,
                actual_odds: Some(current_odds),
                error: None,
                screenshot: result.screenshot,
                placed_at: Some(chrono::Utc::now()),
            })
        }
        OperatorResponse::Reject => {
            // Clear coupon
            clear_coupon(&page, &instruction.bookmaker_id).await.ok();

            Ok(BetResult {
                bet_id: instruction.id.clone(),
                status: BetStatus::Rejected,
                external_bet_id: None,
                actual_odds: None,
                error: Some("Operator rejected".to_string()),
                screenshot: None,
                placed_at: None,
            })
        }
    }
}

/// Events sent to operator
#[derive(Debug, Clone)]
pub enum OperatorEvent {
    BetAwaitingConfirmation {
        bet_id: String,
        fork_id: uuid::Uuid,
        bookmaker: String,
        event: String,
        market: String,
        selection: String,
        odds: Decimal,
        stake: Decimal,
        expected_odds: Decimal,
        screenshot: Option<Vec<u8>>,
        coupon_html: String,
        expires_at: chrono::DateTime<chrono::Utc>,
    },
}

/// Responses from operator
#[derive(Debug, Clone)]
pub enum OperatorResponse {
    Confirm { adjusted_stake: Option<Decimal> },
    Reject,
}

/// Fill stake amount
async fn fill_stake(
    page: &playwright::api::Page,
    bookmaker_id: &str,
    stake: &Decimal,
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

/// Click place bet button
async fn place_bet_click(
    page: &playwright::api::Page,
    bookmaker_id: &str,
) -> Result<(), BettingError> {
    let selector = match bookmaker_id {
        "pari" => ".place-bet-btn, [data-testid='place-bet']",
        "fonbet" => ".place-bet, .coupon-submit",
        "marathon" => ".place-bet, [data-testid='place-bet']",
        _ => ".place-bet, .confirm-bet",
    };

    page.click(selector)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Failed to click place bet: {}", e)))?;

    Ok(())
}

/// Clear coupon
async fn clear_coupon(
    page: &playwright::api::Page,
    bookmaker_id: &str,
) -> Result<(), BettingError> {
    let selector = match bookmaker_id {
        "pari" => ".clear-coupon, .remove-all",
        "fonbet" => ".clear-coupon, .coupon-clear",
        "marathon" => ".clear-betslip, .remove-all",
        _ => ".clear, .remove-all",
    };

    page.click(selector)
        .await
        .map_err(|e| BettingError::BrowserError(format!("Failed to clear coupon: {}", e)))?;

    Ok(())
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

/// Scrape coupon odds
async fn scrape_coupon_odds(
    page: &playwright::api::Page,
    bookmaker_id: &str,
) -> Result<Decimal, String> {
    let selectors = match bookmaker_id {
        "pari" => vec![".coupon-odds", ".betslip-odds"],
        "fonbet" => vec![".coupon-odds", ".odds-value"],
        "marathon" => vec![".betslip-odds", ".coupon-odds"],
        _ => vec![".odds", ".coupon-odds"],
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

/// Confirmation result
struct ConfirmationResult {
    external_id: Option<String>,
    screenshot: Option<Vec<u8>>,
}

/// Wait for bet confirmation
async fn wait_for_confirmation(
    page: &playwright::api::Page,
    bookmaker_id: &str,
) -> Result<ConfirmationResult, String> {
    let success_selectors = match bookmaker_id {
        "pari" => vec![".bet-success", ".bet-confirmed"],
        "fonbet" => vec![".bet-placed", ".coupon-success"],
        "marathon" => vec![".bet-accepted", ".success-message"],
        _ => vec![".success", ".bet-placed"],
    };

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        for selector in &success_selectors {
            if page.is_visible(selector).await.unwrap_or(false) {
                // Take success screenshot
                let screenshot = page
                    .screenshot_builder()
                    .screenshot()
                    .await
                    .ok();

                let external_id = page
                    .eval_on_selector(selector, "el => el.dataset.betId")
                    .await
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()));

                return Ok(ConfirmationResult {
                    external_id,
                    screenshot,
                });
            }
        }
    }

    Err("Timeout waiting for confirmation".to_string())
}
