// 🧪 Diagnostic tool: показывает fingerprint matching для реальных данных
// Запуск: cargo run --bin debug_matching
//
// Этот инструмент:
// 1. Загружает парсеры
// 2. Fetch события от 2+ БК
// 3. Показывает fingerprint matching
// 4. Пробует найти вилки
// 5. Показывает почему вилки НЕ находятся

use engine::calculator::SurebetCalculator;
use engine::normalizer::Normalizer;
use parsers::parser_factory::ParserFactory;
use std::collections::HashMap;
use std::sync::Arc;

fn normalize_team_name(name: &str) -> String {
    let mut s = name
        .to_lowercase()
        .replace("фк ", "")
        .replace("ск ", "")
        .replace("пк ", "")
        .replace("фк", "")
        .replace("ск", "")
        .replace("пк", "")
        .replace("хк ", "")
        .replace("хк", "")
        .replace(" москва", "")
        .replace(" спб", "")
        .replace(" санкт-петербург", "")
        .replace(" с.-петербург", "")
        .replace(" петербург", "")
        .replace(" питер", "")
        .replace("(", "")
        .replace(")", "")
        .replace("-", " ")
        .replace(".", "")
        .replace(",", "")
        .replace("'", "")
        .replace("\"", "")
        .replace("_", " ")
        .trim()
        .to_string();
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    s.trim().to_string()
}

fn event_fingerprint(event: &shared::Event) -> String {
    let norm = Normalizer::new();
    let norm_event = norm.normalize_event(event.clone());
    let home = normalize_team_name(&norm_event.home_team);
    let away = normalize_team_name(&norm_event.away_team);
    let league = norm_event.league.to_lowercase().replace(" ", "");
    let (first, second) = if home < away {
        (home, away)
    } else {
        (away, home)
    };
    format!("{:?}|{}|{}|{}", event.sport, league, first, second)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🧪 Diagnostic: Cross-BK Event Matching\n");
    println!("Загружаю парсеры...");

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Rust) diagnostic")
            .build()?,
    );

    let factory = ParserFactory::new(http_client.clone());
    let parsers = factory.get_enabled();
    println!("✅ {} парсеров загружено\n", parsers.len());

    // Fetch events от первых 2 парсеров
    let parser_a = &parsers[0];
    let parser_b = &parsers[1];

    println!("📥 Fetch events от {}...", parser_a.slug());
    let result_a = match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        parser_a.fetch_all(),
    )
    .await
    {
        Ok(Ok(r)) => Some(r),
        Ok(Err(e)) => {
            println!("❌ Error: {}", e);
            None
        }
        Err(_) => {
            println!("⏱️  Timeout");
            None
        }
    };

    println!("📥 Fetch events от {}...", parser_b.slug());
    let result_b = match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        parser_b.fetch_all(),
    )
    .await
    {
        Ok(Ok(r)) => Some(r),
        Ok(Err(e)) => {
            println!("❌ Error: {}", e);
            None
        }
        Err(_) => {
            println!("⏱️  Timeout");
            None
        }
    };

    let (events_a, odds_a) = result_a
        .map(|r| (r.events, r.odds))
        .unwrap_or((vec![], vec![]));
    let (events_b, odds_b) = result_b
        .map(|r| (r.events, r.odds))
        .unwrap_or((vec![], vec![]));

    println!("\n📊 Results:");
    println!(
        "  {}: {} events, {} odds",
        parser_a.slug(),
        events_a.len(),
        odds_a.len()
    );
    println!(
        "  {}: {} events, {} odds",
        parser_b.slug(),
        events_b.len(),
        odds_b.len()
    );

    if events_a.is_empty() || events_b.is_empty() {
        println!("\n❌ Не получилось загрузить события от обоих БК");
        return Ok(());
    }

    // Normalizer
    let norm = Normalizer::new();
    let norm_events_a: Vec<_> = events_a
        .iter()
        .map(|e| norm.normalize_event(e.clone()))
        .collect();
    let norm_events_b: Vec<_> = events_b
        .iter()
        .map(|e| norm.normalize_event(e.clone()))
        .collect();

    // Fingerprints
    println!("\n🔍 Fingerprint analysis (first 10 from each):");
    println!("\n{} fingerprints:", parser_a.slug());
    let fps_a: Vec<_> = norm_events_a
        .iter()
        .take(10)
        .map(|e| {
            let fp = event_fingerprint(e);
            println!("  {} vs {} → {}", e.home_team, e.away_team, fp);
            fp
        })
        .collect();

    println!("\n{} fingerprints:", parser_b.slug());
    let fps_b: Vec<_> = norm_events_b
        .iter()
        .take(10)
        .map(|e| {
            let fp = event_fingerprint(e);
            println!("  {} vs {} → {}", e.home_team, e.away_team, fp);
            fp
        })
        .collect();

    // Count matches
    let mut match_count = 0;
    for fp_a in &fps_a {
        if fps_b.contains(fp_a) {
            match_count += 1;
        }
    }

    println!("\n📈 Matching stats:");
    println!("  First 10 events from A: {} match with B", match_count);

    // Full analysis
    println!("\n🔬 Full event matching:");
    let mut all_matches: HashMap<String, Vec<&shared::Event>> = HashMap::new();
    for ev in norm_events_a.iter().chain(norm_events_b.iter()) {
        let fp = event_fingerprint(ev);
        all_matches.entry(fp).or_default().push(ev);
    }

    let multi_bk = all_matches
        .values()
        .filter(|evs| {
            let bks: std::collections::HashSet<_> =
                evs.iter().map(|e| e.bookmaker_slug.as_str()).collect();
            bks.len() >= 2
        })
        .count();

    println!("  Total unique fingerprints: {}", all_matches.len());
    println!("  With 2+ BKs: {}", multi_bk);

    // Show top 5 multi-BK matches
    println!("\n🏆 Top 5 multi-BK matches:");
    let mut shown = 0;
    for (fp, events) in &all_matches {
        if shown >= 5 {
            break;
        }
        let bks: std::collections::HashSet<_> =
            events.iter().map(|e| e.bookmaker_slug.as_str()).collect();
        if bks.len() >= 2 {
            println!("\n  #{} FP: {}", shown + 1, fp);
            println!("    BKs: {:?}", bks);
            println!("    Events: {}", events.len());

            // Show market types
            let markets: std::collections::HashSet<_> = events
                .iter()
                .flat_map(|e| {
                    odds_a
                        .iter()
                        .chain(odds_b.iter())
                        .filter(|o| o.event_id == e.id)
                        .map(|o| o.market.clone())
                })
                .collect();
            println!(
                "    Markets: {:?}",
                markets.iter().take(5).collect::<Vec<_>>()
            );

            shown += 1;
        }
    }

    // Try to find surebets
    println!("\n🧮 Trying to find surebets...");
    let calc = SurebetCalculator::new(0.5, 30.0, 1000.0, 10000, 0.01);

    let all_events: Vec<_> = norm_events_a
        .into_iter()
        .chain(norm_events_b.into_iter())
        .collect();
    let all_odds: Vec<_> = odds_a.into_iter().chain(odds_b.into_iter()).collect();

    let surebets = calc.find_surebets(&all_events, &all_odds);
    println!("  Found {} surebets", surebets.len());

    if !surebets.is_empty() {
        for sb in surebets.iter().take(3) {
            println!("\n  💰 Surebet: {:.2}% profit", sb.profit_percent);
            println!("    {} vs {}", sb.home_team, sb.away_team);
            for leg in &sb.legs {
                println!(
                    "    - {} {}@{:.2} stake={:.2}",
                    leg.bookmaker, leg.selection, leg.odds, leg.stake
                );
            }
        }
    } else {
        println!("\n❌ No surebets found. Possible reasons:");
        println!("  1. No complementary odds (e.g., 1 from BK_A, X from BK_B, 2 from BK_C)");
        println!("  2. Markets don't match between BKs");
        println!("  3. No real arbitrage opportunities at this moment");
        println!("  4. min_profit too high (current: 0.5%)");
    }

    Ok(())
}
