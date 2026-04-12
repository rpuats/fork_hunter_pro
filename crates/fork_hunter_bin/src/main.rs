use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use api::handlers::AppState;
use api::routes::create_router;
use auto_betting::engine::AutoBetEngine;
use bankroll_manager::manager::BankrollManager;
use bonus_hunter::hunter::BonusHunter;
use corridor_scanner::scanner::CorridorScanner;
use engine::calculator::SurebetCalculator;
use engine::event_pool::EventPool;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use engine::mirror::MirrorDetector;
use engine::momentum::MomentumScanner;
use engine::normalizer::Normalizer;
use engine::odds_errors::OddsErrorDetector;
use engine::value::ValueDetector;
use engine::verifier::OddsVerifier;
use express_forks::scanner::ExpressForkScanner;
use parsers::parser_factory::ParserFactory;
use persistence::execution_state::ExecutionStateStore;
use persistence::history::SurebetHistory;
use scanner::engine::GhostScanner;
use scanner::runner::ScannerRunner;
use shared::{AppConfig, AutoBetConfig, BankrollConfig, BonusConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "fork_hunter=info,scanner=info,engine=info,parsers=info,tower_http=info".into()
            }),
        )
        .init();

    tracing::info!("Ghost Imperium starting...");

    let config = AppConfig::load()?;
    tracing::info!("Configuration loaded");

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(config.scanner.request_timeout_secs))
            .user_agent("Mozilla/5.0 (Rust) fork_hunter")
            .build()?,
    );

    let parser_factory = Arc::new(ParserFactory::new(http_client.clone()));
    let parser_metadata = Arc::new(parser_factory.bookmaker_metadata());
    let parser_coverage = Arc::new(parser_factory.parser_coverage());
    let parser_health = Arc::new(parser_factory.parser_health_snapshots());
    let parsers = parser_factory.get_enabled();
    tracing::info!("{} parsers loaded", parsers.len());

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

    let generosity_index = Arc::new(GenerosityIndexCalc::new());
    let mirror_detector = Arc::new(MirrorDetector::new(0.1));
    let momentum_scanner = Arc::new(MomentumScanner::new(
        config.scanner.min_profit_percent,
        1000.0,
    ));
    let odds_error_detector = Arc::new(OddsErrorDetector::new(150.0, 3));
    let value_detector = Arc::new(ValueDetector::new(5.0));
    let odds_verifier = Arc::new(OddsVerifier::new(3, 10, 60));

    let corridor_scanner = Arc::new(CorridorScanner::new(0.5));
    let express_fork_scanner = Arc::new(ExpressForkScanner::new(
        3,
        config.scanner.min_profit_percent,
        1000.0,
    ));
    let bankroll_manager = Arc::new(BankrollManager::new(BankrollConfig::default()));
    let bonus_hunter = Arc::new(BonusHunter::new(BonusConfig::default()));

    let execution_state_store = Arc::new(ExecutionStateStore::new(&config.database.url).await?);
    let execution_registry = Arc::new(auto_betting::ExecutionRegistry::with_persistence(
        execution_state_store,
    ));
    if let Err(error) = execution_registry.restore_persisted_state().await {
        tracing::warn!(error = %error, "Failed to restore execution registry state");
    }
    let auto_bet_engine = Arc::new(AutoBetEngine::with_registry(
        AutoBetConfig::default(),
        execution_registry,
    ));

    let event_bus = Arc::new(shared::EventBus::new());
    let history = SurebetHistory::new(&config.database.url).await?;
    let history = Arc::new(history);

    // Telegram bot (optional — запускается если есть токен в конфиге)
    let telegram_handle = if let Some(token) = std::env::var("TELEGRAM_BOT_TOKEN").ok() {
        let admin_chats: Vec<i64> = std::env::var("TELEGRAM_ADMIN_CHATS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if !admin_chats.is_empty() {
            let bot = Arc::new(bot::telegram::TelegramBot::new(
                &token,
                admin_chats,
                config.scanner.min_profit_percent,
                false,
            ));
            tracing::info!("Telegram bot starting...");
            Some(bot.spawn())
        } else {
            tracing::warn!("TELEGRAM_ADMIN_CHATS not set, bot will only respond to commands");
            let bot = Arc::new(bot::telegram::TelegramBot::new(
                &token,
                vec![],
                config.scanner.min_profit_percent,
                false,
            ));
            Some(bot.spawn())
        }
    } else {
        tracing::info!("TELEGRAM_BOT_TOKEN not set, skipping Telegram bot");
        None
    };

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
        corridor_scanner,
        express_fork_scanner,
        bankroll_manager.clone(),
        bonus_hunter.clone(),
        auto_bet_engine.clone(),
        event_bus.clone(),
        config.scanner.scan_interval_secs,
    ));

    let scanner_runner = Arc::new(ScannerRunner::new(scanner.clone()));

    let api_state = AppState {
        scanner: scanner_runner.clone(),
        bookmakers: parser_metadata,
        parser_coverage,
        parser_health,
        history: history.clone(),
        freebet_hunter: freebet_hunter.clone(),
        generosity_index: generosity_index.clone(),
        auto_bet_engine: auto_bet_engine.clone(),
        bankroll_manager: bankroll_manager.clone(),
        bonus_hunter: bonus_hunter.clone(),
        event_bus: event_bus.clone(),
    };

    let app = create_router(api_state);

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {}", addr);

    let scanner_handle = tokio::spawn({
        let r = scanner_runner.clone();
        async move {
            tracing::info!("Scanner task starting...");
            r.start().await;
            tracing::info!("Scanner task finished");
        }
    });
    tracing::info!("Scanner spawned, starting API server");

    tracing::info!("Ghost Imperium is running!");

    axum::serve(listener, app).await?;

    scanner_runner.stop();
    scanner_handle.abort();

    if let Some(handle) = telegram_handle {
        handle.abort();
        tracing::info!("Telegram bot stopped");
    }

    tracing::info!("Ghost Imperium shut down gracefully");
    Ok(())
}
