// Проверка fingerprint matching между БК
use std::collections::HashMap;
use std::sync::Arc;
use engine::normalizer::Normalizer;
use parsers::parser_factory::ParserFactory;

fn normalize_team_name(name: &str) -> String {
    let mut s = name.to_lowercase()
        .replace("фк ", "").replace("ск ", "").replace("пк ", "")
        .replace("фк", "").replace("ск", "").replace("пк", "")
        .replace("хк ", "").replace("хк", "")
        .replace(" москва", "").replace(" спб", "")
        .replace(" санкт-петербург", "").replace(" с.-петербург", "")
        .replace(" петербург", "").replace(" питер", "")
        .replace("(", "").replace(")", "").replace("-", " ")
        .replace(".", "").replace(",", "").replace("'", "")
        .replace("\"", "").replace("_", " ")
        .trim().to_string();
    while s.contains("  ") { s = s.replace("  ", " "); }
    s.trim().to_string()
}

fn event_fingerprint(event: &shared::Event) -> String {
    let norm = Normalizer::new();
    let norm_event = norm.normalize_event(event.clone());
    let home = normalize_team_name(&norm_event.home_team);
    let away = normalize_team_name(&norm_event.away_team);
    let league = norm_event.league.to_lowercase().replace(" ", "");
    let (first, second) = if home < away { (home, away) } else { (away, home) };
    format!("{:?}|{}|{}|{}", event.sport, league, first, second)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🔍 Checking REAL fingerprint matching between BKs...\n");
    
    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent("Mozilla/5.0")
            .build().expect("Failed client")
    );
    
    let factory = ParserFactory::new(http_client.clone());
    let parsers = factory.get_enabled();
    println!("{} parsers loaded\n", parsers.len());
    
    // Fetch 2 parsers only (faster)
    let parser_a = parsers.iter().find(|p| p.slug() == "pari").unwrap();
    let parser_b = parsers.iter().find(|p| p.slug() == "bettery").unwrap();
    
    println!("Fetching {}...", parser_a.slug());
    let result_a = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        parser_a.fetch_all()
    ).await.unwrap().unwrap();
    
    println!("Fetching {}...", parser_b.slug());
    let result_b = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        parser_b.fetch_all()
    ).await.unwrap().unwrap();
    
    println!("\n📊 {}: {} events, {} odds", parser_a.slug(), result_a.events.len(), result_a.odds.len());
    println!("📊 {}: {} events, {} odds\n", parser_b.slug(), result_b.events.len(), result_b.odds.len());
    
    // Build fingerprints
    let norm = Normalizer::new();
    
    let mut fp_to_events_a: HashMap<String, Vec<&shared::Event>> = HashMap::new();
    for ev in &result_a.events {
        let norm_ev = norm.normalize_event(ev.clone());
        let fp = event_fingerprint(&norm_ev);
        fp_to_events_a.entry(fp).or_default().push(ev);
    }
    
    let mut fp_to_events_b: HashMap<String, Vec<&shared::Event>> = HashMap::new();
    for ev in &result_b.events {
        let norm_ev = norm.normalize_event(ev.clone());
        let fp = event_fingerprint(&norm_ev);
        fp_to_events_b.entry(fp).or_default().push(ev);
    }
    
    // Count matches
    let mut matched = 0;
    let mut total_a = fp_to_events_a.len();
    
    for fp in fp_to_events_a.keys() {
        if fp_to_events_b.contains_key(fp) {
            matched += 1;
            if matched <= 3 {
                let events_a = &fp_to_events_a[fp];
                let events_b = &fp_to_events_b[fp];
                println!("✅ Match #{}: FP = {}", matched, fp);
                println!("   {}: {} events", parser_a.slug(), events_a.len());
                println!("   {}: {} events", parser_b.slug(), events_b.len());
                
                // Show sample
                if let Some(ea) = events_a.first() {
                    println!("   {}: {} vs {}", ea.bookmaker_slug, ea.home_team, ea.away_team);
                }
                if let Some(eb) = events_b.first() {
                    println!("   {}: {} vs {}", eb.bookmaker_slug, eb.home_team, eb.away_team);
                }
                println!();
            }
        }
    }
    
    println!("📈 Matching: {}/{} ({:.1}%) events from {} match with {}", 
             matched, total_a, 
             if total_a > 0 { matched as f64 / total_a as f64 * 100.0 } else { 0.0 },
             parser_a.slug(), parser_b.slug());
    
    Ok(())
}
