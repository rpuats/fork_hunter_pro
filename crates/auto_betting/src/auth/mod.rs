//! Auth module - Bookmaker authorization and session management
//! Handles credentials storage, browser-based auth, and session persistence

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub mod browser_auth;
pub mod display_config;
pub mod session_storage;
pub mod streaming_auth;

pub use session_storage::SessionStorage;

/// Bookmaker credentials storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerCredentials {
    pub bookmaker_id: String,        // "pari", "fonbet", "marathon", etc.
    pub login: String,               // login/phone/email
    pub password: String,            // encrypted password
    pub phone_prefix: Option<String>, // "+7" for RU, "+375" for BY
    pub two_fa_secret: Option<String>, // TOTP 2FA secret (optional)
    pub status: AuthStatus,
    pub cookies: Option<AuthSession>,
    pub balance: Option<Decimal>,
    pub last_auth: Option<DateTime<Utc>>,
    pub display_config: Option<DisplaySettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    NotAuthenticated,
    AwaitingCaptcha { attempt: u32 },      // waiting for manual captcha input
    Awaiting2FA { method: TwoFAMethod },   // waiting for 2FA code
    Authenticated,                         // full session
    SessionExpired,                        // cookies expired
    AuthFailed(String),                  // error with description
}

impl Default for AuthStatus {
    fn default() -> Self {
        AuthStatus::NotAuthenticated
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TwoFAMethod {
    Sms,
    Totp,
    Email,
    App,
}

impl std::fmt::Display for TwoFAMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TwoFAMethod::Sms => write!(f, "SMS"),
            TwoFAMethod::Totp => write!(f, "TOTP"),
            TwoFAMethod::Email => write!(f, "Email"),
            TwoFAMethod::App => write!(f, "App"),
        }
    }
}

/// Session cookies and browser state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCookies {
    pub cookies: Vec<Cookie>,
    pub user_agent: String,
    pub local_storage: Option<String>,
    pub session_storage: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Default for SessionCookies {
    fn default() -> Self {
        Self {
            cookies: Vec::new(),
            user_agent: String::new(),
            local_storage: None,
            session_storage: None,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<DateTime<Utc>>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
}

/// Display settings per bookmaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    pub odds_format: OddsFormat,
    pub language: String,
    pub animations_enabled: bool,
    pub quick_mode: bool,
    pub custom_css: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OddsFormat {
    Decimal,
    Fractional,
    American,
}

/// Auth events sent to operator
#[derive(Debug, Clone)]
pub enum AuthEvent {
    AuthStarted {
        bookmaker: String,
    },
    AuthProgress {
        bookmaker: String,
        step: String,
    },
    AuthSuccess {
        bookmaker: String,
        balance: Decimal,
    },
    AuthFailed {
        bookmaker: String,
        error: String,
    },
    CaptchaRequired {
        bookmaker: String,
        image_base64: String,
        attempt: u32,
    },
    TwoFARequired {
        bookmaker: String,
        method: TwoFAMethod,
    },
    SessionRestored {
        bookmaker: String,
    },
}

/// Auth errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials for {bookmaker}")]
    InvalidCredentials { bookmaker: String },
    
    #[error("Captcha required for {bookmaker}")]
    CaptchaRequired { bookmaker: String },
    
    #[error("2FA required for {bookmaker} via {method}")]
    TwoFARequired { bookmaker: String, method: TwoFAMethod },
    
    #[error("Session expired for {bookmaker}")]
    SessionExpired { bookmaker: String },
    
    #[error("Browser error: {0}")]
    BrowserError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Operator cancelled")]
    OperatorCancelled,
    
    #[error("Timeout waiting for {operation}")]
    Timeout { operation: String },
    
    #[error("Auth failed for {bookmaker}: {error}")]
    AuthFailed { bookmaker: String, error: String },
}

/// Trait for bookmaker authorization
#[async_trait::async_trait]
pub trait BookmakerAuth: Send + Sync {
    async fn authorize(&self, account: &shared::BookmakerAccount) -> Result<AuthSession, AuthError>;
    async fn check_session(&self, session: &AuthSession) -> Result<bool, AuthError>;
}

/// Session material for bookmaker
#[derive(Debug, Clone)]
pub struct BookmakerSessionMaterial {
    pub session: AuthSession,
    pub credentials: BookmakerCredentials,
    pub cookie_header: Option<String>,
    pub authorization_header: Option<String>,
    pub csrf_token: Option<String>,
    pub user_agent: Option<String>,
    pub extra_headers: Option<HashMap<String, String>>,
    pub source: String,
    pub imported_at: Option<DateTime<Utc>>,
}

impl BookmakerSessionMaterial {
    /// Check if has credentials (STUB)
    pub fn has_credentials(&self) -> bool {
        // STUB: Always return true for now
        true
    }
    
    /// Get summary of session material
    pub fn summary(&self) -> BookmakerSessionMaterialSummary {
        BookmakerSessionMaterialSummary {
            bookmaker_id: self.session.bookmaker_id.clone(),
            is_authenticated: true, // STUB
            balance: None, // STUB
            source: self.source.clone(),
            cookie_header_present: self.cookie_header.is_some(),
            authorization_header_present: self.authorization_header.is_some(),
            csrf_token_present: self.csrf_token.is_some(),
            user_agent_present: self.user_agent.is_some(),
            extra_header_count: self.extra_headers.as_ref().map(|h| h.len()).unwrap_or(0),
            imported_at: self.imported_at.unwrap_or(self.session.created_at),
            redacted_hint: format!("{}***", &self.session.bookmaker_id.chars().take(3).collect::<String>()),
        }
    }
}

/// Auth session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub bookmaker_id: String,
    pub cookies: SessionCookies,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Summary of session material
#[derive(Debug, Clone)]
pub struct BookmakerSessionMaterialSummary {
    pub bookmaker_id: String,
    pub is_authenticated: bool,
    pub balance: Option<Decimal>,
    pub source: String,
    pub cookie_header_present: bool,
    pub authorization_header_present: bool,
    pub csrf_token_present: bool,
    pub user_agent_present: bool,
    pub extra_header_count: usize,
    pub imported_at: DateTime<Utc>,
    pub redacted_hint: String,
}

/// Auth manager - main entry point
pub struct AuthManager {
    pub credentials: HashMap<String, BookmakerCredentials>,
    pub storage: session_storage::SessionStorage,
    event_tx: mpsc::Sender<AuthEvent>,
}

impl AuthManager {
    pub fn new(event_tx: mpsc::Sender<AuthEvent>) -> Self {
        Self {
            credentials: HashMap::new(),
            storage: session_storage::SessionStorage::new(),
            event_tx,
        }
    }

    pub fn add_credentials(&mut self, creds: BookmakerCredentials) {
        self.credentials.insert(creds.bookmaker_id.clone(), creds);
    }

    pub fn get_credentials(&self, bookmaker_id: &str) -> Option<&BookmakerCredentials> {
        self.credentials.get(bookmaker_id)
    }

    pub fn get_all_credentials(&self) -> Vec<&BookmakerCredentials> {
        self.credentials.values().collect()
    }

    pub fn update_status(&mut self, bookmaker_id: &str, status: AuthStatus) {
        if let Some(creds) = self.credentials.get_mut(bookmaker_id) {
            creds.status = status;
        }
    }

    pub async fn try_restore_session(&mut self, bookmaker_id: &str) -> Result<(), AuthError> {
        match self.storage.load_session(bookmaker_id).await {
            Ok(Some(session)) => {
                if let Some(creds) = self.credentials.get_mut(bookmaker_id) {
                    creds.cookies = Some(session);
                    creds.status = AuthStatus::Authenticated;
                    creds.last_auth = Some(Utc::now());
                }
                let _ = self.event_tx.send(AuthEvent::SessionRestored {
                    bookmaker: bookmaker_id.to_string(),
                }).await;
                Ok(())
            }
            Ok(None) => Err(AuthError::SessionExpired {
                bookmaker: bookmaker_id.to_string(),
            }),
            Err(e) => Err(AuthError::StorageError(e.to_string())),
        }
    }

    pub async fn save_session(&self, bookmaker_id: &str) -> Result<(), AuthError> {
        if let Some(creds) = self.credentials.get(bookmaker_id) {
            if let Some(cookies) = &creds.cookies {
                self.storage.save_session(bookmaker_id, cookies).await
                    .map_err(|e| AuthError::StorageError(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn get_authenticated_accounts(&self) -> Vec<&BookmakerCredentials> {
        self.credentials
            .values()
            .filter(|c| matches!(c.status, AuthStatus::Authenticated))
            .collect()
    }

    pub fn get_accounts_needing_auth(&self) -> Vec<&BookmakerCredentials> {
        self.credentials
            .values()
            .filter(|c| {
                matches!(
                    c.status,
                    AuthStatus::NotAuthenticated | AuthStatus::SessionExpired | AuthStatus::AuthFailed(_)
                )
            })
            .collect()
    }

    /// Get session for a bookmaker (STUB)
    pub fn get_session(&self, bookmaker_id: &str) -> Option<&AuthSession> {
        // STUB: Returns None for now
        None
    }
}

/// Supported bookmakers list
pub const SUPPORTED_BOOKMAKERS: &[&str] = &[
    "pari",
    "fonbet",
    "marathon",
    "betcity",
    "zenit",
    "baltbet",
    "bettery",
    "leon",
    "sportbet",
    "bet24",
    "olimp",
    "winline",
];

/// Get human-readable bookmaker name
pub fn get_bookmaker_display_name(bookmaker_id: &str) -> String {
    let names: HashMap<&str, &str> = [
        ("pari", "Пари"),
        ("fonbet", "Фонбет"),
        ("marathon", "Марафон"),
        ("betcity", "Бетсити"),
        ("zenit", "Зенит"),
        ("baltbet", "Балтбет"),
        ("bettery", "Беттери"),
        ("leon", "Леон"),
        ("sportbet", "Спортбет"),
        ("bet24", "24bet"),
        ("olimp", "Олимп"),
        ("winline", "Винлайн"),
    ].iter().cloned().collect();
    
    names.get(bookmaker_id).unwrap_or(&bookmaker_id).to_string()
}

/// Detect phone prefix from login
pub fn detect_phone_prefix(login: &str) -> Option<String> {
    if login.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        // Russian number by default
        Some("+7".to_string())
    } else {
        None
    }
}

/// Format login with phone prefix
pub fn format_login(login: &str, prefix: Option<&str>) -> String {
    if login.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        if login.starts_with('+') {
            login.to_string()
        } else {
            format!("{}{}", prefix.unwrap_or("+7"), login)
        }
    } else {
        login.to_string()
    }
}

/// Authenticate bookmaker with browser (STUB)
pub async fn authenticate_bookmaker(
    _credentials: &BookmakerCredentials,
    _browser: &playwright::api::Browser,
    _event_tx: tokio::sync::mpsc::Sender<AuthEvent>,
) -> Result<AuthSession, AuthError> {
    Err(AuthError::BrowserError("Browser auth not yet implemented".to_string()))
}

/// Continue after captcha (STUB)
pub async fn continue_after_captcha(
    _credentials: &BookmakerCredentials,
    _browser: &playwright::api::Browser,
    _captcha_code: &str,
    _event_tx: tokio::sync::mpsc::Sender<AuthEvent>,
) -> Result<AuthSession, AuthError> {
    Err(AuthError::BrowserError("Captcha handling not yet implemented".to_string()))
}

/// Continue after 2FA (STUB)
pub async fn continue_after_2fa(
    _credentials: &BookmakerCredentials,
    _browser: &playwright::api::Browser,
    _code: &str,
    _event_tx: tokio::sync::mpsc::Sender<AuthEvent>,
) -> Result<AuthSession, AuthError> {
    Err(AuthError::BrowserError("2FA handling not yet implemented".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_login() {
        assert_eq!(format_login("9991234567", Some("+7")), "+79991234567");
        assert_eq!(format_login("+79991234567", Some("+7")), "+79991234567");
        assert_eq!(format_login("user@email.com", None), "user@email.com");
    }

    #[test]
    fn test_two_fa_method_display() {
        assert_eq!(TwoFAMethod::Sms.to_string(), "SMS");
        assert_eq!(TwoFAMethod::Totp.to_string(), "TOTP");
    }
}
