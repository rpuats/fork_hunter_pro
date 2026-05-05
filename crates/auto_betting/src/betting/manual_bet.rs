//! Manual Bet - Preparation only, operator does the rest

use super::{BetInstruction, BetResult, BettingError};
use crate::auth::SessionCookies;
use playwright::api::Browser;

/// Manual bet - only prepare coupon, don't place
pub async fn prepare_manual_bet(
    _instruction: &BetInstruction,
    _session: &SessionCookies,
    _browser: &Browser,
) -> Result<BetResult, BettingError> {
    // STUB: Temporarily disabled
    Err(BettingError::BrowserError("Manual bet not yet implemented".to_string()))
}

/// Build event URL for bookmaker
fn _build_event_url(_bookmaker_id: &str, _event_id: &str) -> String {
    String::new()
}
