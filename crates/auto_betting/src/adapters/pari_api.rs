//! Real Pari API implementation using cookie-based session authentication
//! 
//! Flow:
//! 1. Import cookies from Playwright session storage
//! 2. Exchange cookies for API bearer token via /api/v1/auth/session
//! 3. Use token for authenticated requests (balance, bet placement)

use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const API_BASE: &str = "https://api.pari.ru";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct PariApiClient {
    client: Client,
    bearer_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuthRequest {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    success: Option<bool>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceResponse {
    #[serde(rename = "availableBalance")]
    pub available_balance: Option<f64>,
    #[serde(rename = "totalBalance")]
    pub total_balance: Option<f64>,
    #[serde(rename = "bonusBalance")]
    pub bonus_balance: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BetPlacementResponse {
    #[serde(rename = "betId")]
    pub bet_id: Option<String>,
    #[serde(rename = "ticketId")]
    pub ticket_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    #[serde(rename = "acceptedStake")]
    pub accepted_stake: Option<f64>,
    #[serde(rename = "acceptedOdds")]
    pub accepted_odds: Option<f64>,
}

#[derive(Debug, Serialize)]
struct BetRequest {
    #[serde(rename = "eventId")]
    event_id: String,
    market: String,
    selection: String,
    odds: f64,
    stake: f64,
    #[serde(rename = "isLive")]
    is_live: bool,
}

impl PariApiClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            bearer_token: None,
        }
    }

    /// Create client with pre-authenticated bearer token
    pub fn with_token(token: String) -> Self {
        let mut client = Self::new();
        client.bearer_token = Some(token);
        client
    }

    /// Authenticate using session cookie from browser storage
    pub async fn authenticate_with_cookie(&mut self, cookie_header: &str) -> Result<String, String> {
        info!("Authenticating with Pari API using session cookie");

        // Parse session ID from cookie header
        let session_id = self.extract_session_id(cookie_header);
        
        if session_id.is_none() {
            warn!("No session ID found in cookies, trying full cookie exchange");
        }

        // Build auth request
        let auth_req = AuthRequest { session_id };

        // Make auth request with cookies
        let mut request = self.client
            .post(format!("{}/api/v1/auth/session", API_BASE))
            .json(&auth_req);

        // Add cookie header if present
        if !cookie_header.is_empty() {
            request = request.header(header::COOKIE, cookie_header);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Auth request failed: {}", e))?;

        let status = response.status();
        let body = response
            .json::<AuthResponse>()
            .await
            .map_err(|e| format!("Failed to parse auth response: {}", e))?;

        if !status.is_success() {
            return Err(format!("Auth failed with status {}: {:?}", status, body.error));
        }

        let token = body
            .access_token
            .ok_or_else(|| "No access token in auth response".to_string())?;

        self.bearer_token = Some(token.clone());
        info!("Successfully authenticated with Pari API");

        Ok(token)
    }

    /// Refresh bearer token using refresh token
    pub async fn refresh_token(&mut self, refresh_token: &str) -> Result<String, String> {
        let response = self.client
            .post(format!("{}/api/v1/auth/refresh", API_BASE))
            .json(&serde_json::json!({ "refreshToken": refresh_token }))
            .send()
            .await
            .map_err(|e| format!("Token refresh failed: {}", e))?;

        let body = response
            .json::<AuthResponse>()
            .await
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        let token = body
            .access_token
            .ok_or_else(|| "No access token in refresh response".to_string())?;

        self.bearer_token = Some(token.clone());
        Ok(token)
    }

    /// Get current balance
    pub async fn get_balance(&self) -> Result<BalanceResponse, String> {
        let token = self
            .bearer_token
            .as_ref()
            .ok_or_else(|| "Not authenticated".to_string())?;

        let response = self.client
            .get(format!("{}/api/v1/account/balance", API_BASE))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| format!("Balance request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Balance request failed: {}", response.status()));
        }

        let balance = response
            .json::<BalanceResponse>()
            .await
            .map_err(|e| format!("Failed to parse balance: {}", e))?;

        debug!("Retrieved balance: {:?}", balance.available_balance);
        Ok(balance)
    }

    /// Place a bet
    pub async fn place_bet(
        &self,
        event_id: String,
        market: String,
        selection: String,
        odds: f64,
        stake: f64,
        is_live: bool,
    ) -> Result<BetPlacementResponse, String> {
        let token = self
            .bearer_token
            .as_ref()
            .ok_or_else(|| "Not authenticated".to_string())?;

        let bet_req = BetRequest {
            event_id,
            market,
            selection,
            odds,
            stake,
            is_live,
        };

        info!("Placing bet: {:?}", bet_req);

        let response = self.client
            .post(format!("{}/api/v1/bets/place", API_BASE))
            .bearer_auth(token)
            .json(&bet_req)
            .send()
            .await
            .map_err(|e| format!("Bet placement request failed: {}", e))?;

        let status = response.status();
        let bet_resp = response
            .json::<BetPlacementResponse>()
            .await
            .map_err(|e| format!("Failed to parse bet response: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "Bet placement failed with status {}: {:?}",
                status, bet_resp.error
            ));
        }

        info!("Bet placed successfully: {:?}", bet_resp.bet_id);
        Ok(bet_resp)
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.bearer_token.is_some()
    }

    /// Get bearer token
    pub fn token(&self) -> Option<&String> {
        self.bearer_token.as_ref()
    }

    /// Extract session ID from cookie header
    fn extract_session_id(&self, cookie_header: &str) -> Option<String> {
        // Parse cookie header format: "name1=value1; name2=value2"
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some((name, value)) = cookie.split_once('=') {
                let name = name.trim();
                let value = value.trim();
                
                // Common session cookie names
                if name.eq_ignore_ascii_case("session")
                    || name.eq_ignore_ascii_case("sessionid")
                    || name.eq_ignore_ascii_case("session_id")
                    || name.eq_ignore_ascii_case("pari_session")
                    || name.eq_ignore_ascii_case("sid")
                {
                    return Some(value.to_string());
                }
            }
        }
        None
    }
}

impl Default for PariApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_session_id() {
        let client = PariApiClient::new();
        
        // Test with session cookie
        let cookie = "session=abc123; other=value";
        assert_eq!(
            client.extract_session_id(cookie),
            Some("abc123".to_string())
        );

        // Test with sessionid
        let cookie = "sessionid=xyz789; path=/";
        assert_eq!(
            client.extract_session_id(cookie),
            Some("xyz789".to_string())
        );

        // Test with no session
        let cookie = "other=value; something=else";
        assert_eq!(client.extract_session_id(cookie), None);
    }
}
