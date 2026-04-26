use std::sync::Arc;
use std::time::Duration;

use parsers::parser_factory::ParserFactory;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("fork_hunter=info,parsers=info")
        .init();

    tracing::info!("🧪 Simple Parser Test starting...");

    // Create parser factory
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let factory = ParserFactory::new(Arc::new(client));

    tracing::info!("Available parsers: {:?}", factory.registered_slugs());

    // Test a few parsers
    for slug in &["pari", "marathon", "bettery"] {
        if let Some(parser) = factory.get(slug) {
            tracing::info!("Testing {} parser...", slug);
            match parser.fetch_events().await {
                Ok(events) => {
                    tracing::info!("  ✅ {}: {} events", slug, events.len());
                }
                Err(e) => {
                    tracing::warn!("  ❌ {}: {}", slug, e);
                }
            }
        }
    }

    tracing::info!("✅ Parser test completed!");
    Ok(())
}