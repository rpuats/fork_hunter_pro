//! Auth handlers - API endpoints for bookmaker authorization

use auto_betting::auth::{
    AuthEvent, AuthManager, AuthStatus, BookmakerCredentials, SessionCookies, TwoFAMethod,
    authenticate_bookmaker, continue_after_2fa, continue_after_captcha,
    get_bookmaker_display_name, SUPPORTED_BOOKMAKERS,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use super::AppState;

/// Account response DTO
#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub bookmaker_id: String,
    pub login: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub phone_prefix: Option<String>,
    pub status: String,
    pub balance: Option<Decimal>,
    pub last_auth: Option<String>,
    pub error_message: Option<String>,
}

/// Add account request
#[derive(Debug, Deserialize)]
pub struct AddAccountRequest {
    pub bookmaker_id: String,
    pub login: String,
    pub password: String,
    pub phone_prefix: Option<String>,
    pub two_fa_secret: Option<String>,
}

/// Captcha submission
#[derive(Debug, Deserialize)]
pub struct CaptchaRequest {
    pub code: String,
}

/// 2FA submission
#[derive(Debug, Deserialize)]
pub struct TwoFARequest {
    pub code: String,
}

/// Auth progress response
#[derive(Debug, Serialize)]
pub struct AuthProgressResponse {
    pub status: String,
    pub step: Option<String>,
    pub requires_action: Option<String>, // "captcha", "2fa"
}

/// List all accounts
pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<AccountResponse>> {
    let auth_manager = state.auth_manager.lock().await;
    
    let accounts: Vec<AccountResponse> = auth_manager
        .get_all_credentials()
        .into_iter()
        .map(|c| AccountResponse {
            bookmaker_id: c.bookmaker_id.clone(),
            login: c.login.clone(),
            password: c.password.clone(),
            phone_prefix: c.phone_prefix.clone(),
            status: match c.status {
                AuthStatus::NotAuthenticated => "NotAuthenticated".to_string(),
                AuthStatus::AwaitingCaptcha { .. } => "AwaitingCaptcha".to_string(),
                AuthStatus::Awaiting2FA { .. } => "Awaiting2FA".to_string(),
                AuthStatus::Authenticated => "Authenticated".to_string(),
                AuthStatus::SessionExpired => "SessionExpired".to_string(),
                AuthStatus::AuthFailed(_) => "AuthFailed".to_string(),
            },
            balance: c.balance,
            last_auth: c.last_auth.map(|d| d.to_rfc3339()),
            error_message: match &c.status {
                AuthStatus::AuthFailed(err) => Some(err.clone()),
                _ => None,
            },
        })
        .collect();
    
    Json(accounts)
}

/// Add new account
pub async fn add_account(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddAccountRequest>,
) -> Result<Json<AccountResponse>, (axum::http::StatusCode, String)> {
    // Validate bookmaker is supported
    if !SUPPORTED_BOOKMAKERS.contains(&request.bookmaker_id.as_str()) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            format!("Unsupported bookmaker: {}", request.bookmaker_id),
        ));
    }
    
    // Detect phone prefix if not provided and login is numeric
    let phone_prefix = if request.phone_prefix.is_none() && request.login.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        Some("+7".to_string())
    } else {
        request.phone_prefix
    };
    
    let credentials = BookmakerCredentials {
        bookmaker_id: request.bookmaker_id.clone(),
        login: request.login.clone(),
        password: request.password,
        phone_prefix: phone_prefix.clone(),
        two_fa_secret: request.two_fa_secret,
        status: AuthStatus::NotAuthenticated,
        cookies: None,
        balance: None,
        last_auth: None,
        display_config: None,
    };
    
    let mut auth_manager = state.auth_manager.lock().await;
    auth_manager.add_credentials(credentials);
    
    let response = AccountResponse {
        bookmaker_id: request.bookmaker_id,
        login: request.login,
        password: "***".to_string(),
        phone_prefix,
        status: "NotAuthenticated".to_string(),
        balance: None,
        last_auth: None,
        error_message: None,
    };
    
    Ok(Json(response))
}

/// Remove account
pub async fn remove_account(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    let mut auth_manager = state.auth_manager.lock().await;
    
    // Remove from memory
    auth_manager.credentials.remove(&bk_id);
    
    // Delete saved session
    if let Err(e) = auth_manager.storage.delete_session(&bk_id).await {
        tracing::warn!("Failed to delete session for {}: {}", bk_id, e);
    }
    
    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .unwrap())
}

/// Authenticate single account
pub async fn authenticate_one(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
) -> Result<Json<AuthProgressResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let auth_manager = state.auth_manager.lock().await;
    
    let credentials = auth_manager
        .get_credentials(&bk_id)
        .cloned()
        .ok_or_else(|| {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Account not found",
                    "code": "NOT_FOUND"
                })),
            )
        })?;
    
    drop(auth_manager); // Release lock
    
    // Get event channel
    let (tx, mut rx) = mpsc::channel::<AuthEvent>(32);
    
    // Spawn authentication task
    let auth_manager_clone = state.auth_manager.clone();
    let browser_pool = state.browser_pool.clone();
    
    tokio::spawn(async move {
        match browser_pool.get_browser().await {
            Ok(browser) => {
                match authenticate_bookmaker(&credentials, &browser, tx.clone()).await {
                    Ok(session) => {
                        // Update credentials with session
                        let mut manager = auth_manager_clone.lock().await;
                        if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                            creds.cookies = Some(session);
                            creds.status = AuthStatus::Authenticated;
                            creds.last_auth = Some(Utc::now());
                        }
                        // Save session to disk
                        let _ = manager.save_session(&bk_id).await;
                    }
                    Err(e) => {
                        let mut manager = auth_manager_clone.lock().await;
                        if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                            match &e {
                                auto_betting::auth::AuthError::CaptchaRequired { .. } => {
                                    creds.status = AuthStatus::AwaitingCaptcha { attempt: 1 };
                                }
                                auto_betting::auth::AuthError::TwoFARequired { method, .. } => {
                                    creds.status = AuthStatus::Awaiting2FA { method: method.clone() };
                                }
                                _ => {
                                    creds.status = AuthStatus::AuthFailed(e.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(AuthEvent::AuthFailed {
                    bookmaker: bk_id.clone(),
                    error: format!("Browser error: {}", e),
                }).await;
            }
        }
    });
    
    // Wait for first event to determine status
    match rx.recv().await {
        Some(AuthEvent::AuthStarted { .. }) => {
            Ok(Json(AuthProgressResponse {
                status: "Started".to_string(),
                step: Some("opening_browser".to_string()),
                requires_action: None,
            }))
        }
        Some(AuthEvent::AuthFailed { error, .. }) => {
            Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": error,
                    "code": "AUTH_FAILED"
                })),
            ))
        }
        _ => {
            Ok(Json(AuthProgressResponse {
                status: "Started".to_string(),
                step: None,
                requires_action: None,
            }))
        }
    }
}

/// Authenticate all selected accounts
#[derive(Debug, Deserialize)]
pub struct AuthenticateAllRequest {
    pub bookmakers: Vec<String>,
}

pub async fn authenticate_all(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AuthenticateAllRequest>,
) -> Result<Json<Vec<AuthProgressResponse>>, (axum::http::StatusCode, String)> {
    let mut results = Vec::new();
    
    for bk_id in request.bookmakers {
        // Start authentication for each (non-blocking)
        let state_clone = state.clone();
        tokio::spawn(async move {
            // Reuse authenticate_one logic
            let _ = authenticate_one(State(state_clone), Path(bk_id)).await;
        });
        
        results.push(AuthProgressResponse {
            status: "Queued".to_string(),
            step: Some("queued".to_string()),
            requires_action: None,
        });
    }
    
    Ok(Json(results))
}

/// Submit captcha code
pub async fn submit_captcha(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
    Json(request): Json<CaptchaRequest>,
) -> Result<Json<AccountResponse>, (axum::http::StatusCode, String)> {
    let auth_manager = state.auth_manager.lock().await;
    
    let credentials = auth_manager
        .get_credentials(&bk_id)
        .cloned()
        .ok_or_else(|| {
            (axum::http::StatusCode::NOT_FOUND, "Account not found".to_string())
        })?;
    
    drop(auth_manager);
    
    // Verify account is in awaiting captcha state
    if !matches!(credentials.status, AuthStatus::AwaitingCaptcha { .. }) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Account is not awaiting captcha".to_string(),
        ));
    }
    
    // Continue authentication with captcha
    let (tx, _rx) = mpsc::channel::<AuthEvent>(32);
    let browser = state.browser_pool.get_browser().await
        .map_err(|e| (axum::http::StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    
    match continue_after_captcha(&credentials, &browser, &request.code, tx).await {
        Ok(session) => {
            let mut manager = state.auth_manager.lock().await;
            if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                creds.cookies = Some(session);
                creds.status = AuthStatus::Authenticated;
                creds.last_auth = Some(Utc::now());
            }
            let _ = manager.save_session(&bk_id).await;
        }
        Err(e) => {
            let mut manager = state.auth_manager.lock().await;
            if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                creds.status = AuthStatus::AuthFailed(e.to_string());
            }
        }
    }
    
    // Return updated account
    let manager = state.auth_manager.lock().await;
    let creds = manager.get_credentials(&bk_id).unwrap();
    
    Ok(Json(AccountResponse {
        bookmaker_id: creds.bookmaker_id.clone(),
        login: creds.login.clone(),
        password: "***".to_string(),
        phone_prefix: creds.phone_prefix.clone(),
        status: match creds.status {
            AuthStatus::Authenticated => "Authenticated".to_string(),
            _ => "AuthFailed".to_string(),
        },
        balance: creds.balance,
        last_auth: creds.last_auth.map(|d| d.to_rfc3339()),
        error_message: None,
    }))
}

/// Submit 2FA code
pub async fn submit_2fa(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
    Json(request): Json<TwoFARequest>,
) -> Result<Json<AccountResponse>, (axum::http::StatusCode, String)> {
    let auth_manager = state.auth_manager.lock().await;
    
    let credentials = auth_manager
        .get_credentials(&bk_id)
        .cloned()
        .ok_or_else(|| {
            (axum::http::StatusCode::NOT_FOUND, "Account not found".to_string())
        })?;
    
    drop(auth_manager);
    
    // Verify account is in awaiting 2FA state
    if !matches!(credentials.status, AuthStatus::Awaiting2FA { .. }) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Account is not awaiting 2FA".to_string(),
        ));
    }
    
    // Continue authentication with 2FA code
    let (tx, _rx) = mpsc::channel::<AuthEvent>(32);
    let browser = state.browser_pool.get_browser().await
        .map_err(|e| (axum::http::StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    
    match continue_after_2fa(&credentials, &browser, &request.code, tx).await {
        Ok(session) => {
            let mut manager = state.auth_manager.lock().await;
            if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                creds.cookies = Some(session);
                creds.status = AuthStatus::Authenticated;
                creds.last_auth = Some(Utc::now());
            }
            let _ = manager.save_session(&bk_id).await;
        }
        Err(e) => {
            let mut manager = state.auth_manager.lock().await;
            if let Some(creds) = manager.credentials.get_mut(&bk_id) {
                creds.status = AuthStatus::AuthFailed(e.to_string());
            }
        }
    }
    
    // Return updated account
    let manager = state.auth_manager.lock().await;
    let creds = manager.get_credentials(&bk_id).unwrap();
    
    Ok(Json(AccountResponse {
        bookmaker_id: creds.bookmaker_id.clone(),
        login: creds.login.clone(),
        password: "***".to_string(),
        phone_prefix: creds.phone_prefix.clone(),
        status: match creds.status {
            AuthStatus::Authenticated => "Authenticated".to_string(),
            _ => "AuthFailed".to_string(),
        },
        balance: creds.balance,
        last_auth: creds.last_auth.map(|d| d.to_rfc3339()),
        error_message: None,
    }))
}

/// Logout from account
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    let mut auth_manager = state.auth_manager.lock().await;
    
    if let Some(creds) = auth_manager.credentials.get_mut(&bk_id) {
        creds.status = AuthStatus::NotAuthenticated;
        creds.cookies = None;
        creds.balance = None;
    }
    
    // Delete saved session
    let _ = auth_manager.storage.delete_session(&bk_id).await;
    
    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .unwrap())
}

/// Get balance for account
pub async fn get_balance(
    State(state): State<Arc<AppState>>,
    Path(bk_id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let auth_manager = state.auth_manager.lock().await;
    
    let creds = auth_manager
        .get_credentials(&bk_id)
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "Account not found".to_string()))?;
    
    // If not authenticated, can't get balance
    if !matches!(creds.status, AuthStatus::Authenticated) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Account not authenticated".to_string(),
        ));
    }
    
    // TODO: Scrape balance using browser with existing session
    // For now return cached balance
    Ok(Json(serde_json::json!({
        "bookmaker_id": bk_id,
        "balance": creds.balance,
        "currency": "RUB",
        "updated_at": Utc::now().to_rfc3339(),
    })))
}

/// Get supported bookmakers
pub async fn supported_bookmakers() -> Json<Vec<serde_json::Value>> {
    let bookmakers: Vec<serde_json::Value> = SUPPORTED_BOOKMAKERS
        .iter()
        .map(|&id| {
            serde_json::json!({
                "id": id,
                "name": get_bookmaker_display_name(id),
            })
        })
        .collect();
    
    Json(bookmakers)
}

/// Auth routes
pub fn auth_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/accounts", get(list_accounts))
        .route("/auth/accounts", post(add_account))
        .route("/auth/accounts/:bk_id", delete(remove_account))
        .route("/auth/authenticate-all", post(authenticate_all))
        .route("/auth/authenticate/:bk_id", post(authenticate_one))
        .route("/auth/logout/:bk_id", post(logout))
        .route("/auth/captcha/:bk_id", post(submit_captcha))
        .route("/auth/2fa/:bk_id", post(submit_2fa))
        .route("/auth/balance/:bk_id", get(get_balance))
        .route("/auth/supported", get(supported_bookmakers))
}
