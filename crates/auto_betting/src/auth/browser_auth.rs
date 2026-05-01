//! Browser-based authentication using Playwright
//! Handles the full auth flow including captcha and 2FA

use super::{
    AuthError, AuthEvent, AuthStatus, BookmakerCredentials, Cookie, SessionCookies, TwoFAMethod,
    display_config::{self, DisplaySettings, OddsFormat},
    format_login,
};
use anyhow::{Context, Result};
use chrono::Utc;
use playwright::api::{Browser, BrowserContext, Page};
use rust_decimal::Decimal;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Authenticate with a bookmaker using browser automation
pub async fn authenticate_bookmaker(
    credentials: &BookmakerCredentials,
    browser: &Browser,
    event_tx: mpsc::Sender<AuthEvent>,
) -> Result<SessionCookies, AuthError> {
    // Send started event
    let _ = event_tx.send(AuthEvent::AuthStarted {
        bookmaker: credentials.bookmaker_id.clone(),
    }).await;

    let context = browser
        .new_context()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let page = context
        .new_page()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Step 1: Open login page
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "opening_browser".to_string(),
    }).await;

    let login_url = get_login_url(&credentials.bookmaker_id);
    page.goto_builder(login_url)
        .goto()
        .await
        .map_err(|e| AuthError::BrowserError(format!("Failed to open login page: {}", e)))?;

    // Wait for page to load
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Step 2: Fill login
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "filling_login".to_string(),
    }).await;

    fill_login_field(&page, credentials).await?;

    // Step 3: Fill password
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "filling_password".to_string(),
    }).await;

    fill_password_field(&page, credentials).await?;

    // Step 4: Submit
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "submitting".to_string(),
    }).await;

    click_submit(&page, &credentials.bookmaker_id).await?;

    // Wait for response
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Step 5: Check for captcha
    if detect_captcha(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        let _ = event_tx.send(AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "awaiting_captcha".to_string(),
        }).await;

        // Take screenshot
        let screenshot = page
            .screenshot_builder()
            .screenshot()
            .await
            .map_err(|e| AuthError::BrowserError(e.to_string()))?;

        let base64 = base64::encode(&screenshot);

        let _ = event_tx
            .send(AuthEvent::CaptchaRequired {
                bookmaker: credentials.bookmaker_id.clone(),
                image_base64: base64,
                attempt: 1,
            })
            .await;

        return Err(AuthError::CaptchaRequired {
            bookmaker: credentials.bookmaker_id.clone(),
        });
    }

    // Step 6: Check for 2FA
    if let Some(method) = detect_2fa(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        let _ = event_tx.send(AuthEvent::AuthProgress {
            bookmaker: credentials.bookmaker_id.clone(),
            step: "awaiting_2fa".to_string(),
        }).await;

        let _ = event_tx
            .send(AuthEvent::TwoFARequired {
                bookmaker: credentials.bookmaker_id.clone(),
                method: method.clone(),
            })
            .await;

        return Err(AuthError::TwoFARequired {
            bookmaker: credentials.bookmaker_id.clone(),
            method,
        });
    }

    // Step 7: Check if login was successful
    if !is_logged_in(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        return Err(AuthError::AuthFailed {
            bookmaker: credentials.bookmaker_id.clone(),
            error: "Login failed - check credentials".to_string(),
        });
    }

    // Step 8: Apply display configuration
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "configuring_display".to_string(),
    }).await;

    let config = display_config::get_display_config(&credentials.bookmaker_id);
    display_config::apply_display_config(&page, &config)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Step 9: Scrape balance
    let balance = scrape_balance(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Step 10: Save session
    let _ = event_tx.send(AuthEvent::AuthProgress {
        bookmaker: credentials.bookmaker_id.clone(),
        step: "saving_session".to_string(),
    }).await;

    let session = extract_session(&context, &page)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Send success event
    let _ = event_tx.send(AuthEvent::AuthSuccess {
        bookmaker: credentials.bookmaker_id.clone(),
        balance,
    }).await;

    Ok(session)
}

/// Continue authentication after captcha is solved
pub async fn continue_after_captcha(
    credentials: &BookmakerCredentials,
    browser: &Browser,
    captcha_code: &str,
    event_tx: mpsc::Sender<AuthEvent>,
) -> Result<SessionCookies, AuthError> {
    // Reuse existing context or create new
    let context = browser
        .new_context()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let page = context
        .new_page()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Navigate back to login
    let login_url = get_login_url(&credentials.bookmaker_id);
    page.goto_builder(login_url)
        .goto()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Fill login/password again
    fill_login_field(&page, credentials).await?;
    fill_password_field(&page, credentials).await?;

    // Fill captcha
    fill_captcha(&page, captcha_code, &credentials.bookmaker_id).await?;

    // Submit
    click_submit(&page, &credentials.bookmaker_id).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check for 2FA
    if let Some(method) = detect_2fa(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        let _ = event_tx
            .send(AuthEvent::TwoFARequired {
                bookmaker: credentials.bookmaker_id.clone(),
                method: method.clone(),
            })
            .await;

        return Err(AuthError::TwoFARequired {
            bookmaker: credentials.bookmaker_id.clone(),
            method,
        });
    }

    // Check success and extract session
    if !is_logged_in(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        return Err(AuthError::AuthFailed {
            bookmaker: credentials.bookmaker_id.clone(),
            error: "Login failed after captcha".to_string(),
        });
    }

    // Apply config and extract
    let config = display_config::get_display_config(&credentials.bookmaker_id);
    display_config::apply_display_config(&page, &config)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let balance = scrape_balance(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let session = extract_session(&context, &page)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let _ = event_tx.send(AuthEvent::AuthSuccess {
        bookmaker: credentials.bookmaker_id.clone(),
        balance,
    }).await;

    Ok(session)
}

/// Continue authentication after 2FA code is provided
pub async fn continue_after_2fa(
    credentials: &BookmakerCredentials,
    browser: &Browser,
    code: &str,
    event_tx: mpsc::Sender<AuthEvent>,
) -> Result<SessionCookies, AuthError> {
    let context = browser
        .new_context()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let page = context
        .new_page()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    // Navigate to 2FA page or re-login
    let login_url = get_login_url(&credentials.bookmaker_id);
    page.goto_builder(login_url)
        .goto()
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Fill login/password
    fill_login_field(&page, credentials).await?;
    fill_password_field(&page, credentials).await?;
    click_submit(&page, &credentials.bookmaker_id).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Fill 2FA code
    fill_2fa_code(&page, code, &credentials.bookmaker_id).await?;

    // Submit 2FA
    click_2fa_submit(&page, &credentials.bookmaker_id).await?;

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check success
    if !is_logged_in(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?
    {
        return Err(AuthError::AuthFailed {
            bookmaker: credentials.bookmaker_id.clone(),
            error: "2FA failed".to_string(),
        });
    }

    // Apply config and extract
    let config = display_config::get_display_config(&credentials.bookmaker_id);
    display_config::apply_display_config(&page, &config)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let balance = scrape_balance(&page, &credentials.bookmaker_id)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let session = extract_session(&context, &page)
        .await
        .map_err(|e| AuthError::BrowserError(e.to_string()))?;

    let _ = event_tx.send(AuthEvent::AuthSuccess {
        bookmaker: credentials.bookmaker_id.clone(),
        balance,
    }).await;

    Ok(session)
}

// ============ Helper functions ============

fn get_login_url(bookmaker_id: &str) -> &'static str {
    match bookmaker_id {
        "pari" => "https://www.pari.ru/",
        "fonbet" => "https://www.fonbet.ru/",
        "marathon" => "https://www.marathonbet.ru/",
        "leon" => "https://leon.ru/",
        "winline" => "https://winline.ru/",
        "zenit" => "https://zenitbet.com/",
        "betcity" => "https://betcity.ru/",
        "baltbet" => "https://baltbet.ru/",
        "bettery" => "https://bettery.ru/",
        "sportbet" => "https://sportbet.ru/",
        _ => "https://www.google.com/",
    }
}

async fn fill_login_field(page: &Page, credentials: &BookmakerCredentials) -> Result<(), AuthError> {
    let selectors = get_login_selectors(&credentials.bookmaker_id);
    let formatted_login = format_login(
        &credentials.login,
        credentials.phone_prefix.as_deref(),
    );

    for selector in selectors {
        if let Ok(field) = page.wait_for_selector_with_timeout(&selector, 2000).await {
            field
                .fill(&formatted_login)
                .await
                .map_err(|e| AuthError::BrowserError(e.to_string()))?;
            return Ok(());
        }
    }

    Err(AuthError::BrowserError(
        "Could not find login field".to_string(),
    ))
}

async fn fill_password_field(
    page: &Page,
    credentials: &BookmakerCredentials,
) -> Result<(), AuthError> {
    let selectors = vec![
        "input[type='password']",
        "input[name='password']",
        "input[name='pass']",
        ".password-input",
        "[data-testid='password-input']",
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

    Err(AuthError::BrowserError(
        "Could not find password field".to_string(),
    ))
}

async fn click_submit(page: &Page, bookmaker_id: &str) -> Result<(), AuthError> {
    let selectors = match bookmaker_id {
        "pari" => vec!["button[type='submit']", ".login-button"],
        "fonbet" => vec![".login-submit", "button[type='submit']"],
        "marathon" => vec!["[data-testid='login-btn']", ".login-btn"],
        _ => vec!["button[type='submit']", ".submit", ".login-button"],
    };

    for selector in selectors {
        if page.click(selector).await.is_ok() {
            return Ok(());
        }
    }

    // Try pressing Enter
    page.keyboard().press("Enter").await.ok();

    Ok(())
}

async fn detect_captcha(page: &Page, bookmaker_id: &str) -> Result<bool> {
    let captcha_selectors = match bookmaker_id {
        "pari" => vec![".g-recaptcha", ".hcaptcha", "[data-testid='captcha']"],
        "fonbet" => vec![".captcha-container", ".captcha-image"],
        _ => vec![".captcha", ".g-recaptcha", ".hcaptcha"],
    };

    for selector in captcha_selectors {
        if page.is_visible(selector).await.unwrap_or(false) {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn detect_2fa(page: &Page, bookmaker_id: &str) -> Result<Option<TwoFAMethod>> {
    let selectors = match bookmaker_id {
        "pari" => vec![
            ("input[name='code']", TwoFAMethod::Sms),
            ("[data-testid='2fa-input']", TwoFAMethod::Totp),
        ],
        "fonbet" => vec![
            (".two-fa-input", TwoFAMethod::Totp),
            ("input[name='sms_code']", TwoFAMethod::Sms),
        ],
        _ => vec![
            ("input[name='code']", TwoFAMethod::Sms),
            (".two-fa", TwoFAMethod::Totp),
        ],
    };

    for (selector, method) in selectors {
        if page.is_visible(selector).await.unwrap_or(false) {
            return Ok(Some(method));
        }
    }

    Ok(None)
}

async fn is_logged_in(page: &Page, bookmaker_id: &str) -> Result<bool> {
    let success_selectors = match bookmaker_id {
        "pari" => vec![".user-balance", "[data-testid='balance']", ".user-menu"],
        "fonbet" => vec![".user-info", ".balance", ".account-menu"],
        "marathon" => vec![".user-balance", ".account-dropdown"],
        _ => vec![".balance", ".user-menu", ".account"],
    };

    // Wait a bit for redirect
    tokio::time::sleep(Duration::from_secs(2)).await;

    for selector in success_selectors {
        if page.is_visible(selector).await.unwrap_or(false) {
            return Ok(true);
        }
    }

    // Check URL - if redirected to main page, likely logged in
    let url = page.url().await.unwrap_or_default();
    if !url.contains("login") && !url.contains("auth") {
        return Ok(true);
    }

    Ok(false)
}

async fn fill_captcha(
    page: &Page,
    code: &str,
    bookmaker_id: &str,
) -> Result<(), AuthError> {
    let selectors = vec![
        "input[name='captcha']",
        ".captcha-input",
        "[data-testid='captcha-input']",
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

    Err(AuthError::BrowserError(
        "Could not find captcha field".to_string(),
    ))
}

async fn fill_2fa_code(
    page: &Page,
    code: &str,
    bookmaker_id: &str,
) -> Result<(), AuthError> {
    let selectors = vec![
        "input[name='code']",
        "input[name='sms_code']",
        ".two-fa-input",
        "[data-testid='2fa-input']",
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

    Err(AuthError::BrowserError(
        "Could not find 2FA field".to_string(),
    ))
}

async fn click_2fa_submit(page: &Page, bookmaker_id: &str) -> Result<(), AuthError> {
    let selectors = vec![
        "button[type='submit']",
        ".confirm-2fa",
        ".submit-2fa",
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

async fn scrape_balance(page: &Page, bookmaker_id: &str) -> Result<Decimal> {
    let selectors = match bookmaker_id {
        "pari" => vec![".user-balance", "[data-testid='balance']"],
        "fonbet" => vec![".balance-amount", ".user-balance"],
        "marathon" => vec![".user-balance", ".balance"],
        _ => vec![".balance", ".user-balance"],
    };

    for selector in selectors {
        if let Ok(element) = page.wait_for_selector_with_timeout(selector, 2000).await {
            if let Ok(text) = element.text_content().await {
                // Parse balance from text like "15 420 ₽" or "15,420.50"
                let cleaned: String = text
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                    .collect();
                
                if let Ok(balance) = cleaned.parse::<f64>() {
                    return Ok(Decimal::try_from(balance).unwrap_or(Decimal::ZERO));
                }
            }
        }
    }

    Ok(Decimal::ZERO)
}

async fn extract_session(context: &BrowserContext, page: &Page) -> Result<SessionCookies> {
    // Get cookies
    let pw_cookies = context
        .cookies()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get cookies: {}", e))?;

    let cookies: Vec<Cookie> = pw_cookies
        .into_iter()
        .map(|c| Cookie {
            name: c.name,
            value: c.value,
            domain: c.domain,
            path: c.path,
            expires: c.expires.map(|e| {
                // Convert seconds since epoch to DateTime
                chrono::DateTime::from_timestamp(e as i64, 0).unwrap_or_else(|| Utc::now())
            }),
            http_only: c.http_only,
            secure: c.secure,
            same_site: c.same_site.map(|s| format!("{:?}", s)),
        })
        .collect();

    // Get user agent
    let user_agent = page
        .evaluate("() => navigator.userAgent")
        .await
        .and_then(|r| r.value().ok_or_else(|| playwright::Error::External("No value".to_string())))
        .map_err(|e| anyhow::anyhow!("Failed to get user agent: {}", e))?;
    let user_agent = user_agent.as_str().unwrap_or("").to_string();

    // Get local storage
    let local_storage = page
        .evaluate("() => JSON.stringify(localStorage)")
        .await
        .ok()
        .and_then(|r| r.value().ok())
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // Get session storage
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

fn get_login_selectors(bookmaker_id: &str) -> Vec<&'static str> {
    match bookmaker_id {
        "pari" => vec!["input[name='phone']", "input[name='login']", "input[type='tel']"],
        "fonbet" => vec!["input[name='login']", "input[type='text']"],
        "marathon" => vec!["input[name='username']", "input[name='login']"],
        _ => vec!["input[name='login']", "input[name='username']", "input[type='tel']"],
    }
}
