//! Streaming Auth - Real-time browser automation with operator interaction
//! Handles the full auth flow with WebSocket event streaming

use super::{
    AuthError, AuthEvent, AuthStatus, BookmakerCredentials, SessionCookies, TwoFAMethod,
    browser_auth::{self, get_login_url},
    display_config, format_login,
};
use anyhow::Result;
use chrono::Utc;
use playwright::api::{Browser, BrowserContext, Page};
use rust_decimal::Decimal;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Streaming authentication with real-time event reporting
pub async fn streaming_authenticate(
    credentials: &BookmakerCredentials,
    browser: &Browser,
    event_tx: mpsc::Sender<AuthEvent>,
    captcha_rx: &mut mpsc::Receiver<String>,
    twofa_rx: &mut mpsc::Receiver<String>,
) -> Result<SessionCookies, AuthError> {
    let context = browser
        .new_context()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let page = context
        .new_page()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Step 1: Open login page
    send_event(&event_tx, AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "opening_browser".to_string(),
    }).await;

    let login_url = get_login_url(&credentials.bookmaker_id);
    page.goto_builder(login_url)
        .goto()
        .await
        .map_err(|e| AuthError::BrowserError(format!("Failed to open login page: {}", e)))?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Step 2: Fill login
    send_event(&event_tx, AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "filling_login".to_string(),
    }).await;

    fill_login_with_prefix(&page, credentials).await?;

    // Step 3: Fill password
    send_event(&event_tx, AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "filling_password".to_string(),
    }).await;

    fill_password(&page, credentials).await?;

    // Step 4: Submit
    send_event(&event_tx, AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "submitting".to_string(),
    }).await;

    click_submit(&page, &credentials.bookmaker_id).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Step 5: Handle captcha if present
    if detect_captcha(&page, &credentials.bookmaker_id).await? {
        send_event(&event_tx, AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "awaiting_captcha".to_string(),
        }).await;

        // Take screenshot
        let screenshot = page
            .screenshot_builder()
            .screenshot()
            .await
            .map_err(|e| AuthError::BrowserError(e.to_string()))?;

        send_event(&event_tx, AuthEvent::CaptchaRequired {
            bookmaker: credentials.bookmaker_id.clone(),
            image_base64: base64::encode(&screenshot),
            attempt: 1,
        }).await;

        // Wait for operator to provide captcha code
        let captcha_code = wait_for_operator_input(captcha_rx, 120, "captcha").await?;
        
        send_event(&event_tx, AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "filling_captcha".to_string(),
        }).await;

        fill_captcha(&page, &captcha_code).await?;
        click_submit(&page, &credentials.bookmaker_id).await?;
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    // Step 6: Handle 2FA if present
    if let Some(two_fa_method) = detect_2fa(&page, &credentials.bookmaker_id).await? {
        send_event(&event_tx, AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "awaiting_2fa".to_string(),
        }).await;

        send_event(&event_tx, AuthEvent::TwoFARequired {
            bookmaker: credentials.bookmaker_id.clone(),
            method: two_fa_method.clone(),
        }).await;

        // Wait for operator to provide 2FA code
        let twofa_code = wait_for_operator_input(twofa_rx, 60, "2FA code").await?;

        send_event(&event_tx, AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "filling_2fa".to_string(),
        }).await;

        fill_2fa(&page, &twofa_code).await?;
        submit_2fa(&page).await?;
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    // Step 7: Verify login success
    if !verify_logged_in(&page, &credentials.bookmaker_id).await? {
        return Err(AuthError::AuthFailed {
            bookmaker: credentials.bookmaker_id.clone(),
            error: "Login verification failed".to_string(),
        });
    }

    // Step 8: Apply display configuration
    send_event(&event_tx, AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "configuring_display".to_string(),
    }).await;

    let config = display_config::get_display_config(&credentials.bookmaker_id);
    display_config::apply_display_config(&page, &config)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Step 9: Scrape balance
    let balance = scrape_balance(&page, &credentials.bookmaker_id).await?;

    // Step 10: Extract session
    let session = extract_session(&context, &page).await?;

    send_event(&event_tx, AuthEvent::AuthSuccess {
        bookmaker: credentials.bookmaker_id.clone(),
        balance,
    }).await;

    Ok(session)
}

/// Wait for operator input with timeout
async fn wait_for_operator_input(
    rx: &mut mpsc::Receiver<String>,
    timeout_secs: u64,
    what: &str,
) -> Result<String, AuthError> {
    match timeout(Duration::from_secs(timeout_secs), rx.recv()).await {
        Ok(Some(code)) => Ok(code),
        Ok(None) => Err(AuthError::OperatorCancelled),
        Err(_) => Err(AuthError::Timeout {
            operation: format!("Waiting for {}", what),
        }),
    }
}

/// Send event helper
async fn send_event(tx: &mpsc::Sender<AuthEvent>, event: AuthEvent) {
    let _ = tx.send(event).await;
}

// ============ Browser interaction helpers ============

async fn fill_login_with_prefix(page: &Page, credentials: &BookmakerCredentials) -> Result<(), AuthError> {
    let formatted_login = format_login(
        &credentials.login,
        credentials.phone_prefix.as_deref(),
    );

    let selectors = match credentials.bookmaker_id.as_str() {
        "pari" => vec!["input[name='phone']", "input[name='login']", "input[type='tel']"],
        "fonbet" => vec!["input[name='login']", "input[type='text']"],
        "marathon" => vec!["input[name='username']", "input[name='login']"],
        _ => vec!["input[name='login']", "input[name='username']", "input[type='tel']"],
    };

    for selector in selectors {
        if let Ok(field) = page.wait_for_selector_with_timeout(selector, 2000).await {
            field
                .fill(&formatted_login)
                .await
                .map_err(|e| AuthError::BrowserError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(AuthError::BrowserError("Login field not found".to_string()))
}

async fn fill_password(page: &Page, credentials: &BookmakerCredentials) -> Result<(), AuthError> {
    let selectors = vec![
        "input[type='password']",
        "input[name='password']",
        "input[name='pass']",
        ".password-input",
    ];

    for selector in selectors {
        if let Ok(field) = page.wait_for_selector_with_timeout(selector, 2000).await {
            field
                .fill(&credentials.password)
                .await
                .map_err(|e| AuthError::BrowserError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(AuthError::BrowserError("Password field not found".to_string()))
}

async fn click_submit(page: &Page, bookmaker_id: &str) -> Result<(), AuthError> {
    let selectors: Vec<&str> = match bookmaker_id {
        "pari" => vec!["button[type='submit']", ".login-button", ".btn-login"],
        "fonbet" => vec![".login-submit", "button[type='submit']", ".btn-primary"],
        "marathon" => vec!["[data-testid='login-btn']", ".login-btn", ".btn-login"],
        "winline" => vec![".login-form__submit", "button[type='submit']"],
        "leon" => vec![".auth-button", "button[type='submit']"],
        _ => vec!["button[type='submit']", ".submit", ".login-button"],
    };

    for selector in selectors {
        if page.click(selector).await.is_ok() {
            return Ok(());
        }
    }

    // Fallback: press Enter
    page.keyboard().press("Enter").await.ok();
    Ok(())
}

async fn detect_captcha(page: &Page, bookmaker_id: &str) -> Result<bool, AuthError> {
    let selectors: Vec<&str> = match bookmaker_id {
        "pari" => vec![".g-recaptcha", ".hcaptcha", "[data-testid='captcha']", ".captcha"],
        "fonbet" => vec![".captcha-container", ".captcha-image", ".g-recaptcha"],
        "marathon" => vec![".captcha", ".recaptcha", "[data-testid='captcha']"],
        _ => vec![".captcha", ".g-recaptcha", ".hcaptcha", ".recaptcha"],
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    for selector in selectors {
        match page.is_visible(selector).await {
            Ok(true) => return Ok(true),
            _ => continue,
        }
    }

    Ok(false)
}

async fn fill_captcha(page: &Page, code: &str) -> Result<(), AuthError> {
    let selectors = vec![
        "input[name='captcha']",
        ".captcha-input",
        "[data-testid='captcha-input']",
        "input[placeholder*='апч']",
        "input[placeholder*='apch']",
    ];

    for selector in selectors {
        if let Ok(field) = page.wait_for_selector_with_timeout(selector, 2000).await {
            field
                .fill(code)
                .await
                .map_err(|e| AuthError::BrowserError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(AuthError::BrowserError("Captcha field not found".to_string()))
}

async fn detect_2fa(page: &Page, bookmaker_id: &str) -> Result<Option<TwoFAMethod>, AuthError> {
    let selectors = match bookmaker_id {
        "pari" => vec![
            ("input[name='code']", TwoFAMethod::Sms),
            ("[data-testid='2fa-input']", TwoFAMethod::Totp),
            ("input[placeholder*='код']", TwoFAMethod::Sms),
        ],
        "fonbet" => vec![
            (".two-fa-input", TwoFAMethod::Totp),
            ("input[name='sms_code']", TwoFAMethod::Sms),
            ("input[placeholder*='код']", TwoFAMethod::Sms),
        ],
        "winline" => vec![
            ("input[name='otp']", TwoFAMethod::Sms),
            (".two-fa-input", TwoFAMethod::Totp),
        ],
        _ => vec![
            ("input[name='code']", TwoFAMethod::Sms),
            ("input[name='otp']", TwoFAMethod::Sms),
            (".two-fa", TwoFAMethod::Totp),
            ("input[placeholder*='код']", TwoFAMethod::Sms),
        ],
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    for (selector, method) in selectors {
        match page.is_visible(selector).await {
            Ok(true) => return Ok(Some(method)),
            _ => continue,
        }
    }

    Ok(None)
}

async fn fill_2fa(page: &Page, code: &str) -> Result<(), AuthError> {
    let selectors = vec![
        "input[name='code']",
        "input[name='otp']",
        "input[name='sms_code']",
        ".two-fa-input",
        "[data-testid='2fa-input']",
        "input[placeholder*='код']",
    ];

    for selector in selectors {
        if let Ok(field) = page.wait_for_selector_with_timeout(selector, 2000).await {
            field
                .fill(code)
                .await
                .map_err(|e| AuthError::BrowserError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(AuthError::BrowserError("2FA field not found".to_string()))
}

async fn submit_2fa(page: &Page) -> Result<(), AuthError> {
    let selectors = vec![
        "button[type='submit']",
        ".confirm-2fa",
        ".submit-2fa",
        ".btn-primary",
        "[data-testid='confirm-2fa']",
    ];

    for selector in selectors {
        if page.click(selector).await.is_ok() {
            return Ok(());
        }
    }

    page.keyboard().press("Enter").await.ok();
    Ok(())
}

async fn verify_logged_in(page: &Page, bookmaker_id: &str) -> Result<bool, AuthError> {
    let success_selectors: Vec<&str> = match bookmaker_id {
        "pari" => vec![".user-balance", "[data-testid='balance']", ".user-menu", ".account-dropdown"],
        "fonbet" => vec![".user-info", ".balance", ".account-menu", ".user-balance"],
        "marathon" => vec![".user-balance", ".account-dropdown", ".balance-amount"],
        "winline" => vec![".user-balance", ".header-balance", ".account-menu"],
        "leon" => vec![".balance", ".user-menu", ".account-info"],
        _ => vec![".balance", ".user-menu", ".account", ".user-info"],
    };

    tokio::time::sleep(Duration::from_secs(3)).await;

    for selector in success_selectors {
        match page.is_visible(selector).await {
            Ok(true) => return Ok(true),
            _ => continue,
        }
    }

    // Check URL doesn't contain login
    let url = page.url().await.unwrap_or_default();
    if !url.contains("login") && !url.contains("auth") {
        return Ok(true);
    }

    Ok(false)
}

async fn scrape_balance(page: &Page, bookmaker_id: &str) -> Result<Decimal, AuthError> {
    let selectors: Vec<&str> = match bookmaker_id {
        "pari" => vec![".user-balance", "[data-testid='balance']", ".balance-amount"],
        "fonbet" => vec![".balance-amount", ".user-balance", ".header-balance"],
        "marathon" => vec![".user-balance", ".balance", ".account-balance"],
        "winline" => vec![".user-balance", ".header-balance", ".balance-value"],
        "leon" => vec![".balance", ".user-balance", ".balance-amount"],
        _ => vec![".balance", ".user-balance"],
    };

    for selector in selectors {
        if let Ok(element) = page.wait_for_selector_with_timeout(selector, 3000).await {
            if let Ok(text) = element.text_content().await {
                // Parse balance - handle formats like "15 420 ₽", "15,420.50", "15420"
                let cleaned: String = text
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                    .collect()
                    .replace(',', ".");
                
                if let Ok(balance) = cleaned.parse::<f64>() {
                    return Ok(Decimal::try_from(balance).unwrap_or(Decimal::ZERO));
                }
            }
        }
    }

    Ok(Decimal::ZERO)
}

async fn extract_session(context: &BrowserContext, page: &Page) -> Result<SessionCookies, AuthError> {
    use super::Cookie;

    let cookies = context
        .cookies()
        .await
        .map_err(|e| AuthError::BrowserError(format!("Failed to get cookies: {}", e)))?;

    let cookies: Vec<Cookie> = cookies
        .into_iter()
        .map(|c| Cookie {
            name: c.name,
            value: c.value,
            domain: c.domain,
            path: c.path,
            expires: c.expires.map(|e| chrono::DateTime::from_timestamp(e as i64, 0).unwrap_or_else(|| Utc::now())),
            http_only: c.http_only,
            secure: c.secure,
            same_site: c.same_site.map(|s| format!("{:?}", s)),
        })
        .collect();

    let user_agent = page
        .evaluate("() => navigator.userAgent")
        .await
        .and_then(|r| r.value().ok_or_else(|| playwright::Error::External("No value".to_string())))
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;
    let user_agent = user_agent.as_str().unwrap_or("").to_string();

    let local_storage = page
        .evaluate("() => JSON.stringify(localStorage)")
        .await
        .ok()
        .and_then(|r| r.value().ok())
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let session_storage = page
        .evaluate("() => JSON.stringify(sessionStorage)")
        .await
        .ok()
        .and_then(|r| r.value().ok())
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    Ok(SessionCookies {
        cookies,
        user_agent,
        local_storage,
        session_storage,
        created_at: Utc::now(),
    })
}
