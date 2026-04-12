// Быстрая проверка: есть ли 1X2 odds от обеих БК для одного матча
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
    println!("🔍 Checking 1X2 odds coverage between BKs...\n");

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0")
            .build()
            .expect("Failed to build HTTP client"),
    );

    let factory = ParserFactory::new(http_client.clone());
    let parsers = factory.get_enabled();

    // Take 24bet and Bettery
    let parser_24bet = parsers.iter().find(|p| p.slug() == "_24bet").unwrap();
    let parser_bettery = parsers.iter().find(|p| p.slug() == "bettery").unwrap();

    println!("Fetching from 24bet...");
    let result_24bet =
        tokio::time::timeout(std::time::Duration::from_secs(60), parser_24bet.fetch_all())
            .await
            .unwrap()
            .unwrap();

    println!("Fetching from Bettery...");
    let result_bettery = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        parser_bettery.fetch_all(),
    )
    .await
    .unwrap()
    .unwrap();

    println!(
        "\n📊 24bet: {} events, {} odds",
        result_24bet.events.len(),
        result_24bet.odds.len()
    );
    println!(
        "📊 Bettery: {} events, {} odds\n",
        result_bettery.events.len(),
        result_bettery.odds.len()
    );

    // Group by fingerprint
    let norm = Normalizer::new();

    let mut fp_to_events: HashMap<String, Vec<shared::Event>> = HashMap::new();
    let mut fp_to_odds: HashMap<String, Vec<shared::Odd>> = HashMap::new();

    let mut all_events = Vec::new();
    let mut all_odds: Vec<shared::Odd> = Vec::new();

    // 24bet
    for ev in &result_24bet.events {
        let norm_ev = norm.normalize_event(ev.clone());
        let fp = event_fingerprint(&norm_ev);
        fp_to_events.entry(fp).or_default().push(norm_ev.clone());
        all_events.push(norm_ev);
    }
    for odd in &result_24bet.odds {
        if let Some(ev) = all_events.iter().find(|e| e.id == odd.event_id) {
            let fp = event_fingerprint(ev);
            fp_to_odds.entry(fp).or_default().push(odd.clone());
        }
    }

    // Bettery
    let battery_events: Vec<_> = result_bettery
        .events
        .iter()
        .map(|ev| norm.normalize_event(ev.clone()))
        .collect();
    let battery_odds: Vec<_> = result_bettery.odds.clone();

    for ev in &battery_events {
        let fp = event_fingerprint(ev);
        fp_to_events.entry(fp).or_default().push(ev.clone());
    }
    for odd in &battery_odds {
        if let Some(ev) = battery_events.iter().find(|e| e.id == odd.event_id) {
            let fp = event_fingerprint(ev);
            fp_to_odds.entry(fp).or_default().push(odd.clone());
        }
    }

    // Find matches with 1X2 from both BKs
    println!("🔎 Searching for matches with 1X2 from both BKs...\n");

    let mut count_1x2_both = 0;
    let mut count_full_1x2 = 0; // Has 1, X, AND 2

    for (fp, odds) in &fp_to_odds {
        let bks: std::collections::HashSet<_> =
            odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
        if bks.len() < 2 {
            continue;
        }

        let odds_1x2: Vec<_> = odds.iter().filter(|o| o.market == "1X2").collect();
        if odds_1x2.is_empty() {
            continue;
        }

        let has_1 = odds_1x2.iter().any(|o| o.selection == "1");
        let has_x = odds_1x2.iter().any(|o| o.selection == "X");
        let has_2 = odds_1x2.iter().any(|o| o.selection == "2");

        if has_1 && has_x && has_2 {
            count_full_1x2 += 1;
            if count_full_1x2 <= 3 {
                println!("✅ Match #{}: {}", count_full_1x2, fp);
                println!("   BKs: {:?}", bks);
                let ones: Vec<_> = odds_1x2.iter().filter(|o| o.selection == "1").collect();
                let xs: Vec<_> = odds_1x2.iter().filter(|o| o.selection == "X").collect();
                let twos: Vec<_> = odds_1x2.iter().filter(|o| o.selection == "2").collect();

                println!(
                    "   1: {} odds (best: {:.2})",
                    ones.len(),
                    ones.iter().map(|o| o.odds).fold(0.0f64, f64::max)
                );
                println!(
                    "   X: {} odds (best: {:.2})",
                    xs.len(),
                    xs.iter().map(|o| o.odds).fold(0.0f64, f64::max)
                );
                println!(
                    "   2: {} odds (best: {:.2})",
                    twos.len(),
                    twos.iter().map(|o| o.odds).fold(0.0f64, f64::max)
                );

                // Calculate surebet profit
                let best_1 = ones.iter().map(|o| o.odds).fold(0.0f64, f64::max);
                let best_x = xs.iter().map(|o| o.odds).fold(0.0f64, f64::max);
                let best_2 = twos.iter().map(|o| o.odds).fold(0.0f64, f64::max);

                if let Some(profit) =
                    shared::odds::calculate_surebet_profit(&[best_1, best_x, best_2])
                {
                    println!("   💰 Surebet profit: {:.2}%", profit);
                } else {
                    println!("   ❌ No surebet (margin > 0)");
                }
                println!();
            }
        } else if has_1 || has_x || has_2 {
            count_1x2_both += 1;
        }
    }

    println!("\n📈 Summary:");
    println!("  Matches with SOME 1X2 from 2+ BKs: {}", count_1x2_both);
    println!("  Matches with FULL 1X2 (1+X+2): {}", count_full_1x2);
    println!("\nIf count_full_1x2 > 0 but no surebets found, then:");
    println!("  → No real arbitrage opportunities exist right now");
    println!("  → Bookmakers have efficient pricing");

    Ok(())
}
