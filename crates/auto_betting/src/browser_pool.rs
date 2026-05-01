//! Browser Pool - Manages Playwright browser instances

use anyhow::{Context, Result};
use playwright::api::Browser;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

/// Pool of browser instances for automation
pub struct BrowserPool {
    /// Semaphore to limit concurrent browsers
    semaphore: Arc<Semaphore>,
    /// Playwright instance
    playwright: Arc<Mutex<Option<playwright::Playwright>>>,
}

impl BrowserPool {
    /// Create a new browser pool with max concurrent browsers
    pub fn new(max_browsers: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_browsers)),
            playwright: Arc::new(Mutex::new(None)),
        }
    }

    /// Get a browser instance from the pool
    pub async fn get_browser(&self) -> Result<Browser> {
        // Acquire permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .context("Failed to acquire browser permit")?;

        // Initialize playwright if needed
        let mut playwright_guard = self.playwright.lock().await;
        if playwright_guard.is_none() {
            let playwright = playwright::Playwright::initialize()
                .await
                .context("Failed to initialize Playwright")?;
            *playwright_guard = Some(playwright);
        }

        let playwright = playwright_guard.as_ref().unwrap();

        // Launch browser
        let browser = playwright
            .chromium()
            .launcher()
            .headless(false) // Visible browser for auth
            .launch()
            .await
            .context("Failed to launch browser")?;

        Ok(browser)
    }

    /// Create pool with sensible defaults
    pub fn default() -> Self {
        Self::new(5)
    }
}

impl Default for BrowserPool {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_browser_pool_creation() {
        let pool = BrowserPool::new(3);
        assert_eq!(pool.semaphore.available_permits(), 3);
    }
}
