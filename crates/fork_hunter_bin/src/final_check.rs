// Финальный диагностический тест — ищет РЕАЛЬНЫЕ вилки между БК
// cargo run --bin final_check 2>&1 | more

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
    println!("🔍 FINAL CHECK: Searching REAL surebets between BKs\n");

    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0")
            .build()
            .expect("Failed client"),
    );

    let factory = ParserFactory::new(http_client.clone());
    let parsers = factory.get_enabled();
    println!("{} parsers loaded\n", parsers.len());

    // Fetch ALL parsers
    let mut all_events: Vec<shared::Event> = Vec::new();
    let mut all_odds: Vec<shared::Odd> = Vec::new();

    for parser in &parsers {
        print!("Fetching {}... ", parser.slug());
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(45), parser.fetch_all()).await;

        match result {
            Ok(Ok(r)) => {
                println!("{} events, {} odds", r.events.len(), r.odds.len());
                all_events.extend(r.events);
                all_odds.extend(r.odds);
            }
            Ok(Err(e)) => println!("ERROR: {}", e),
            Err(_) => println!("TIMEOUT"),
        }
    }

    println!(
        "\n📊 TOTAL: {} events, {} odds\n",
        all_events.len(),
        all_odds.len()
    );

    // Normalize events
    let norm = Normalizer::new();
    let norm_events: Vec<_> = all_events
        .iter()
        .map(|e| norm.normalize_event(e.clone()))
        .collect();

    // Group by fingerprint
    let mut matches: HashMap<String, Vec<&shared::Event>> = HashMap::new();
    for ev in &norm_events {
        let fp = event_fingerprint(ev);
        matches.entry(fp).or_default().push(ev);
    }

    // Group odds by fingerprint
    let mut event_by_id: HashMap<String, &shared::Event> = HashMap::new();
    for ev in &norm_events {
        event_by_id.insert(ev.id.clone(), ev);
    }

    let mut odds_by_match: HashMap<String, Vec<&shared::Odd>> = HashMap::new();
    for odd in &all_odds {
        if let Some(ev) = event_by_id.get(&odd.event_id) {
            let fp = event_fingerprint(ev);
            odds_by_match.entry(fp).or_default().push(odd);
        }
    }

    let multi_bk: Vec<_> = odds_by_match
        .iter()
        .filter(|(_, odds)| {
            let bks: std::collections::HashSet<_> =
                odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
            bks.len() >= 2
        })
        .collect();

    println!(
        "🔗 Matched: {} total, {} with 2+ BKs\n",
        matches.len(),
        multi_bk.len()
    );

    // Calculator with 0.1% min
    let calc = SurebetCalculator::new(0.1, 30.0, 1000.0, 100000, 0.01);

    // Search surebets
    let mut total_surebets = 0;
    let mut checked = 0;

    for (fp, odds) in &odds_by_match {
        if odds.len() < 3 {
            continue;
        }

        let bks: std::collections::HashSet<_> =
            odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
        if bks.len() < 2 {
            continue;
        }

        checked += 1;

        // Group by market
        let mut by_market: HashMap<String, Vec<&&shared::Odd>> = HashMap::new();
        for odd in odds {
            let line_key = odd.line.map(|l| format!("{:.1}", l)).unwrap_or_default();
            let key = format!("{}|{}", odd.market.to_lowercase(), line_key);
            by_market.entry(key).or_default().push(odd);
        }

        for (market, market_odds) in &by_market {
            let lower = market.to_lowercase();

            if lower.starts_with("1x2") || lower.starts_with("исход") || lower.starts_with("match")
            {
                let ones: Vec<_> = market_odds
                    .iter()
                    .filter(|o| o.selection == "1" || o.selection.to_lowercase() == "п1")
                    .cloned()
                    .collect();
                let xs: Vec<_> = market_odds
                    .iter()
                    .filter(|o| o.selection == "X" || o.selection.to_lowercase() == "х")
                    .cloned()
                    .collect();
                let twos: Vec<_> = market_odds
                    .iter()
                    .filter(|o| o.selection == "2" || o.selection.to_lowercase() == "п2")
                    .cloned()
                    .collect();

                if ones.is_empty() || xs.is_empty() || twos.is_empty() {
                    continue;
                }

                for &o1 in &ones {
                    for &ox in &xs {
                        for &o2 in &twos {
                            let bks: std::collections::HashSet<_> = [
                                o1.bookmaker_slug.as_str(),
                                ox.bookmaker_slug.as_str(),
                                o2.bookmaker_slug.as_str(),
                            ]
                            .iter()
                            .cloned()
                            .collect();
                            if bks.len() < 2 {
                                continue;
                            }

                            if let Some(profit) =
                                shared::odds::calculate_surebet_profit(&[o1.odds, ox.odds, o2.odds])
                            {
                                if profit < 0.1 {
                                    continue;
                                }

                                total_surebets += 1;
                                if total_surebets <= 5 {
                                    println!(
                                        "💰 SUREBET #{}: {:.2}% profit",
                                        total_surebets, profit
                                    );
                                    println!("   Match: {}", fp);
                                    println!(
                                        "   1: {} @{:.2} ({})",
                                        o1.bookmaker_slug, o1.odds, o1.selection
                                    );
                                    println!(
                                        "   X: {} @{:.2} ({})",
                                        ox.bookmaker_slug, ox.odds, ox.selection
                                    );
                                    println!(
                                        "   2: {} @{:.2} ({})",
                                        o2.bookmaker_slug, o2.odds, o2.selection
                                    );
                                    println!();
                                }
                            }
                        }
                    }
                }
            }

            if lower.starts_with("total") || lower.starts_with("тотал") {
                let overs: Vec<_> = market_odds
                    .iter()
                    .filter(|o| {
                        o.selection.to_lowercase().contains("over")
                            || o.selection.to_lowercase().contains("больше")
                            || o.selection.to_lowercase() == "тб"
                    })
                    .cloned()
                    .collect();
                let unders: Vec<_> = market_odds
                    .iter()
                    .filter(|o| {
                        o.selection.to_lowercase().contains("under")
                            || o.selection.to_lowercase().contains("меньше")
                            || o.selection.to_lowercase() == "тм"
                    })
                    .cloned()
                    .collect();

                for &o_over in &overs {
                    for &o_under in &unders {
                        if o_over.bookmaker_slug == o_under.bookmaker_slug {
                            continue;
                        }

                        if let (Some(l1), Some(l2)) = (o_over.line, o_under.line) {
                            if (l1 - l2).abs() > 0.1 {
                                continue;
                            }
                        }

                        if let Some(profit) =
                            shared::odds::calculate_surebet_profit(&[o_over.odds, o_under.odds])
                        {
                            if profit < 0.1 {
                                continue;
                            }

                            total_surebets += 1;
                            if total_surebets <= 5 {
                                println!(
                                    "💰 SUREBET #{} (Total): {:.2}% profit",
                                    total_surebets, profit
                                );
                                println!("   Match: {}", fp);
                                println!(
                                    "   Over: {} @{:.2} line={:?}",
                                    o_over.bookmaker_slug, o_over.odds, o_over.line
                                );
                                println!(
                                    "   Under: {} @{:.2} line={:?}",
                                    o_under.bookmaker_slug, o_under.odds, o_under.line
                                );
                                println!();
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n📈 RESULTS:");
    println!("  Checked matches: {}", checked);
    println!("  Surebets found: {}", total_surebets);

    if total_surebets == 0 {
        println!("\n❌ No surebets found. Checking market coverage...");

        // Check what markets we actually have
        let mut market_stats: HashMap<String, (usize, usize)> = HashMap::new(); // (count, multi_bk_count)
        for (fp, odds) in &odds_by_match {
            let bks: std::collections::HashSet<_> =
                odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
            let is_multi = bks.len() >= 2;

            let mut by_market: HashMap<String, Vec<&&shared::Odd>> = HashMap::new();
            for odd in odds {
                let line_key = odd.line.map(|l| format!("{:.1}", l)).unwrap_or_default();
                let key = format!("{}|{}", odd.market.to_lowercase(), line_key);
                by_market.entry(key).or_default().push(odd);
            }

            for (market, market_odds) in &by_market {
                let (count, multi_count) = market_stats.entry(market.clone()).or_insert((0, 0));
                *count += 1;
                if is_multi {
                    *multi_count += 1;
                }
            }
        }

        println!("\nMarket coverage:");
        let mut stats_vec: Vec<_> = market_stats.iter().collect();
        stats_vec.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        for (market, (count, multi)) in stats_vec.iter().take(15) {
            let pct = if *count > 0 {
                (*multi as f64 / *count as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  {:30} {:5} matches ({:5} multi-BK, {:.0}%)",
                market, count, multi, pct
            );
        }
    }

    Ok(())
}
