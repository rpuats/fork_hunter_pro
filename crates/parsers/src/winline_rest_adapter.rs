use crate::base::{BookmakerParser, ParserResult};
use crate::winline_rest::WinlineRestParser;
use reqwest::Client;
use std::sync::Arc;
use std::time::Instant;
use chrono::Utc;

/// Адаптер Winline REST парсера
pub struct WinlineRestAdapter {
    parser: WinlineRestParser,
}

impl WinlineRestAdapter {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            parser: WinlineRestParser::new(client),
        }
    }

    /// Парсит события из Winline
    pub async fn parse(&self) -> Result<ParserResult, String> {
        let start = Instant::now();
        
        let events = self.parser.fetch_events().await?;
        let fetch_time_ms = start.elapsed().as_millis() as u64;

        Ok(ParserResult::new(
            "winline",
            events,
            vec![],
            fetch_time_ms,
        ))
    }

    pub fn name(&self) -> &str {
        "Winline REST API"
    }
}
