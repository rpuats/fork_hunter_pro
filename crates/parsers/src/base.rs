use async_trait::async_trait;
use chrono::Utc;
use shared::{Event, Odd};
use std::fmt;

#[async_trait]
pub trait BookmakerParser: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
    fn is_enabled(&self) -> bool;

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>>;
    async fn fetch_odds(&self, event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>>;
    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>>;

    fn base_url(&self) -> &str;
    fn user_agent(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct ParserResult {
    pub bookmaker: String,
    pub events: Vec<Event>,
    pub odds: Vec<Odd>,
    pub fetch_time_ms: u64,
    pub timestamp: chrono::DateTime<Utc>,
}

impl ParserResult {
    pub fn new(bookmaker: &str, events: Vec<Event>, odds: Vec<Odd>, fetch_time_ms: u64) -> Self {
        Self {
            bookmaker: bookmaker.to_string(),
            events,
            odds,
            fetch_time_ms,
            timestamp: Utc::now(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.odds.is_empty()
    }
}
