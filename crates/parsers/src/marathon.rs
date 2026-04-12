use crate::base::{BookmakerParser, ParserResult};
use crate::factors_catalog::FactorsCatalog;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Marathon API — shared platform (*-resources.com, scopeMarket=3000)
/// Загружает факторы из каталога для динамического обнаружения рынков
#[derive(Debug)]
pub struct MarathonParser {
    http_client: Arc<Client>,
    live_url: String,
    prematch_url: String,
    factors: Arc<FactorsCatalog>,
    api_base: String,
}

impl MarathonParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            http_client: client.clone(),
            live_url: "https://www.marathonbet.ru/su/live".to_string(),
            prematch_url: "https://www.marathonbet.ru/su/line".to_string(),
            factors: Arc::new(FactorsCatalog::new(
                client.clone(),
                "https://line51.tf39be-resources.com",
                3000,
            )),
            api_base: "https://line51.tf39be-resources.com".to_string(),
        }
    }

    pub async fn load_factors(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        self.factors.load().await
    }

    /// Fetch events and odds from HTTP API
    async fn fetch_via_browser(
        &self,
        _url: &str,
        is_live: bool,
    ) -> Result<Vec<(Event, Vec<Odd>)>, Box<dyn std::error::Error + Send + Sync>> {
        let scope = "3000";
        let suffix = if is_live {
            "events/list"
        } else {
            "events/listBase"
        };
        let url = format!("{}/{}?lang=ru&scopeMarket={}", self.api_base, suffix, scope);

        eprintln!("[MARATHON] Creating new client for {}", url);

        // Create a fresh client for each request to avoid connection pool issues
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .gzip(true)
            .build()?;

        eprintln!("[MARATHON] Sending request to {}", url);
        let resp = client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .send()
            .await?;

        eprintln!("[MARATHON] Response received: {}", resp.status());

        if !resp.status().is_success() {
            eprintln!("[MARATHON] HTTP failed: {}", resp.status());
            return Ok(Vec::new());
        }

        eprintln!("[MARATHON] Parsing JSON...");
        let json: serde_json::Value = resp.json().await?;
        eprintln!("[MARATHON] JSON parsed, extracting events...");
        let result = parse_api_response(&json, is_live, "marathon", &self.factors);
        eprintln!("[MARATHON] Fetch complete");
        result
    }
}

#[async_trait]
impl BookmakerParser for MarathonParser {
    fn name(&self) -> &str {
        "Marathon"
    }
    fn slug(&self) -> &str {
        "marathon"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();

        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_via_browser(url, is_live).await {
                Ok(results) => {
                    for (event, _) in results {
                        all_events.push(event);
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Marathon fetch events failed");
                }
            }
        }

        info!(count = all_events.len(), "Marathon events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();

        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_via_browser(url, is_live).await {
                Ok(results) => {
                    for (_, odds) in results {
                        all_odds.extend(odds);
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Marathon fetch odds failed");
                }
            }
        }

        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        // Загружаем каталог факторов
        match self.factors.load().await {
            Ok(count) => debug!(factor_count = count, "Factors catalog loaded for Marathon"),
            Err(e) => warn!(error = %e, "Failed to load factors catalog"),
        }

        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        for (url, is_live) in [(&self.live_url, true), (&self.prematch_url, false)] {
            match self.fetch_via_browser(url, is_live).await {
                Ok(results) => {
                    for (event, odds) in results {
                        all_events.push(event);
                        all_odds.extend(odds);
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Marathon fetch failed");
                }
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Marathon fetch complete"
        );
        Ok(ParserResult::new("marathon", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://www.marathonbet.ru"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
}

/// HTTP fallback — если headless Chrome недоступен
pub async fn fetch_platform_http(
    client: &Client,
    base_url: &str,
    is_live: bool,
    slug: &str,
    factors: &FactorsCatalog,
) -> Result<Vec<(Event, Vec<Odd>)>, Box<dyn std::error::Error + Send + Sync>> {
    let suffix = if is_live {
        "events/list"
    } else {
        "events/listBase"
    };
    let scope = match slug {
        "pari" => "2300",
        "marathon" => "3000",
        "bettery" => "501",
        "zenit" => "1300",
        _ => "3000",
    };
    let url = format!("{}/{}?lang=ru&scopeMarket={}", base_url, suffix, scope);

    debug!(url, "HTTP fetch");
    let resp = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "ru-RU,ru;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("Origin", format!("https://{}.ru", slug))
        .header("Referer", format!("https://{}.ru/", slug))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !resp.status().is_success() {
        warn!(status = %resp.status(), slug, "HTTP fetch failed");
        return Ok(Vec::new());
    }

    // Get raw bytes and handle decompression manually
    let bytes = resp.bytes().await?;
    debug!(slug, bytes = bytes.len(), "HTTP response size");

    // Decompress if needed
    let json_bytes = if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        // Gzip compressed
        use flate2::read::GzDecoder;
        use std::io::Read;
        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_ok() {
            debug!(slug, decompressed = decompressed.len(), "Gzip decompressed");
            decompressed
        } else {
            bytes.to_vec()
        }
    } else {
        bytes.to_vec()
    };

    let json: serde_json::Value = match serde_json::from_slice(&json_bytes) {
        Ok(j) => j,
        Err(e) => {
            warn!(error = %e, slug, "JSON parse failed, first 200 bytes: {}", String::from_utf8_lossy(&json_bytes[..json_bytes.len().min(200)]));
            return Ok(Vec::new());
        }
    };

    parse_api_response(&json, is_live, slug, factors)
}

/// Парсит JSON ответ API shared platform
pub fn parse_api_response(
    json: &serde_json::Value,
    is_live: bool,
    slug: &str,
    factors: &FactorsCatalog,
) -> Result<Vec<(Event, Vec<Odd>)>, Box<dyn std::error::Error + Send + Sync>> {
    let events = match json.get("events").and_then(|e| e.as_array()) {
        Some(e) => e,
        None => return Ok(Vec::new()),
    };

    let custom_factors = match json.get("customFactors").and_then(|f| f.as_array()) {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };

    // Build factor lookup
    let mut factor_map: HashMap<u64, Vec<(u64, f64, Option<f64>)>> = HashMap::new();
    for factor_entry in custom_factors {
        if let (Some(event_id), Some(factors_arr)) = (
            factor_entry.get("e").and_then(|e| e.as_u64()),
            factor_entry.get("factors").and_then(|f| f.as_array()),
        ) {
            for f in factors_arr {
                if let (Some(fid), Some(fval)) = (
                    f.get("f").and_then(|x| x.as_u64()),
                    f.get("v").and_then(|x| x.as_f64()),
                ) {
                    let line = f
                        .get("p")
                        .and_then(|x| x.as_f64())
                        .or_else(|| f.get("pt").and_then(|x| x.as_f64()));
                    factor_map
                        .entry(event_id)
                        .or_default()
                        .push((fid, fval, line));
                }
            }
        }
    }

    let mut results = Vec::new();

    for event_data in events {
        let (Some(event_id), Some(team1), Some(team2)) = (
            event_data.get("id").and_then(|i| i.as_u64()),
            event_data.get("team1").and_then(|t| t.as_str()),
            event_data.get("team2").and_then(|t| t.as_str()),
        ) else {
            continue;
        };

        if team1.is_empty() || team2.is_empty() {
            continue;
        }

        let event_name = event_data
            .get("name")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let sport = detect_sport(event_name);
        let league = extract_league(event_name);

        let event = Event {
            id: format!("{}-{}", slug, event_id),
            sport,
            league,
            home_team: team1.to_string(),
            away_team: team2.to_string(),
            start_time: None,
            is_live,
            bookmaker_slug: slug.to_string(),
            raw_url: None,
            extra: HashMap::new(),
        };

        let mut odds = Vec::new();
        if let Some(factors_list) = factor_map.get(&event_id) {
            let now = Utc::now();
            for &(fid, val, line) in factors_list {
                if val <= 1.0 {
                    continue;
                }

                if let Some(factor_def) = factors.get_factor(fid) {
                    let mt = factor_def.market_type.to_lowercase();
                    let sel = factor_def.selection_name.to_lowercase();
                    if classify_and_add_odd(&mut odds, slug, &event, fid, val, line, &mt, &sel, now)
                    {
                        continue;
                    }
                }

                // Hardcoded fallback
                match fid {
                    921 => odds.push(make_odd(
                        slug,
                        &event,
                        "1X2",
                        "1",
                        val,
                        None,
                        OddsType::Home,
                        now,
                    )),
                    922 => odds.push(make_odd(
                        slug,
                        &event,
                        "1X2",
                        "X",
                        val,
                        None,
                        OddsType::Draw,
                        now,
                    )),
                    923 => odds.push(make_odd(
                        slug,
                        &event,
                        "1X2",
                        "2",
                        val,
                        None,
                        OddsType::Away,
                        now,
                    )),
                    924 | 1002 | 1010 | 1054 => {
                        if let Some(l) = line {
                            odds.push(make_odd(
                                slug,
                                &event,
                                "Total",
                                "Over",
                                val,
                                Some(l),
                                OddsType::Over,
                                now,
                            ));
                        }
                    }
                    925 | 1003 | 1011 | 1055 => {
                        if let Some(l) = line {
                            odds.push(make_odd(
                                slug,
                                &event,
                                "Total",
                                "Under",
                                val,
                                Some(l),
                                OddsType::Under,
                                now,
                            ));
                        }
                    }
                    1006 | 1004 | 1005 | 1012 | 1013 => {
                        if let Some(l) = line {
                            let sel = if l > 0.0 { "1" } else { "2" };
                            odds.push(make_odd(
                                slug,
                                &event,
                                "Handicap",
                                sel,
                                val,
                                Some(l),
                                OddsType::Handicap,
                                now,
                            ));
                        }
                    }
                    926 => odds.push(make_odd(
                        slug,
                        &event,
                        "BothTeamsScore",
                        "Yes",
                        val,
                        None,
                        OddsType::BothTeamsScoreYes,
                        now,
                    )),
                    927 => odds.push(make_odd(
                        slug,
                        &event,
                        "BothTeamsScore",
                        "No",
                        val,
                        None,
                        OddsType::BothTeamsScoreNo,
                        now,
                    )),
                    928 => odds.push(make_odd(
                        slug,
                        &event,
                        "EvenOdd",
                        "Even",
                        val,
                        None,
                        OddsType::Even,
                        now,
                    )),
                    929 => odds.push(make_odd(
                        slug,
                        &event,
                        "EvenOdd",
                        "Odd",
                        val,
                        None,
                        OddsType::Odd,
                        now,
                    )),
                    _ => {}
                }
            }
        }

        results.push((event, odds));
    }

    Ok(results)
}

fn classify_and_add_odd(
    odds: &mut Vec<Odd>,
    slug: &str,
    event: &Event,
    _fid: u64,
    val: f64,
    line: Option<f64>,
    market_type: &str,
    selection: &str,
    now: chrono::DateTime<Utc>,
) -> bool {
    if market_type.contains("1x2")
        || market_type.contains("исход")
        || market_type.contains("winner")
    {
        if selection.contains("п1") || selection == "1" || selection.contains("home") {
            odds.push(make_odd(
                slug,
                event,
                "1X2",
                "1",
                val,
                None,
                OddsType::Home,
                now,
            ));
            return true;
        } else if selection.contains("х") || selection == "x" || selection.contains("draw") {
            odds.push(make_odd(
                slug,
                event,
                "1X2",
                "X",
                val,
                None,
                OddsType::Draw,
                now,
            ));
            return true;
        } else if selection.contains("п2") || selection == "2" || selection.contains("away") {
            odds.push(make_odd(
                slug,
                event,
                "1X2",
                "2",
                val,
                None,
                OddsType::Away,
                now,
            ));
            return true;
        }
    } else if market_type.contains("total") || market_type.contains("тотал") {
        if let Some(l) = line {
            if selection.contains("больше")
                || selection.contains("over")
                || selection.contains("тб")
            {
                odds.push(make_odd(
                    slug,
                    event,
                    "Total",
                    "Over",
                    val,
                    Some(l),
                    OddsType::Over,
                    now,
                ));
                return true;
            } else if selection.contains("меньше")
                || selection.contains("under")
                || selection.contains("тм")
            {
                odds.push(make_odd(
                    slug,
                    event,
                    "Total",
                    "Under",
                    val,
                    Some(l),
                    OddsType::Under,
                    now,
                ));
                return true;
            }
        }
    } else if market_type.contains("handicap") || market_type.contains("фора") {
        if let Some(l) = line {
            let sel = if l > 0.0 { "1" } else { "2" };
            odds.push(make_odd(
                slug,
                event,
                "Handicap",
                sel,
                val,
                Some(l),
                OddsType::Handicap,
                now,
            ));
            return true;
        }
    } else if market_type.contains("both")
        || market_type.contains("обе")
        || market_type.contains("oz")
        || market_type.contains("btts")
    {
        if selection.contains("да") || selection.contains("yes") {
            odds.push(make_odd(
                slug,
                event,
                "BothTeamsScore",
                "Yes",
                val,
                None,
                OddsType::BothTeamsScoreYes,
                now,
            ));
            return true;
        } else if selection.contains("нет") || selection.contains("no") {
            odds.push(make_odd(
                slug,
                event,
                "BothTeamsScore",
                "No",
                val,
                None,
                OddsType::BothTeamsScoreNo,
                now,
            ));
            return true;
        }
    } else if market_type.contains("even")
        || market_type.contains("odd")
        || market_type.contains("чёт")
        || market_type.contains("нечет")
    {
        if selection.contains("чёт") || selection.contains("even") {
            odds.push(make_odd(
                slug,
                event,
                "EvenOdd",
                "Even",
                val,
                None,
                OddsType::Even,
                now,
            ));
            return true;
        } else if selection.contains("нечет") || selection.contains("odd") {
            odds.push(make_odd(
                slug,
                event,
                "EvenOdd",
                "Odd",
                val,
                None,
                OddsType::Odd,
                now,
            ));
            return true;
        }
    } else if market_type.contains("double") || market_type.contains("двойн") {
        odds.push(make_odd(
            slug,
            event,
            "DoubleChance",
            selection,
            val,
            None,
            OddsType::Custom,
            now,
        ));
        return true;
    } else if market_type.contains("correct")
        || market_type.contains("точн")
        || market_type.contains("score")
    {
        odds.push(make_odd(
            slug,
            event,
            "CorrectScore",
            selection,
            val,
            None,
            OddsType::Custom,
            now,
        ));
        return true;
    }
    false
}

fn detect_sport(event_name: &str) -> Sport {
    let name = event_name.to_lowercase();
    if name.contains("футбол") || name.contains("football") {
        Sport::Football
    } else if name.contains("баскет") || name.contains("basket") {
        Sport::Basketball
    } else if name.contains("хоккей") || name.contains("hockey") {
        Sport::Hockey
    } else if name.contains("теннис") || name.contains("tennis") {
        Sport::Tennis
    } else if name.contains("волейбол") || name.contains("volley") {
        Sport::Volleyball
    } else {
        Sport::Football
    }
}

fn extract_league(event_name: &str) -> String {
    event_name
        .splitn(2, ':')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn make_odd(
    slug: &str,
    event: &Event,
    market: &str,
    selection: &str,
    odds: f64,
    line: Option<f64>,
    odds_type: OddsType,
    timestamp: chrono::DateTime<Utc>,
) -> Odd {
    Odd {
        id: format!(
            "{}-{}-{}",
            slug,
            event.id,
            selection.replace(['.', ' ', '/'], "_")
        ),
        event_id: event.id.clone(),
        bookmaker_slug: slug.to_string(),
        market: market.into(),
        selection: selection.into(),
        odds,
        odds_type,
        line,
        timestamp,
    }
}
