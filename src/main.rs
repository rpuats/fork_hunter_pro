use api::handlers::AppState;
use api::routes::create_router;
use bot::TelegramBot;
use core::calculator::SurebetCalculator;
use core::event_pool::EventPool;
use core::freebet::FreebetHunter;
use shared::GenerosityIndex as GenIndex;
use core::mirror::MirrorDetector;
use core::momentum::MomentumScanner;
use core::normalizer::Normalizer;
use core::odds_errors::OddsErrorDetector;
use core::value::ValueDetector;
use core::verifier::OddsVerifier;
use parsers::parser_factory::ParserFactory;
use persistence::history::SurebetHistory;
use scanner::engine::GhostScanner;
use scanner::runner::ScannerRunner;
use shared::AppConfig;
use std::sync::Arc;
// use std::future; // removed to avoid conflicts with tokio::main
use axum::http::HeaderValue;
use axum::extract::Path;
use axum::extract::Query;
use anyhow::Result;
use parsers::base::BookmakerParser;
use serde::Deserialize;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    info!("👻 Ghost Imperium v{} starting...", env!("CARGO_PKG_VERSION"));

    let config = AppConfig::load()?;
    info!("Configuration loaded");

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.scanner.request_timeout_secs))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()?,
    );

    let parser_factory = ParserFactory::new(http_client.clone());
    let parsers: Vec<Arc<dyn BookmakerParser + Send + Sync>> = parser_factory.get_enabled();
    info!("{} parsers loaded", parsers.len());

    let calculator = Arc::new(SurebetCalculator::new(
        config.scanner.min_profit_percent,
        config.scanner.max_profit_percent,
        1000.0,
        config.scanner.bloom_filter_capacity,
        config.scanner.bloom_filter_error_rate,
    ));

    let normalizer = Arc::new(Normalizer::new());
    let event_pool = Arc::new(EventPool::new(
        config.scanner.bloom_filter_capacity,
        config.scanner.bloom_filter_error_rate,
        10000,
    ));

    let freebet_hunter = Arc::new(FreebetHunter::new(
        vec![500.0, 1000.0, 2000.0, 5000.0],
        0.0,
        config.scanner.cache_ttl_secs,
    ));

    let generosity_index = Arc::new(GenIndex::new());
    let mirror_detector = Arc::new(MirrorDetector::new(0.1));
    let momentum_scanner = Arc::new(MomentumScanner::new(
        config.scanner.min_profit_percent,
        1000.0,
    ));
    let odds_error_detector = Arc::new(OddsErrorDetector::new(150.0, 3));
    let value_detector = Arc::new(ValueDetector::new(5.0));
    let odds_verifier = Arc::new(OddsVerifier::new(3, 10, 60));

    let event_bus = Arc::new(shared::EventBus::new());

    let history = SurebetHistory::new(&config.database.url).await?;
    let history = Arc::new(history);

    let scanner = Arc::new(GhostScanner::new(
        parsers,
        calculator,
        normalizer,
        event_pool,
        freebet_hunter.clone(),
        generosity_index.clone(),
        mirror_detector,
        momentum_scanner,
        odds_error_detector,
        value_detector,
        odds_verifier,
        event_bus.clone(),
        config.scanner.scan_interval_secs,
    ));

    let scanner_runner = Arc::new(ScannerRunner::new(scanner.clone()));

    let api_state = AppState {
        scanner: scanner_runner.clone(),
        history: history.clone(),
        freebet_hunter: freebet_hunter.clone(),
        generosity_index: generosity_index.clone(),
    };

    let app = create_router(api_state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("🌐 API server listening on {}", addr);

    let scanner_handle = tokio::spawn({
        let runner = scanner_runner.clone();
        async move {
            runner.start().await;
        }
    });

    if !config.telegram.bot_token.is_empty() {
        let bot = TelegramBot::new(
            &config.telegram.bot_token,
            config.telegram.admin_chat_ids,
            config.telegram.notify_min_profit,
            config.telegram.silent_mode,
        );
        tokio::spawn(async move {
            bot.start().await;
        });
    }

    info!("🚀 Ghost Imperium is running!");
    info!("📊 API: http://{}", addr);
    info!("🔗 WebSocket: ws://{}/ws", addr);

    axum::serve(listener, app).await?;

    scanner_runner.stop();
    scanner_handle.abort();

    info!("👋 Ghost Imperium shut down gracefully");
    Ok(())
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fork_hunter=info,tower_http=info".into()),
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}
