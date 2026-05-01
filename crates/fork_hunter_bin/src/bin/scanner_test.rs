use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use auto_betting::engine::AutoBetEngine;
use bankroll_manager::manager::BankrollManager;
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
use express_forks::ExpressForkScanner;
use parsers::parser_factory::ParserFactory;
use persistence::history::SurebetHistory;
use scanner::engine::GhostScanner;
use shared::config::{FeatureFlag, FeatureFlags, RuntimeProfile};
use shared::{AutoBetConfig, BankrollConfig, BonusConfig, EventBus};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "fork_hunter=info,scanner=info,engine=info,parsers=info".into()
            }),
        )
        .init();

    tracing::info!("🧪 Scanner Test starting...");

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?,
    );

    let parser_factory = Arc::new(ParserFactory::new(client));
    let parsers = parser_factory.get_enabled();

    tracing::info!("Loaded {} enabled parsers", parsers.len());

    let history = Arc::new(SurebetHistory::new("memory").await?);

    let scanner = GhostScanner::new(
        parsers,
        Arc::new(SurebetCalculator::new(0.5, 30.0, 1000.0, 10000, 0.01)),
        Arc::new(Normalizer::new()),
        Arc::new(EventPool::new(10_000, 0.01, 10_000)),
        Arc::new(FreebetHunter::new(vec![1000.0], 1.0, 60)),
        Arc::new(GenerosityIndexCalc::new()),
        Arc::new(MirrorDetector::new(0.05)),
        Arc::new(MomentumScanner::new(0.1, 1000.0)),
        Arc::new(OddsErrorDetector::new(25.0, 3)),
        Arc::new(ValueDetector::new(1.0)),
        Arc::new(MiddleDetector::new(0.02, 0.5)),
        Arc::new(OddsVerifier::new(3, 30, 60)),
        Arc::new(CorridorScanner::new(0.5)),
        Arc::new(ExpressForkScanner::new(3, 0.1, 1000.0)),
        Arc::new(BankrollManager::new(BankrollConfig::default())),
        Arc::new(BonusHunter::new(BonusConfig::default())),
        Arc::new(AutoBetEngine::new(AutoBetConfig::default())),
        history,
        Arc::new(EventBus::new()),
        RuntimeProfile::Dev,
        FeatureFlags {
            offline_synced_events_fallback: FeatureFlag::Disabled,
        },
        30,
        30,
        HashMap::new(),
    );

    tracing::info!("🚀 Running scanner cycle...");

    let metrics = scanner.run_cycle().await;

    tracing::info!("📊 Scanner cycle completed!");
    tracing::info!("   Events parsed: {}", metrics.events_parsed);
    tracing::info!("   Surebets found: {}", metrics.surebets_found);
    tracing::info!("   Cycle time: {}ms", metrics.cycle_time_ms);
    tracing::info!("   Active bookmakers: {}", metrics.active_bookmakers);

    tracing::info!("✅ Scanner test completed successfully!");
    Ok(())
}
