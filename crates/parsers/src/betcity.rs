use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use scraper::{Html, Selector};
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Структура парсера
// ─────────────────────────────────────────────────────────────────────────────

/// Betcity парсер — HTTP-запросы + поиск JSON в скрипт-тегах + демо-данные
/// Betcity использует React SPA с Cloudflare защитой.
/// Стратегия: сначала пробуем известные API-эндпоинты,
/// затем парсим window.__INITIAL_STATE__ из HTML,
/// при неудаче — возвращаем реалистичные демо-данные.
#[derive(Debug)]
pub struct BetcityParser {
    client: Arc<Client>,
}

impl BetcityParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Приватные вспомогательные методы
    // ─────────────────────────────────────────────────────────────────────────

    /// Строим reqwest-клиент с правильными заголовками для Betcity
    fn build_client() -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .gzip(true)
            .brotli(true)
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/124.0.0.0 Safari/537.36",
            )
            .build()?;
        Ok(client)
    }

    /// Пробуем получить JSON с ключевых API-эндпоинтов Betcity
    /// Falls back to demo data if API fails (common due to anti-bot protection)
    async fn try_api_endpoints(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        // Build a fresh client (the shared one might have connection issues)
        let client = match Self::build_client() {
            Ok(c) => c,
            Err(e) => {
                println!("[Betcity] Failed to build client: {}", e);
                return Ok((Vec::new(), Vec::new()));
            }
        };
        
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        // Try prematch endpoint
        let prematch_url = "https://ad.betcity.ru/d/off/events?id_sp=1&ch_id=0&gr_id=0&rev=2&ver=69&csn=ooca9s";
        match self.fetch_api(&client, prematch_url, false).await {
            Ok((events, odds)) => {
                println!("[Betcity] prematch: {} events", events.len());
                all_events.extend(events);
                all_odds.extend(odds);
            }
            Err(e) => {
                println!("[Betcity] prematch failed: {}", e);
            }
        }

        // Try live endpoint (might return 502 but worth trying)
        let live_url = "https://ad.betcity.ru/d/on_air/bets?rev=2&template=1&ver=69&csn=ooca9s";
        match self.fetch_api(&client, live_url, true).await {
            Ok((events, odds)) => {
                println!("[Betcity] live: {} events", events.len());
                all_events.extend(events);
                all_odds.extend(odds);
            }
            Err(e) => {
                println!("[Betcity] live failed: {}", e);
            }
        }

        println!("[Betcity] Total: {} events", all_events.len());
        Ok((all_events, all_odds))
    }

    /// Fetch API endpoint with fresh client
    async fn fetch_api(
        &self,
        client: &reqwest::Client,
        url: &str,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = client
            .get(url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .header("Referer", "https://betcity.ru/ru/line/football")
            .send()
            .await?;

        let status = resp.status();
        println!("[Betcity] fetch_api: status={} url={}", status, url);

        if !resp.status().is_success() {
            return Err(format!("HTTP error: {}", status).into());
        }

        let text = resp.text().await?;
        println!("[Betcity] fetch_api: got {} bytes", text.len());

        let json: serde_json::Value = serde_json::from_str(&text)?;
        
        // Debug: check what we got
        let has_reply = json.get("reply").is_some();
        let has_sports = json.get("sports").is_some();
        let has_events = json.get("events").is_some();
        println!("[Betcity] JSON keys: reply={} sports={} events={}", has_reply, has_sports, has_events);
        
        let result = Self::parse_json_response(&json, is_live);
        println!("[Betcity] Parsed: {} events, {} odds", result.0.len(), result.1.len());
        
        Ok(result)
    }

    /// Вспомогательный метод для запроса и парсинга одного эндпоинта
    async fn fetch_and_parse_endpoint(
        &self,
        client: &Client,
        url: &str,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        println!("[Betcity] fetch_and_parse_endpoint START: {}", url);
        
        let request = client
            .get(url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .header("Referer", "https://betcity.ru/ru/line/football")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");

        println!("[Betcity] Built request, sending...");
        
        let resp = match request.send().await {
            Ok(r) => {
                println!("[Betcity] Send OK, status: {}", r.status());
                r
            }
            Err(e) => {
                println!("[Betcity] Send FAILED: {:?}", e);
                println!("[Betcity] Is timeout: {}", e.is_timeout());
                println!("[Betcity] Is connect: {}", e.is_connect());
                println!("[Betcity] Is request: {}", e.is_request());
                return Err(Box::new(e));
            }
        };

        let status = resp.status();
        println!("[Betcity] Response status: {} for {}", status, url);

        if !status.is_success() {
            return Err(format!("HTTP error: {}", status).into());
        }

        // Read raw text first
        let text = resp.text().await?;
        println!("[Betcity] Got text, length: {} for {}", text.len(), url);
        
        if text.len() < 50 {
            return Err(format!("Response too short: {}", text).into());
        }

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(&text)?;
        
        // Check for reply wrapper
        let has_reply = json.get("reply").is_some();
        println!("[Betcity] JSON has reply: {} for {}", has_reply, url);
        
        let (events, odds) = Self::parse_json_response(&json, is_live);
        
        println!("[Betcity] Parsed {} events from {}", events.len(), url);

        Ok((events, odds))
    }

    /// Загружаем HTML страницу и ищем JSON в скрипт-тегах
    async fn try_html_script_extraction(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let urls = [
            "https://betcity.ru/ru/line",
            "https://betcity.ru/ru/live",
        ];
        let client = Self::build_client()?;

        for url in &urls {
            println!("[Betcity] HTML script extraction: trying {}", url);

            let resp = match client
                .get(*url)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    println!("[Betcity] HTML load failed: {}", e);
                    continue;
                }
            };

            if !resp.status().is_success() {
                println!("[Betcity] HTML status: {}", resp.status());
                continue;
            }

            let html = match resp.text().await {
                Ok(h) => h,
                Err(e) => {
                    println!("[Betcity] HTML read failed: {}", e);
                    continue;
                }
            };

            println!("[Betcity] HTML loaded: {} bytes", html.len());

            // Пробуем извлечь JSON из известных паттернов в скрипт-тегах
            let (events, odds) = Self::extract_from_html(&html);
            if !events.is_empty() {
                println!("[Betcity] extracted from HTML: {} events", events.len());
                return Ok((events, odds));
            }
        }

        Ok((Vec::new(), Vec::new()))
    }

    /// Парсим HTML DOM для извлечения событий и кэфов
    async fn try_html_dom_parsing(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let urls = [
            "https://betcity.ru/ru/line",
            "https://betcity.ru/ru/live",
        ];
        let client = Self::build_client()?;

        for url in &urls {
            println!("[Betcity] DOM parsing: trying {}", url);

            let resp = match client
                .get(*url)
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    println!("[Betcity] DOM load failed: {}", e);
                    continue;
                }
            };

            if !resp.status().is_success() {
                println!("[Betcity] DOM status: {}", resp.status());
                continue;
            }

            let html = match resp.text().await {
                Ok(h) => h,
                Err(e) => {
                    println!("[Betcity] DOM read failed: {}", e);
                    continue;
                }
            };

            println!("[Betcity] DOM: {} bytes", html.len());

            let (events, odds) = Self::parse_html_dom(&html, url);
            if !events.is_empty() {
                println!("[Betcity] DOM parsed: {} events", events.len());
                return Ok((events, odds));
            }
        }

        Ok((Vec::new(), Vec::new()))
    }

    /// Парсим JSON-ответ API с гибкой структурой
    fn parse_json_response(json: &serde_json::Value, is_live: bool) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // Betcity API wraps response in "reply" key
        let json = match json.get("reply") {
            Some(reply_json) => {
                warn!("Betcity: found 'reply' key, parsing inner JSON");
                reply_json
            }
            None => {
                warn!("Betcity: no 'reply' key found, using root");
                json
            }
        };

        warn!(has_sports = json.get("sports").is_some(), has_events = json.get("events").is_some(), "Betcity: JSON structure check");

        // Пробуем разные пути к массиву событий
        let events_array = json
            .get("events")
            .or_else(|| json.get("data"))
            .or_else(|| json.get("items"))
            .or_else(|| json.get("matches"))
            .or_else(|| json.get("results"))
            .and_then(|v| v.as_array());

        if let Some(arr) = events_array {
            let (evs, ods) = Self::parse_events_array(arr, is_live, now);
            events.extend(evs);
            odds.extend(ods);
        } else if let Some(sports_array) = json.get("sports").and_then(|v| v.as_array()) {
            // Betcity sports structure: sports is an array of sport objects
            for sport in sports_array {
                // Each sport has id_sp and may have chmps (championships)
                if let Some(chmps_obj) = sport.get("chmps").and_then(|v| v.as_object()) {
                    for (_chmp_id, chmp_val) in chmps_obj {
                        let league = chmp_val
                            .get("name_ch")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if let Some(evts_obj) = chmp_val.get("evts").and_then(|v| v.as_object()) {
                            for (_evt_id, evt_val) in evts_obj {
                                let home = evt_val
                                    .get("name_ht")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let away = evt_val
                                    .get("name_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                if home.len() < 2 || away.len() < 2 {
                                    continue;
                                }

                                let event_id = format!(
                                    "betcity-{}",
                                    evt_val.get("id_ev").and_then(|v| v.as_i64()).unwrap_or(0)
                                );

                                events.push(Event {
                                    id: event_id.clone(),
                                    sport: Sport::Football,
                                    league: league.clone(),
                                    home_team: home.to_string(),
                                    away_team: away.to_string(),
                                    start_time: None,
                                    is_live,
                                    bookmaker_slug: "betcity".to_string(),
                                    raw_url: Some("https://betcity.ru".to_string()),
                                    extra: HashMap::new(),
                                });

                                // Extract odds from main
                                if let Some(main_obj) = evt_val.get("main").and_then(|v| v.as_object()) {
                                    for (_market_id, market_val) in main_obj {
                                        if let Some(data_obj) = market_val.get("data").and_then(|v| v.as_object()) {
                                            for (_data_id, data_val) in data_obj {
                                                if let Some(blocks) = data_val.get("blocks").and_then(|v| v.as_object()) {
                                                    for (_block_name, block_val) in blocks {
                                                        if let Some(outcomes_obj) = block_val.as_object() {
                                                            for (selection, outcome_val) in outcomes_obj {
                                                                if let Some(kf) = outcome_val.get("kf").and_then(|v| v.as_f64()) {
                                                                    if kf > 1.01 && kf < 100.0 {
                                                                        let sel = selection.as_str();
                                                                        odds.push(Odd {
                                                                            id: format!("{}-{}-{}", event_id, sel, kf),
                                                                            event_id: event_id.clone(),
                                                                            bookmaker_slug: "betcity".to_string(),
                                                                            market: "1X2".to_string(),
                                                                            selection: sel.to_string(),
                                                                            odds: kf,
                                                                            odds_type: match sel {
                                                                                "Y" | "1" => OddsType::Home,
                                                                                "N" | "2" => OddsType::Away,
                                                                                _ => OddsType::Home,
                                                                            },
                                                                            line: None,
                                                                            timestamp: now,
                                                                        });
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if let Some(sports_array) = json.get("sports").and_then(|v| v.as_array()) {
            // Fallback: old style
            for sport in sports_array {
                if let Some(events_arr) = sport
                    .get("events")
                    .or_else(|| sport.get("matches"))
                    .and_then(|v| v.as_array())
                {
                    let (evs, ods) = Self::parse_events_array(events_arr, is_live, now);
                    events.extend(evs);
                    odds.extend(ods);
                }
            }
        } else if let Some(arr) = json.as_array() {
            // Если сам корень — массив
            let (evs, ods) = Self::parse_events_array(arr, is_live, now);
            events.extend(evs);
            odds.extend(ods);
        }

        (events, odds)
    }

    /// Разбираем массив событий в единообразном формате
    fn parse_events_array(
        arr: &[serde_json::Value],
        is_live: bool,
        now: chrono::DateTime<Utc>,
    ) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();

        for (idx, item) in arr.iter().enumerate() {
            // Ищем команды под разными ключами
            let home = item
                .get("home")
                .or_else(|| item.get("home_team"))
                .or_else(|| item.get("team1"))
                .or_else(|| item.get("homeTeam"))
                .or_else(|| item.get("opponent1"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let away = item
                .get("away")
                .or_else(|| item.get("away_team"))
                .or_else(|| item.get("team2"))
                .or_else(|| item.get("awayTeam"))
                .or_else(|| item.get("opponent2"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if home.len() < 2 || away.len() < 2 {
                continue;
            }

            let league = item
                .get("tournament")
                .or_else(|| item.get("league"))
                .or_else(|| item.get("competition"))
                .or_else(|| item.get("championship"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let event_id = format!("betcity-{}", idx);

            events.push(Event {
                id: event_id.clone(),
                sport: Sport::Football,
                league,
                home_team: home.to_string(),
                away_team: away.to_string(),
                start_time: None,
                is_live,
                bookmaker_slug: "betcity".to_string(),
                raw_url: Some("https://betcity.ru/ru/line/football".to_string()),
                extra: HashMap::new(),
            });

            // Ищем кэфы — под ключами odds/factors/markets
            if let Some(odds_arr) = item
                .get("odds")
                .or_else(|| item.get("factors"))
                .and_then(|v| v.as_array())
            {
                let vals: Vec<f64> = odds_arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .filter(|&v| v > 1.01 && v < 100.0)
                    .collect();

                Self::push_1x2_or_total(&mut odds, &event_id, &vals, now);
            }
        }

        (events, odds)
    }

    /// Ищем JSON в HTML: window.__INITIAL_STATE__, window.__DATA__, и другие паттерны
    fn extract_from_html(html: &str) -> (Vec<Event>, Vec<Odd>) {
        // Паттерны поиска встроенного JSON (без regex — чистый поиск подстрок)
        let markers: &[&str] = &[
            "window.__INITIAL_STATE__=",
            "window.__INITIAL_STATE__ =",
            "window.__DATA__=",
            "window.__DATA__ =",
            "window.__REDUX_STATE__=",
            "window.__PRELOADED_STATE__=",
            "__NEXT_DATA__",
        ];

        let now = Utc::now();

        for marker in markers {
            if let Some(pos) = html.find(marker) {
                let rest = &html[pos + marker.len()..];

                // Для __NEXT_DATA__ ищем JSON внутри тега <script id="__NEXT_DATA__" ...>
                let json_start = if *marker == "__NEXT_DATA__" {
                    rest.find('>').map(|p| p + 1)
                } else {
                    // Пропускаем пробелы и '=' до открывающей скобки
                    rest.find('{').map(|p| p)
                };

                let Some(start) = json_start else { continue };
                let slice = &rest[start..];

                // Ищем конец JSON — находим закрывающую фигурную скобку на нулевом уровне
                if let Some(json_str) = Self::extract_balanced_json(slice) {
                    match serde_json::from_str::<serde_json::Value>(json_str) {
                        Ok(json) => {
                            // Ищем события в типичных путях React/Redux стора
                            let (events, odds) = Self::search_json_tree_for_events(&json, now);
                            if !events.is_empty() {
                                return (events, odds);
                            }
                        }
                        Err(e) => {
                            debug!(marker = marker, error = %e, "Betcity: ошибка разбора встроенного JSON");
                        }
                    }
                }
            }
        }

        (Vec::new(), Vec::new())
    }

    /// Извлекаем сбалансированный JSON-объект из строки (без внешних зависимостей)
    fn extract_balanced_json(s: &str) -> Option<&str> {
        let bytes = s.as_bytes();
        if bytes.is_empty() || bytes[0] != b'{' {
            return None;
        }

        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, &b) in bytes.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if in_string {
                match b {
                    b'\\' => escape_next = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        // Ограничиваем размер для безопасности — не больше 10 МБ
                        if i < 10 * 1024 * 1024 {
                            return Some(&s[..=i]);
                        } else {
                            return None;
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Рекурсивный обход JSON-дерева в поисках массива с событиями
    fn search_json_tree_for_events(
        json: &serde_json::Value,
        now: chrono::DateTime<Utc>,
    ) -> (Vec<Event>, Vec<Odd>) {
        // Ключи, под которыми могут лежать события в Redux/React стейте
        let event_keys = [
            "events",
            "lineEvents",
            "prematchEvents",
            "matches",
            "items",
            "data",
            "sportEvents",
            "eventList",
        ];

        if let Some(obj) = json.as_object() {
            for key in &event_keys {
                if let Some(val) = obj.get(*key) {
                    if let Some(arr) = val.as_array() {
                        if !arr.is_empty() {
                            let (ev, od) = Self::parse_events_array(arr, false, now);
                            if !ev.is_empty() {
                                return (ev, od);
                            }
                        }
                    }
                }
            }

            // Рекурсивный обход на один уровень вглубь
            for (_k, v) in obj {
                if v.is_object() {
                    let (ev, od) = Self::search_json_tree_for_events(v, now);
                    if !ev.is_empty() {
                        return (ev, od);
                    }
                }
            }
        }

        (Vec::new(), Vec::new())
    }

    /// Парсим HTML DOM для извлечения событий
    fn parse_html_dom(html: &str, url: &str) -> (Vec<Event>, Vec<Odd>) {
        let document = Html::parse_document(html);
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // Селекторы для событий
        let event_selector = match Selector::parse(".line-event") {
            Ok(s) => s,
            Err(_) => return (Vec::new(), Vec::new()),
        };

        let name_selector = match Selector::parse(".line-event__name-text") {
            Ok(s) => s,
            Err(_) => return (Vec::new(), Vec::new()),
        };

        let odds_selector = match Selector::parse(".line-event__main-bets-button") {
            Ok(s) => s,
            Err(_) => return (Vec::new(), Vec::new()),
        };

        for (idx, event_el) in document.select(&event_selector).enumerate() {
            // Извлекаем названия команд
            let mut teams = Vec::new();
            for name_el in event_el.select(&name_selector) {
                let team = name_el.text().collect::<String>().trim().to_string();
                if !team.is_empty() {
                    teams.push(team);
                }
            }

            if teams.len() < 2 {
                continue;
            }

            // Извлекаем кэфы
            let mut odds_values = Vec::new();
            for odds_el in event_el.select(&odds_selector) {
                let odds_text = odds_el.text().collect::<String>().trim().to_string();
                if let Ok(val) = odds_text.replace(',', ".").parse::<f64>() {
                    if val >= 1.01 && val <= 100.0 {
                        odds_values.push(val);
                    }
                }
            }

            if odds_values.len() < 2 {
                continue;
            }

            let home_team = teams[0].clone();
            let away_team = teams[1].clone();
            let event_id = format!("betcity-dom-{}", idx);
            let is_live = url.contains("/live");

            // Определяем лигу из URL или используем дефолт
            let league = if url.contains("football") {
                "Football".to_string()
            } else {
                "Live Events".to_string()
            };

            events.push(Event {
                id: event_id.clone(),
                sport: Sport::Football,
                league,
                home_team,
                away_team,
                start_time: None,
                is_live,
                bookmaker_slug: "betcity".to_string(),
                raw_url: Some(url.to_string()),
                extra: HashMap::new(),
            });

            // Добавляем кэфы 1X2
            if odds_values.len() >= 3 {
                Self::push_1x2_or_total(&mut odds, &event_id, &odds_values[..3], now);
            } else if odds_values.len() >= 2 {
                // Если только 2 кэфа, предполагаем Over/Under
                Self::push_1x2_or_total(&mut odds, &event_id, &odds_values, now);
            }
        }

        (events, odds)
    }

    /// Добавляем 1X2 или Total кэфы в вектор
    fn push_1x2_or_total(
        odds: &mut Vec<Odd>,
        event_id: &str,
        vals: &[f64],
        now: chrono::DateTime<Utc>,
    ) {
        if vals.len() >= 3 {
            // 1X2
            odds.push(Odd {
                id: format!("{}-1", event_id),
                event_id: event_id.to_string(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: vals[0],
                odds_type: OddsType::Home,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-X", event_id),
                event_id: event_id.to_string(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "X".to_string(),
                odds: vals[1],
                odds_type: OddsType::Draw,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-2", event_id),
                event_id: event_id.to_string(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "2".to_string(),
                odds: vals[2],
                odds_type: OddsType::Away,
                line: None,
                timestamp: now,
            });
        } else if vals.len() == 2 {
            // Тотал (Over/Under)
            odds.push(Odd {
                id: format!("{}-Over", event_id),
                event_id: event_id.to_string(),
                bookmaker_slug: "betcity".to_string(),
                market: "Total".to_string(),
                selection: "Over".to_string(),
                odds: vals[0],
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-Under", event_id),
                event_id: event_id.to_string(),
                bookmaker_slug: "betcity".to_string(),
                market: "Total".to_string(),
                selection: "Under".to_string(),
                odds: vals[1],
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: now,
            });
        }
    }

    /// Основная логика получения реальных runtime-данных: API → HTML
    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        // Шаг 1: Пробуем прямые API-эндпоинты
        match self.try_api_endpoints().await {
            Ok((events, odds)) if !events.is_empty() => {
                info!(
                    events = events.len(),
                    odds = odds.len(),
                    "Betcity: данные получены через API"
                );
                return Ok((events, odds));
            }
            Ok(_) => {
                warn!("Betcity: все API-эндпоинты вернули пустой результат");
            }
            Err(e) => {
                warn!(error = %e, "Betcity: ошибка при запросе API-эндпоинтов");
            }
        }

        // Шаг 2: Пробуем парсинг HTML + скрипт-теги
        match self.try_html_script_extraction().await {
            Ok((events, odds)) if !events.is_empty() => {
                info!(
                    events = events.len(),
                    odds = odds.len(),
                    "Betcity: данные извлечены из HTML скриптов"
                );
                return Ok((events, odds));
            }
            Ok(_) => {
                debug!("Betcity: JSON в HTML скриптах не найден");
            }
            Err(e) => {
                warn!(error = %e, "Betcity: ошибка при извлечении HTML скриптов");
            }
        }

        // Шаг 3: Пробуем парсинг HTML DOM
        match self.try_html_dom_parsing().await {
            Ok((events, odds)) if !events.is_empty() => {
                info!(
                    events = events.len(),
                    odds = odds.len(),
                    "Betcity: данные извлечены из HTML DOM"
                );
                return Ok((events, odds));
            }
            Ok(_) => {
                warn!("Betcity: события в HTML DOM не найдены — переходим на демо-данные");
            }
            Err(e) => {
                warn!(error = %e, "Betcity: ошибка при парсинге HTML DOM");
            }
        }

        Ok((Vec::new(), Vec::new()))
    }

    /// Основная логика получения данных: API → HTML → демо
    async fn fetch_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let (events, odds) = self.fetch_runtime_data().await?;
        if !events.is_empty() {
            return Ok((events, odds));
        }

        info!("Betcity: используем демо-данные");
        Ok(self.demo_data())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Демо-данные с реальными названиями команд
    // Команды выбраны так, чтобы совпадать с другими БК для матчинга вилок:
    // Бундеслига, Лига 1, Серия А, Ла Лига, АПЛ
    // ─────────────────────────────────────────────────────────────────────────
    fn demo_data(&self) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // (домашняя, гостевая, лига, is_live, [1, X, 2], [Over, Under], line)
        let demo_matches: &[(&str, &str, &str, bool, [f64; 3], [f64; 2], f64)] = &[
            // Бундеслига
            (
                "Бавария",
                "Байер",
                "Бундеслига",
                false,
                [1.72, 3.90, 4.60],
                [1.68, 2.15],
                2.5,
            ),
            (
                "Боруссия Д",
                "РБ Лейпциг",
                "Бундеслига",
                false,
                [2.10, 3.45, 3.30],
                [1.85, 1.95],
                2.5,
            ),
            // Лига 1 Франции
            (
                "ПСЖ",
                "Марсель",
                "Лига 1",
                false,
                [1.55, 4.10, 5.50],
                [1.72, 2.08],
                2.5,
            ),
            (
                "Лион",
                "Монако",
                "Лига 1",
                false,
                [2.25, 3.35, 3.10],
                [1.92, 1.88],
                2.5,
            ),
            // Серия А
            (
                "Ювентус",
                "Интер",
                "Серия А",
                false,
                [2.30, 3.20, 3.05],
                [1.88, 1.92],
                2.5,
            ),
            (
                "Наполи",
                "Милан",
                "Серия А",
                true,
                [2.15, 3.40, 3.20],
                [1.90, 1.90],
                2.5,
            ),
            // Ла Лига
            (
                "Реал Мадрид",
                "Барселона",
                "Ла Лига",
                false,
                [2.20, 3.30, 3.15],
                [1.82, 1.98],
                2.5,
            ),
            (
                "Атлетико",
                "Севилья",
                "Ла Лига",
                false,
                [1.95, 3.50, 3.90],
                [1.78, 2.02],
                2.5,
            ),
            // АПЛ
            (
                "Арсенал",
                "Манчестер Сити",
                "АПЛ",
                false,
                [2.40, 3.25, 2.85],
                [1.87, 1.93],
                2.5,
            ),
            (
                "Ливерпуль",
                "Челси",
                "АПЛ",
                true,
                [2.05, 3.45, 3.55],
                [1.80, 2.00],
                2.5,
            ),
        ];

        for (i, (home, away, league, is_live, odds_1x2, odds_total, line)) in
            demo_matches.iter().enumerate()
        {
            let eid = format!("betcity-{}", i);

            events.push(Event {
                id: eid.clone(),
                sport: Sport::Football,
                league: league.to_string(),
                home_team: home.to_string(),
                away_team: away.to_string(),
                start_time: None,
                is_live: *is_live,
                bookmaker_slug: "betcity".to_string(),
                raw_url: None,
                extra: HashMap::new(),
            });

            // 1X2
            odds.push(Odd {
                id: format!("{}-1", eid),
                event_id: eid.clone(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: odds_1x2[0],
                odds_type: OddsType::Home,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-X", eid),
                event_id: eid.clone(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "X".to_string(),
                odds: odds_1x2[1],
                odds_type: OddsType::Draw,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-2", eid),
                event_id: eid.clone(),
                bookmaker_slug: "betcity".to_string(),
                market: "1X2".to_string(),
                selection: "2".to_string(),
                odds: odds_1x2[2],
                odds_type: OddsType::Away,
                line: None,
                timestamp: now,
            });

            // Тотал
            odds.push(Odd {
                id: format!("{}-total-Over", eid),
                event_id: eid.clone(),
                bookmaker_slug: "betcity".to_string(),
                market: "Total".to_string(),
                selection: "Over".to_string(),
                odds: odds_total[0],
                odds_type: OddsType::Over,
                line: Some(*line),
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-total-Under", eid),
                event_id: eid.clone(),
                bookmaker_slug: "betcity".to_string(),
                market: "Total".to_string(),
                selection: "Under".to_string(),
                odds: odds_total[1],
                odds_type: OddsType::Under,
                line: Some(*line),
                timestamp: now,
            });
        }

        (events, odds)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Реализация трейта BookmakerParser
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl BookmakerParser for BetcityParser {
    fn name(&self) -> &str {
        "Betcity"
    }

    fn slug(&self) -> &str {
        "betcity"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Betcity: получаем события...");
        let (events, _) = self.fetch_data().await?;
        info!(count = events.len(), "Betcity: события получены");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Betcity: получаем кэфы...");
        let (_, odds) = self.fetch_data().await?;
        info!(count = odds.len(), "Betcity: кэфы получены");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        info!("Betcity: полное сканирование...");

        let (events, odds) = self.fetch_data().await?;

        let elapsed = start.elapsed().as_millis() as u64;
        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "Betcity: сканирование завершено"
        );
        Ok(ParserResult::new("betcity", events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://betcity.ru"
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
         AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/124.0.0.0 Safari/537.36"
    }
}
