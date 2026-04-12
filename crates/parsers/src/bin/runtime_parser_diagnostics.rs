use parsers::diagnostics::{run_runtime_diagnostics, DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let requested: Vec<String> = std::env::args().skip(1).collect();
    let effective = if requested.is_empty() {
        DEFAULT_RUNTIME_DIAGNOSTIC_SLUGS
            .iter()
            .map(|slug| (*slug).to_string())
            .collect::<Vec<_>>()
    } else {
        requested.clone()
    };

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("client"),
    );

    println!("runtime-only parser diagnostics");
    println!("thresholds: live >= 100, prematch >= 2000");
    println!("slugs: {}", effective.join(", "));
    println!();
    println!("slug,total,live,prematch,live_ok,prematch_ok,pass,runtime_only,duration_ms,error");

    for report in run_runtime_diagnostics(client, &requested).await {
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            report.bookmaker_slug,
            report.total_events,
            report.live_events,
            report.prematch_events,
            report.live_threshold_met,
            report.prematch_threshold_met,
            report.passed,
            report.runtime_only,
            report.duration_ms,
            report.error.unwrap_or_default().replace(',', ";")
        );
    }
}
