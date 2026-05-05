use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use rust_decimal::Decimal;

use api::handlers::AppState;
use api::routes::create_router;
use api::EventBroadcaster;
use auto_betting::engine::AutoBetEngine;
use auto_betting::auth::AuthManager;
use auto_betting::BrowserPool;
use auto_betting::betting::{BetMode, OperatorQueue};
use auto_betting::{ExecutionOrchestrator, PerformanceTargets, init_global_monitor};
use bankroll_manager::manager::BankrollManager;
use tokio::sync::Mutex as TokioMutex;
use bonus_hunter::hunter::BonusHunter;
use corridor_scanner::scanner::CorridorScanner;
use engine::calculator::SurebetCalculator;
use engine::event_pool::EventPool;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use engine::middle::MiddleDetector;
use engine::mirror::MirrorDetector;
use engine::momentum::MomentumScanner;
use engine::normalizer::Normalizer;
use engine::odds_errors::OddsErrorDetector;
use engine::value::ValueDetector;
use engine::verifier::OddsVerifier;
use express_forks::scanner::ExpressForkScanner;
use parsers::parser_factory::ParserFactory;
use persistence::execution_ledger::ExecutionLedgerStore;
use persistence::execution_state::ExecutionStateStore;
use persistence::freebet_lifecycle::FreebetLifecycleStore;
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
    tracing::info!(
        profile = ?config.profile,
        offline_synced_events_fallback = config.features.offline_synced_events_fallback_enabled(),
        "Configuration loaded"
    );
    if matches!(config.profile, shared::config::RuntimeProfile::Production) {
        tracing::info!(
            cors_origins = ?config.server.cors_origins,
            "Production profile guardrails validated"
        );
    }

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(config.scanner.request_timeout_secs))
            .user_agent("Mozilla/5.0 (Rust) fork_hunter")
            .build()?,
    );

    let parser_factory = Arc::new(ParserFactory::new(http_client.clone()));
    let parser_metadata = Arc::new(parser_factory.bookmaker_metadata());
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
    let middle_detector = Arc::new(MiddleDetector::default());
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
    let execution_ledger_store = Arc::new(ExecutionLedgerStore::new(&config.database.url).await?);
    let freebet_lifecycle_store = Arc::new(FreebetLifecycleStore::new(&config.database.url).await?);
    let execution_registry = Arc::new(auto_betting::ExecutionRegistry::with_persistence(
        execution_state_store.clone(),
    ));
    if let Err(error) = execution_registry.restore_persisted_state().await {
        tracing::warn!(error = %error, "Failed to restore execution registry state");
    }
    for snapshot in execution_registry.list_balance_snapshots() {
        bankroll_manager.apply_balance_snapshot(&snapshot);
    }
    let auto_bet_engine = Arc::new(AutoBetEngine::with_registry_ledger_and_state(
        AutoBetConfig::default(),
        execution_registry,
        execution_ledger_store.clone(),
        execution_state_store.clone(),
    ));

    let event_bus = Arc::new(shared::EventBus::new());
    let history = SurebetHistory::new(&config.database.url).await?;
    let history = Arc::new(history);

    let telegram_token = if config.telegram.bot_token.is_empty() {
        std::env::var("TELEGRAM_BOT_TOKEN").ok().unwrap_or_default()
    } else {
        config.telegram.bot_token.clone()
    };
    let telegram_admin_chats = if config.telegram.admin_chat_ids.is_empty() {
        std::env::var("TELEGRAM_ADMIN_CHATS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|value| value.trim().parse().ok())
            .collect()
    } else {
        config.telegram.admin_chat_ids.clone()
    };

    // Telegram bot + EventBus bridge (optional)
    let telegram_handles = if telegram_token.is_empty() {
        tracing::info!("Telegram token not configured, skipping Telegram bot");
        None
    } else {
        if telegram_admin_chats.is_empty() {
            tracing::warn!("Telegram admin chats not configured, bridge will stay command-only");
        }

        let bot = Arc::new(bot::telegram::TelegramBot::new(
            &telegram_token,
            telegram_admin_chats,
            config.telegram.notify_min_profit,
            config.telegram.silent_mode,
            Some(event_bus.clone()),
        ));
        tracing::info!("Telegram bot starting...");
        Some((
            bot.clone().spawn(),
            bot::spawn_event_bus_bridge(bot, event_bus.clone()),
        ))
    };

    let scanner = Arc::new(
        GhostScanner::new(
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
            middle_detector,
            odds_verifier,
            corridor_scanner,
            express_fork_scanner,
            bankroll_manager.clone(),
            bonus_hunter.clone(),
            auto_bet_engine.clone(),
            history.clone(),
            event_bus.clone(),
            config.profile,
            config.features.clone(),
            config.scanner.scan_interval_secs,
            config.scanner.request_timeout_secs,
            config.bookmakers.per_bookmaker_timeout_secs.clone(),
        )
        .with_parser_execution_config(&config.scanner)
        .with_freebet_lifecycle_store(freebet_lifecycle_store.clone()),
    );

    let scanner_runner = Arc::new(ScannerRunner::new(scanner.clone()));

    // Initialize new auth and event systems
    let (auth_tx, _auth_rx) = tokio::sync::mpsc::channel(100);
    let auth_manager = Arc::new(TokioMutex::new(AuthManager::new(auth_tx)));
    let browser_pool = Arc::new(BrowserPool::default());
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let execution_orchestrator = Arc::new(TokioMutex::new(
        ExecutionOrchestrator::new(Decimal::from(100000), BetMode::SemiAuto)
    ));
    let operator_queue = Arc::new(TokioMutex::new(OperatorQueue::new()));

    // Initialize performance monitor
    let _perf_monitor = init_global_monitor(PerformanceTargets {
        scan_cycle_ms: 500,
        fork_to_display_ms: 1000,
        auto_bet_ms: 5000,
        semi_auto_bet_ms: 10000,
        ui_fps: 60,
    });
    tracing::info!("Performance monitor initialized with targets: scan=500ms, fork_to_display=1000ms, auto_bet=5000ms");

    let api_state = AppState {
        scanner: scanner_runner.clone(),
        parser_runtime_stale_after_secs: config.scanner.cache_ttl_secs,
        parser_factory: parser_factory.clone(),
        bookmakers: parser_metadata,
        history: history.clone(),
        execution_ledger: execution_ledger_store.clone(),
        execution_state_store: execution_state_store.clone(),
        freebet_lifecycle_store: Some(freebet_lifecycle_store.clone()),
        freebet_hunter: freebet_hunter.clone(),
        generosity_index: generosity_index.clone(),
        auto_bet_engine: auto_bet_engine.clone(),
        bankroll_manager: bankroll_manager.clone(),
        bonus_hunter: bonus_hunter.clone(),
        event_bus: event_bus.clone(),
        auth_manager,
        browser_pool,
        event_broadcaster,
        execution_orchestrator,
        operator_queue,
    };

    let app = create_router(Arc::new(api_state.clone()));

    // Create event channel for scanner bridge
    let (scanner_event_tx, scanner_event_rx) = tokio::sync::mpsc::channel::<shared::BusEvent>(1000);
    
    // Spawn event bus bridge for scanner events
    let _event_bus_bridge = tokio::spawn({
        let event_bus = event_bus.clone();
        let tx = scanner_event_tx.clone();
        async move {
            let mut rx = event_bus.subscribe("scanner_bridge");
            while let Ok(event) = rx.recv().await {
                let _ = tx.send(event).await;
            }
        }
    });

    // Spawn scanner bridge
    let scanner_bridge_handle = auto_betting::spawn_scanner_bridge(
        api_state.execution_orchestrator.clone(),
        api_state.auth_manager.clone(),
        api_state.browser_pool.clone(),
        api_state.operator_queue.clone(),
        scanner_event_rx,
    );

    // Spawn betting runner (temporarily disabled - BettingRunnerConfig not found)
    // let (betting_runner_handle, betting_runner_task) = auto_betting::spawn_betting_runner(
    //     api_state.execution_orchestrator.clone(),
    //     api_state.operator_queue.clone(),
    //     api_state.auth_manager.clone(),
    //     api_state.browser_pool.clone(),
    //     BettingRunnerConfig {
    //         mode: BetMode::SemiAuto,
    //         check_interval_ms: 100,
    //         max_concurrent_forks: 5,
    //         auto_retry_failures: true,
    //     },
    // );
    
    // Start betting runner
    // betting_runner_handle.start().await;

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
    tracing::info!("Scanner spawned, bridge active, starting API server");

    tracing::info!("Ghost Imperium is running!");

    axum::serve(listener, app).await?;

    scanner_runner.stop();
    scanner_handle.abort();
    scanner_bridge_handle.abort();
    // betting_runner_task.abort();
    tracing::info!("Scanner, bridge, and betting runner stopped");

    if let Some((bot_handle, bridge_handle)) = telegram_handles {
        bridge_handle.abort();
        bot_handle.abort();
        tracing::info!("Telegram bot and EventBus bridge stopped");
    }

    tracing::info!("Ghost Imperium shut down gracefully");
    Ok(())
}
