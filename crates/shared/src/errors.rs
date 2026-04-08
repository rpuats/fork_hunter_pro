use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Bookmaker error: {0}")]
    Bookmaker(String),

    #[error("Parser error: {bookmaker} - {source}")]
    Parser {
        bookmaker: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Odds parsing error: {0}")]
    OddsParsing(String),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("No arbitrage opportunity found")]
    NoArbitrage,

    #[error("Database error: {0}")]
    Database(String),

    #[error("HTTP error: {status} - {message}")]
    Http { status: u16, message: String },

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Rate limit exceeded for {bookmaker}")]
    RateLimited { bookmaker: String },

    #[error("Circuit breaker open for {bookmaker}")]
    CircuitBreakerOpen { bookmaker: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;
