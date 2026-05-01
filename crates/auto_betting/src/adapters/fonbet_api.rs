//! Real Fonbet API implementation using cookie-based session authentication
//! 
//! API Base: https://clientsapi24.fonbet.ru
//! 
//! Flow:
//! 1. Import cookies from Playwright session storage
//! 2. Use cookies for authenticated requests
//! 3. No explicit token exchange - cookies are the session

use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const API_BASE: &str = "https://clientsapi24.fonbet.ru";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct FonbetApiClient {
    client: Client,
    cookie_header: Option<String>,
    is_authenticated: bool,
}

#[derive(Debug, Deserialize)]
pub struct FonbetBalance {
    pub available: f64,
    pub total: f64,
    pub bonus: Option<f64>,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
struct FonbetAccountInfo {
    #[serde(rename = "availableBalance")]
    available_balance: Option<f64>,
    #[serde(rename = "totalBalance")]
    total_balance: Option<f64>,
    #[serde(rename = "bonusBalance")]
    bonus_balance: Option<f64>,
    currency: Option<String>,
}

#[derive(Debug, Serialize)]
struct CouponRequest {
    #[serde(rename = "eventId")]
    event_id: String,
    #[serde(rename = "marketId")]
    market_id: String,
    #[serde(rename = "outcomeId")]
    outcome_id: String,
    odds: f64,
    stake: f64,
}

#[derive(Debug, Deserialize)]
pub struct CouponResponse {
    #[serde(rename = "couponId")]
    pub coupon_id: Option<String>,
    #[serde(rename = "ticketId")]
    pub ticket_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    #[serde(rename = "acceptedStake")]
    pub accepted_stake: Option<f64>,
    #[serde(rename = "acceptedOdds")]
    pub accepted_odds: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MinMaxResponse {
    min: Option<f64>,
    max: Option<f64>,
}

impl FonbetApiClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            cookie_header: None,
            is_authenticated: false,
        }
    }

    /// Create client with pre-existing cookies
    pub fn with_cookies(cookie_header: String) -> Self {
        let mut client = Self::new();
        client.cookie_header = Some(cookie_header);
        client.is_authenticated = true;
        client
    }

    /// Authenticate using session cookie from browser storage
    pub async fn authenticate_with_cookie(&mut self, cookie_header: &str) -> Result<(), String> {
        info!("Authenticating with Fonbet API using session cookie");

        // Test authentication by fetching account info
        match self.fetch_account_info(cookie_header).await {
            Ok(account) => {
                info!(
                    "Successfully authenticated with Fonbet API. Balance: {}",
                    account.available_balance.unwrap_or(0.0)
                );
                self.cookie_header = Some(cookie_header.to_string());
                self.is_authenticated = true;
                Ok(())
            }
            Err(e) => {
                error!("Failed to authenticate with Fonbet API: {}", e);
                Err(format!("Authentication failed: {}", e))
            }
        }
    }

    /// Get current balance
    pub async fn get_balance(&self) -> Result<FonbetBalance, String> {
        let cookie_header = self
            .cookie_header
            .as_ref()
            .ok_or_else(|| "Not authenticated".to_string())?;

        let account = self.fetch_account_info(cookie_header).await?;

        let available = account.available_balance.unwrap_or(0.0);
        let total = account.total_balance.unwrap_or(available);

        Ok(FonbetBalance {
            available,
            total,
            bonus: account.bonus_balance,
            currency: account.currency.unwrap_or_else(|| "RUB".into()),
        })
    }

    /// Get min/max stake limits for a specific event/selection
    pub async fn get_min_max_stake(
        &self,
        event_id: &str,
        market_id: &str,
        outcome_id: &str,
    ) -> Result<(f64, f64), String> {
        let cookie_header = self
            .cookie_header
            .as_ref()
            .ok_or_else(|| "Not authenticated".to_string())?;

        let url = format!("{}/api/coupon/getMinMax", API_BASE);
        
        let params = [
            ("eventId", event_id),
            ("marketId", market_id),
            ("outcomeId", outcome_id),
        ];

        let response = self.client
            .get(&url)
            .query(&params)
            .header(header::COOKIE, cookie_header)
            .send()
            .await
            .map_err(|e| format!("Min/max request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Min/max request failed: {}", response.status()));
        }

        let min_max = response
            .json::<MinMaxResponse>()
            .await
            .map_err(|e| format!("Failed to parse min/max: {}", e))?;

        let min = min_max.min.unwrap_or(10.0);
        let max = min_max.max.unwrap_or(100000.0);

        Ok((min, max))
    }

    /// Place a bet (coupon/register)
    pub async fn place_bet(
        &self,
        event_id: String,
        market_id: String,
        outcome_id: String,
        odds: f64,
        stake: f64,
    ) -> Result<CouponResponse, String> {
        let cookie_header = self
            .cookie_header
            .as_ref()
            .ok_or_else(|| "Not authenticated".to_string())?;

        let url = format!("{}/api/coupon/register", API_BASE);

        let coupon_req = CouponRequest {
            event_id,
            market_id,
            outcome_id,
            odds,
            stake,
        };

        info!("Placing Fonbet coupon: {:?}", coupon_req);

        let response = self.client
            .post(&url)
            .header(header::COOKIE, cookie_header)
            .json(&coupon_req)
            .send()
            .await
            .map_err(|e| format!("Coupon request failed: {}", e))?;

        let status = response.status();
        let coupon_resp = response
            .json::<CouponResponse>()
            .await
            .map_err(|e| format!("Failed to parse coupon response: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "Coupon placement failed with status {}: {:?}",
                status, coupon_resp.error
            ));
        }

        info!("Fonbet coupon placed successfully: {:?}", coupon_resp.coupon_id);
        Ok(coupon_resp)
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.is_authenticated
    }

    /// Get cookie header
    pub fn cookie_header(&self) -> Option<&String> {
        self.cookie_header.as_ref()
    }

    /// Fetch account info (internal helper)
    async fn fetch_account_info(&self, cookie_header: &str) -> Result<FonbetAccountInfo, String> {
        let url = format!("{}/api/account/info", API_BASE);

        let response = self.client
            .get(&url)
            .header(header::COOKIE, cookie_header)
            .send()
            .await
            .map_err(|e| format!("Account info request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Account info request failed: {}", response.status()));
        }

        let account = response
            .json::<FonbetAccountInfo>()
            .await
            .map_err(|e| format!("Failed to parse account info: {}", e))?;

        Ok(account)
    }
}

impl Default for FonbetApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fonbet_api_client_new() {
        let client = FonbetApiClient::new();
        assert!(!client.is_authenticated());
        assert!(client.cookie_header().is_none());
    }

    #[test]
    fn test_fonbet_api_client_with_cookies() {
        let client = FonbetApiClient::with_cookies("session=abc123".to_string());
        assert!(client.is_authenticated());
        assert_eq!(client.cookie_header(), Some(&"session=abc123".to_string()));
    }
}
