use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

use shared::{BookmakerAccount, BookmakerSession};

#[derive(Debug, Clone)]
pub struct BookmakerSessionMaterial {
    pub cookie_header: Option<String>,
    pub authorization_header: Option<String>,
    pub csrf_token: Option<String>,
    pub user_agent: Option<String>,
    pub extra_headers: BTreeMap<String, String>,
    pub source: String,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmakerSessionMaterialSummary {
    pub source: String,
    pub cookie_header_present: bool,
    pub authorization_header_present: bool,
    pub csrf_token_present: bool,
    pub user_agent_present: bool,
    pub extra_header_count: usize,
    pub imported_at: DateTime<Utc>,
    pub redacted_hint: String,
}

impl BookmakerSessionMaterial {
    pub fn has_credentials(&self) -> bool {
        self.cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
            || self
                .authorization_header
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
    }

    pub fn summary(&self) -> BookmakerSessionMaterialSummary {
        let cookie_len = self.cookie_header.as_deref().map(str::len).unwrap_or(0);
        let auth_len = self
            .authorization_header
            .as_deref()
            .map(str::len)
            .unwrap_or(0);
        let csrf_len = self.csrf_token.as_deref().map(str::len).unwrap_or(0);
        let user_agent_len = self.user_agent.as_deref().map(str::len).unwrap_or(0);

        BookmakerSessionMaterialSummary {
            source: self.source.clone(),
            cookie_header_present: cookie_len > 0,
            authorization_header_present: auth_len > 0,
            csrf_token_present: csrf_len > 0,
            user_agent_present: user_agent_len > 0,
            extra_header_count: self.extra_headers.len(),
            imported_at: self.imported_at,
            redacted_hint: format!(
                "cookie:{cookie_len}chars;auth:{auth_len}chars;csrf:{csrf_len}chars;ua:{user_agent_len}chars;extra:{}",
                self.extra_headers.len()
            ),
        }
    }
}

// Trait for bookmaker authorization
#[async_trait]
pub trait BookmakerAuth {
    async fn authorize(
        &self,
        account: &BookmakerAccount,
    ) -> Result<BookmakerSession, Box<dyn std::error::Error + Send + Sync>>;
}

// Re-export for convenience in adapters that implement the trait
// Pari adapter will implement this trait with a mock authorization first
