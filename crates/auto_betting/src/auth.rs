use async_trait::async_trait;
use std::error::Error;

use shared::{BookmakerAccount, BookmakerSession};
use shared::BookmakerSessionState;
use chrono::Utc;

// Trait for bookmaker authorization
#[async_trait]
pub trait BookmakerAuth {
    async fn authorize(&self, account: &BookmakerAccount) -> Result<BookmakerSession, Box<dyn std::error::Error + Send + Sync>>;
}

// Re-export for convenience in adapters that implement the trait
// Pari adapter will implement this trait with a mock authorization first
