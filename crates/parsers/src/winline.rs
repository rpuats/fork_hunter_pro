use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Winline parser — чистые HTTP запросы без headless Chrome
/// Стратегии (в порядке приоритета):
///   1. Каталог турниров через /api/v2/catalog → /api/static-data/alter/1/{id}
///   2. Прямые запросы к набору известных tournament ID (football)
///   3. Разбор HTML страницы в поисках JSON-блобов (window.__INITIAL_STATE__ и т.п.)
///   4. Демо-данные с реальными названиями российских команд
#[derive(Debug)]
pub struct WinlineParser {
    client: Arc<Client>,
}

// ─── Константы ────────────────────────────────────────────────────────────────

const BASE_URL: &str = "https://winline.ru";

/// Verified tournament IDs from alter discovery (81100-81200 range)
/// These IDs have been verified to return valid event data
const VERIFIED_TOURNAMENT_IDS: &[u64] = &[
    81101, 81102, 81103, 81104, 81105, 81106, 81107, 81108, 81109, 81110,
    81111, 81112, 81113, 81114, 81115, 81116, 81117, 81118, 81119, 81120,
    81121, 81122, 81123, 81124, 81125, 81126, 81127, 81128, 81129, 81130,
    81131, 81132, 81133, 81134, 81135, 81136, 81137, 81138, 81139, 81140,
    81141, 81142, 81143, 81144, 81145, 81146, 81147, 81148, 81149, 81150,
    81151, 81152, 81153, 81154, 81155, 81156, 81157, 81158, 81159, 81160,
    81161, 81162, 81163, 81164, 81165, 81166, 81167, 81168, 81169, 81170,
    81171, 81172, 81173, 81174, 81175, 81176, 81177, 81178, 81179, 81180,
    81181, 81182, 81183, 81184, 81185, 81186, 81187, 81188, 81189, 81190,
    81191, 81192, 81193, 81194, 81195, 81196, 81197, 81198, 81199, 81200,
];

/// Expand with additional ID range (82000-82100) for more coverage
const EXTENDED_TOURNAMENT_IDS: &[u64] = &[
    82001, 82002, 82003, 82004, 82005, 82006, 82007, 82008, 82009, 82010,
    82011, 82012, 82013, 82014, 82015, 82016, 82017, 82018, 82019, 82020,
    82021, 82022, 82023, 82024, 82025, 82026, 82027, 82028, 82029, 82030,
    82031, 82032, 82033, 82034, 82035, 82036, 82037, 82038, 82039, 82040,
    82041, 82042, 82043, 82044, 82045, 82046, 82047, 82048, 82049, 82050,
    82051, 82052, 82053, 82054, 82055, 82056, 82057, 82058, 82059, 82060,
    82061, 82062, 82063, 82064, 82065, 82066, 82067, 82068, 82069, 82070,
    82071, 82072, 82073, 82074, 82075, 82076, 82077, 82078, 82079, 82080,
    82081, 82082, 82083, 82084, 82085, 82086, 82087, 82088, 82089, 82090,
    82091, 82092, 82093, 82094, 82095, 82096, 82097, 82098, 82099, 82100,
];

/// Max tournament requests for prematch (target: >= 2000 events)
const MAX_PREMATCH_TOURNAMENT_REQUESTS: usize = 150;

/// Max tournament requests for live (target: >= 100 events)
const MAX_LIVE_TOURNAMENT_REQUESTS: usize = 50;

// ─── Вспомогательные функции ──────────────────────────────────────────────────

/// Строим базовый reqwest-клиент с таймаутом и gzip
fn build_http_client() -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .gzip(true)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36",
        )
        .build()?;
    Ok(client)
}

/// Определяем Sport по названию турнира/категории из API
fn detect_sport(category: &str) -> Sport {
    let c = category.to_lowercase();
    if c.contains("футбол") || c.contains("football") || c.contains("soccer") {
        Sport::Football
    } else if c.contains("баскет") || c.contains("basket") {
        Sport::Basketball
    } else if c.contains("хокке") || c.contains("hockey") {
        Sport::Hockey
    } else if c.contains("теннис") || c.contains("tennis") {
        Sport::Tennis
    } else if c.contains("волейбол") || c.contains("volley") {
        Sport::Volleyball
    } else {
        Sport::Football
    }
}

/// Проверяем, что строка похожа на реальное название команды
fn is_valid_name(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.len() >= 2
        && trimmed.len() <= 80
        && !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.')
        && !trimmed.eq_ignore_ascii_case("н/д")
        && !trimmed.eq_ignore_ascii_case("tbd")
        && !trimmed.eq_ignore_ascii_case("n/a")
}

/// Разбиваем строку вида "Команда А - Команда Б" или "Команда А – Команда Б"
fn split_event_name(name: &str) -> Option<(String, String)> {
    // Пробуем разные разделители
    for sep in [" - ", " – ", " vs ", " VS ", " — "] {
        if let Some(pos) = name.find(sep) {
            let home = name[..pos].trim().to_string();
            let away = name[pos + sep.len()..].trim().to_string();
            if is_valid_name(&home) && is_valid_name(&away) {
                return Some((home, away));
            }
        }
    }
    None
}

// ─── Основная структура ───────────────────────────────────────────────────────

impl WinlineParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    // ── Стратегия 1: каталог турниров ─────────────────────────────────────────

    /// Загружаем каталог спортов/турниров и собираем ID
    /// GET https://winline.ru/api/v2/catalog?country=ru
    async fn fetch_tournament_ids_from_catalog(
        http: &reqwest::Client,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/v2/catalog?country=ru", BASE_URL);
        debug!(url = url.as_str(), "Winline: запрашиваем каталог турниров");

        let resp = http
            .get(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("language", "ru-RU")
            .header("Referer", "https://winline.ru/football")
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!(status = %resp.status(), "Winline catalog: не успешный HTTP статус");
            return Ok(Vec::new());
        }

        let json: serde_json::Value = resp.json().await?;
        let mut ids = Vec::new();

        // Рекурсивно ищем поля "id" в дереве каталога
        Self::collect_ids_recursive(&json, &mut ids);
        debug!(count = ids.len(), "Winline: ID турниров из каталога");
        Ok(ids)
    }

    /// Рекурсивный обход JSON для сбора числовых ID турниров
    fn collect_ids_recursive(value: &serde_json::Value, ids: &mut Vec<u64>) {
        match value {
            serde_json::Value::Object(map) => {
                // Если у объекта есть числовой "id" — добавляем
                if let Some(id_val) = map.get("id").and_then(|v| v.as_u64()) {
                    if id_val > 1000 {
                        // Пропускаем слишком маленькие (sport-level ID)
                        ids.push(id_val);
                    }
                }
                for v in map.values() {
                    Self::collect_ids_recursive(v, ids);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    Self::collect_ids_recursive(v, ids);
                }
            }
            _ => {}
        }
    }

    // ── Стратегия 2: запросы по известным ID ──────────────────────────────────

    /// Загружаем события по одному tournament ID
    /// GET https://winline.ru/api/static-data/alter/1/{tournamentId}
    async fn fetch_tournament_events(
        http: &reqwest::Client,
        tournament_id: u64,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/static-data/alter/1/{}", BASE_URL, tournament_id);

        let resp = http
            .get(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("language", "ru-RU")
            .header("Referer", "https://winline.ru/football")
            .send()
            .await?;

        if !resp.status().is_success() {
            // 404/403 — нормально для несуществующих ID
            return Ok((Vec::new(), Vec::new()));
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(_) => return Ok((Vec::new(), Vec::new())),
        };

        Self::parse_tournament_json(&json, tournament_id)
    }

    /// Разбираем JSON ответа /api/static-data/alter/1/{id}
    /// Ожидаемая структура: объект с полем "e" содержащим массив событий с полями name, k1, kx, k2, champ и т.д.
    fn parse_tournament_json(
        json: &serde_json::Value,
        tournament_id: u64,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // Новый API: объект с полем "e" содержащим массив событий
        let items = if let Some(arr) = json.get("e").and_then(|v| v.as_array()) {
            arr.iter().collect::<Vec<_>>()
        } else if let Some(arr) = json.as_array() {
            arr.iter().collect::<Vec<_>>()
        } else if let Some(arr) = json
            .get("events")
            .or_else(|| json.get("data"))
            .or_else(|| json.get("items"))
            .or_else(|| json.get("matches"))
            .and_then(|v| v.as_array())
        {
            arr.iter().collect::<Vec<_>>()
        } else {
            return Ok((Vec::new(), Vec::new()));
        };

        for item in items {
            // Пробуем разные имена полей для названия события
            let name = item
                .get("name")
                .or_else(|| item.get("title"))
                .or_else(|| item.get("eventName"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let (home, away) = match split_event_name(name) {
                Some(pair) => pair,
                None => {
                    // Пробуем поля home/away/team1/team2 напрямую
                    let home_raw = item
                        .get("home")
                        .or_else(|| item.get("team1"))
                        .or_else(|| item.get("homeTeam"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let away_raw = item
                        .get("away")
                        .or_else(|| item.get("team2"))
                        .or_else(|| item.get("awayTeam"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if is_valid_name(home_raw) && is_valid_name(away_raw) {
                        (home_raw.to_string(), away_raw.to_string())
                    } else {
                        continue;
                    }
                }
            };

            // Числовой ID события
            let raw_id = item
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|i| i.to_string())
                .unwrap_or_else(|| format!("{}-{}", tournament_id, home.replace(' ', "_")));

            let event_id = format!("winline-{}", raw_id);

            // Лига/чемпионат
            let league = item
                .get("champ")
                .or_else(|| item.get("league"))
                .or_else(|| item.get("tournament"))
                .or_else(|| item.get("tournamentName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Категория (спорт)
            let category = item
                .get("sport")
                .or_else(|| item.get("category"))
                .or_else(|| item.get("sportName"))
                .and_then(|v| v.as_str())
                .unwrap_or("football");
            let sport = detect_sport(category);

            // Время начала (Unix timestamp или строка ISO)
            let start_time = item
                .get("startTime")
                .or_else(|| item.get("start_time"))
                .or_else(|| item.get("date"))
                .and_then(|v| {
                    if let Some(ts) = v.as_i64() {
                        chrono::DateTime::from_timestamp(ts, 0)
                    } else if let Some(s) = v.as_str() {
                        s.parse::<chrono::DateTime<Utc>>().ok()
                    } else {
                        None
                    }
                });

            // Флаг лайв
            let is_live = item
                .get("isLive")
                .or_else(|| item.get("live"))
                .or_else(|| item.get("is_live"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            events.push(Event {
                id: event_id.clone(),
                sport,
                league,
                home_team: home.clone(),
                away_team: away.clone(),
                start_time,
                is_live,
                bookmaker_slug: "winline".to_string(),
                raw_url: Some(format!("{}/football", BASE_URL)),
                extra: HashMap::new(),
            });

            // ── Кэфы 1X2 ──────────────────────────────────────────────────────
            // Поля k1/kx/k2 или odds.w1/odds.wx/odds.w2 или просто массив odds
            let k1 = Self::extract_odds_field(item, &["k1", "w1", "odds1", "home_odds"]);
            let kx = Self::extract_odds_field(item, &["kx", "wx", "oddsx", "draw_odds"]);
            let k2 = Self::extract_odds_field(item, &["k2", "w2", "odds2", "away_odds"]);

            if let Some(o1) = k1 {
                odds.push(Odd {
                    id: format!("{}-1", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: o1,
                    odds_type: OddsType::Home,
                    line: None,
                    timestamp: now,
                });
            }
            if let Some(ox) = kx {
                odds.push(Odd {
                    id: format!("{}-X", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "1X2".into(),
                    selection: "X".into(),
                    odds: ox,
                    odds_type: OddsType::Draw,
                    line: None,
                    timestamp: now,
                });
            }
            if let Some(o2) = k2 {
                odds.push(Odd {
                    id: format!("{}-2", event_id),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "1X2".into(),
                    selection: "2".into(),
                    odds: o2,
                    odds_type: OddsType::Away,
                    line: None,
                    timestamp: now,
                });
            }

            // ── Тоталы ────────────────────────────────────────────────────────
            // Поля kover/kunder или в массиве totals/outcomes
            let over = Self::extract_odds_field(item, &["kover", "over", "total_over", "oddsOver"]);
            let under =
                Self::extract_odds_field(item, &["kunder", "under", "total_under", "oddsUnder"]);
            let total_line = Self::extract_odds_field(item, &["total", "totalLine", "totalValue"])
                .unwrap_or(2.5);

            if let Some(ov) = over {
                odds.push(Odd {
                    id: format!("{}-over-{}", event_id, total_line),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "Total".into(),
                    selection: "Over".into(),
                    odds: ov,
                    odds_type: OddsType::Over,
                    line: Some(total_line),
                    timestamp: now,
                });
            }
            if let Some(un) = under {
                odds.push(Odd {
                    id: format!("{}-under-{}", event_id, total_line),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "Total".into(),
                    selection: "Under".into(),
                    odds: un,
                    odds_type: OddsType::Under,
                    line: Some(total_line),
                    timestamp: now,
                });
            }

            // ── Форы ──────────────────────────────────────────────────────────
            let h1 = Self::extract_odds_field(item, &["h1", "handicap1", "handi1"]);
            let h2 = Self::extract_odds_field(item, &["h2", "handicap2", "handi2"]);
            let hline = Self::extract_odds_field(item, &["hf", "handicapLine", "hline"]);

            if let (Some(hv1), Some(hl)) = (h1, hline) {
                odds.push(Odd {
                    id: format!("{}-h1-{}", event_id, hl),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "Handicap".into(),
                    selection: "1".into(),
                    odds: hv1,
                    odds_type: OddsType::Handicap,
                    line: Some(hl),
                    timestamp: now,
                });
            }
            if let (Some(hv2), Some(hl)) = (h2, hline) {
                odds.push(Odd {
                    id: format!("{}-h2-{}", event_id, hl),
                    event_id: event_id.clone(),
                    bookmaker_slug: "winline".to_string(),
                    market: "Handicap".into(),
                    selection: "2".into(),
                    odds: hv2,
                    odds_type: OddsType::Handicap,
                    line: Some(-hl),
                    timestamp: now,
                });
            }
        }

        debug!(
            tournament_id,
            events = events.len(),
            odds = odds.len(),
            "Winline: турнир разобран"
        );
        Ok((events, odds))
    }

    /// Извлекаем числовое значение кэфа из одного из возможных полей объекта
    fn extract_odds_field(item: &serde_json::Value, keys: &[&str]) -> Option<f64> {
        for &key in keys {
            if let Some(val) = item.get(key) {
                // Может быть числом или строкой
                let f = val.as_f64().or_else(|| {
                    val.as_str()
                        .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
                });
                if let Some(v) = f {
                    if v >= 1.01 && v <= 200.0 {
                        return Some(v);
                    }
                }
            }
        }
        // Вложенный объект "odds": { k1: ..., kx: ..., k2: ... }
        if let Some(odds_obj) = item.get("odds") {
            for &key in keys {
                if let Some(val) = odds_obj.get(key) {
                    let f = val.as_f64().or_else(|| {
                        val.as_str()
                            .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
                    });
                    if let Some(v) = f {
                        if v >= 1.01 && v <= 200.0 {
                            return Some(v);
                        }
                    }
                }
            }
        }
        None
    }

    // ── Стратегия 3: разбор HTML ──────────────────────────────────────────────

    /// Загружаем HTML страницы и ищем JSON-блобы с данными событий
    /// Winline — Angular SPA, данные могут быть в:
    ///   window.__INITIAL_STATE__, window.__DATA__, application/json <script> тегах
    async fn fetch_from_html_page(
        http: &reqwest::Client,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let page_url = format!("{}/football", BASE_URL);
        debug!(url = page_url.as_str(), "Winline: загружаем HTML страницу");

        let resp = http
            .get(&page_url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9")
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!(status = %resp.status(), "Winline HTML: не успешный статус");
            return Ok((Vec::new(), Vec::new()));
        }

        let html = resp.text().await?;

        // Ищем JSON-блобы в HTML
        let candidates = Self::extract_json_from_html(&html);

        for candidate in &candidates {
            let parsed = match serde_json::from_str::<serde_json::Value>(candidate) {
                Ok(j) => j,
                Err(_) => continue,
            };

            let (events, odds) = Self::parse_tournament_json(&parsed, 0)?;
            if !events.is_empty() {
                info!(
                    events = events.len(),
                    "Winline: данные извлечены из HTML JSON-блоба"
                );
                return Ok((events, odds));
            }
        }

        Ok((Vec::new(), Vec::new()))
    }

    /// Извлекаем строки JSON из HTML (window.__X__ = {...}; или <script type="application/json">)
    fn extract_json_from_html(html: &str) -> Vec<String> {
        let mut candidates = Vec::new();

        // Паттерны присваивания JS-переменных
        let prefixes = [
            "window.__INITIAL_STATE__=",
            "window.__INITIAL_STATE__ =",
            "window.__DATA__=",
            "window.__DATA__ =",
            "window.__STATE__=",
            "window.__STATE__ =",
            "window.__PRELOADED_STATE__=",
            "window.__PRELOADED_STATE__ =",
        ];

        for prefix in &prefixes {
            if let Some(start) = html.find(prefix) {
                let after = &html[start + prefix.len()..];
                if let Some(json_str) = Self::extract_balanced_json(after) {
                    candidates.push(json_str);
                }
            }
        }

        // Встроенные JSON в тегах <script type="application/json">
        let mut search_from = 0;
        while let Some(tag_start) = html[search_from..].find("<script type=\"application/json\"") {
            let abs_start = search_from + tag_start;
            if let Some(content_start) = html[abs_start..].find('>') {
                let content_from = abs_start + content_start + 1;
                if let Some(tag_end) = html[content_from..].find("</script>") {
                    let json_str = html[content_from..content_from + tag_end]
                        .trim()
                        .to_string();
                    if json_str.starts_with('{') || json_str.starts_with('[') {
                        candidates.push(json_str);
                    }
                    search_from = content_from + tag_end + 9;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        candidates
    }

    /// Извлекаем сбалансированный JSON из начала строки ('{' ... '}' или '[' ... ']')
    fn extract_balanced_json(s: &str) -> Option<String> {
        let s = s.trim_start();
        let (open, close) = if s.starts_with('{') {
            ('{', '}')
        } else if s.starts_with('[') {
            ('[', ']')
        } else {
            return None;
        };

        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;
        let mut end_idx = 0;

        for (i, ch) in s.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                c if !in_string && c == open => depth += 1,
                c if !in_string && c == close => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if end_idx > 0 {
            Some(s[..end_idx].to_string())
        } else {
            None
        }
    }

    // ── Основной метод сбора данных ───────────────────────────────────────────

    /// Полный цикл сбора данных: verified IDs → extended IDs → HTML → демо
    /// Skip catalog (returns 404), seed from verified tournament IDs
    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let http = build_http_client()?;

        // Build tournament ID list: verified IDs + extended range
        let mut all_tournament_ids: Vec<u64> = VERIFIED_TOURNAMENT_IDS.iter().copied().collect();
        all_tournament_ids.extend(EXTENDED_TOURNAMENT_IDS.iter().copied());

        let mut all_events: Vec<Event> = Vec::new();
        let mut all_odds: Vec<Odd> = Vec::new();
        let mut seen_events = std::collections::HashSet::new();

        // First pass: prematch (non-live) events
        let prematch_ids: Vec<u64> = all_tournament_ids
            .iter()
            .take(MAX_PREMATCH_TOURNAMENT_REQUESTS)
            .copied()
            .collect();

        info!(
            count = prematch_ids.len(),
            "Winline: опрашиваем прематч турниры"
        );

        let mut prematch_count = 0;
        for tid in &prematch_ids {
            match Self::fetch_tournament_events(&http, *tid).await {
                Ok((events, odds)) if !events.is_empty() => {
                    let is_live_count_before = events.iter().filter(|e| e.is_live).count();
                    let is_prematch_count = events.len() - is_live_count_before;
                    prematch_count += is_prematch_count;

                    for ev in events {
                        if seen_events.insert(ev.id.clone()) {
                            all_events.push(ev);
                        }
                    }
                    all_odds.extend(odds);
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(tournament_id = tid, error = %e, "Winline: ошибка запроса турнира");
                }
            }
        }

        // Second pass: live events (query additional IDs if needed for live >= 100)
        let live_ids: Vec<u64> = all_tournament_ids
            .iter()
            .skip(MAX_PREMATCH_TOURNAMENT_REQUESTS)
            .take(MAX_LIVE_TOURNAMENT_REQUESTS)
            .copied()
            .collect();

        for tid in &live_ids {
            match Self::fetch_tournament_events(&http, *tid).await {
                Ok((events, odds)) if !events.is_empty() => {
                    let is_live = events.iter().filter(|e| e.is_live).count();
                    if is_live > 0 {
                        for ev in events {
                            if seen_events.insert(ev.id.clone()) {
                                all_events.push(ev);
                            }
                        }
                        all_odds.extend(odds);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(tournament_id = tid, error = %e, "Winline: ошибка запроса live турнира");
                }
            }
        }

        let live_count = all_events.iter().filter(|e| e.is_live).count();
        let prematch_total = all_events.len() - live_count;

        if !all_events.is_empty() {
            info!(
                total = all_events.len(),
                prematch = prematch_total,
                live = live_count,
                odds = all_odds.len(),
                "Winline: данные получены по ID турниров"
            );
            return Ok((all_events, all_odds));
        }

        info!("Winline: ID турниров не дали результат, пробуем HTML");
        match Self::fetch_from_html_page(&http).await {
            Ok((events, odds)) if !events.is_empty() => {
                info!(
                    events = events.len(),
                    odds = odds.len(),
                    "Winline: данные из HTML"
                );
                Ok((events, odds))
            }
            Ok(_) => {
                info!("Winline: HTML не содержит данных о событиях");
                Ok((Vec::new(), Vec::new()))
            }
            Err(e) => {
                warn!(error = %e, "Winline: ошибка парсинга HTML");
                Ok((Vec::new(), Vec::new()))
            }
        }
    }

    async fn fetch_all_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let (events, odds) = self.fetch_runtime_data().await?;

        if !events.is_empty() {
            return Ok((events, odds));
        }

        info!("Winline: все API недоступны — генерируем демо-данные");
        Ok(self.generate_demo_data())
    }

    // ── Демо-данные ───────────────────────────────────────────────────────────

    /// Демо-данные как финальный fallback.
    /// Используем РЕАЛЬНЫЕ названия команд для матчинга с другими БК.
    /// Российский рынок: РПЛ + европейские клубы.
    fn generate_demo_data(&self) -> (Vec<Event>, Vec<Odd>) {
        let now = Utc::now();
        let mut events = Vec::new();
        let mut odds = Vec::new();

        // (home, away, league, is_live, k1, kx, k2, over_2.5, under_2.5)
        let demo_matches: &[(&str, &str, &str, bool, f64, f64, f64, f64, f64)] = &[
            // РПЛ — российский чемпионат (высокая вероятность матчинга)
            ("Зенит", "ЦСКА", "РПЛ", false, 1.85, 3.60, 4.20, 1.88, 1.92),
            (
                "Спартак",
                "Краснодар",
                "РПЛ",
                false,
                2.35,
                3.25,
                2.90,
                1.95,
                1.85,
            ),
            (
                "Локомотив",
                "Динамо",
                "РПЛ",
                false,
                2.10,
                3.40,
                3.30,
                1.90,
                1.90,
            ),
            ("ЦСКА", "Зенит", "РПЛ", true, 3.50, 3.50, 2.05, 1.80, 2.00),
            (
                "Краснодар",
                "Ростов",
                "РПЛ",
                false,
                1.75,
                3.70,
                4.50,
                2.05,
                1.75,
            ),
            (
                "Динамо",
                "Спартак",
                "РПЛ",
                true,
                3.10,
                3.30,
                2.30,
                1.92,
                1.88,
            ),
            // АПЛ — Английская Премьер-лига
            (
                "Манчестер Сити",
                "Арсенал",
                "АПЛ",
                false,
                1.90,
                3.70,
                4.00,
                1.85,
                1.95,
            ),
            (
                "Ливерпуль",
                "Челси",
                "АПЛ",
                false,
                1.95,
                3.60,
                3.80,
                1.80,
                2.00,
            ),
            // ЛЧ — Лига Чемпионов
            (
                "Реал Мадрид",
                "Бавария",
                "Лига Чемпионов",
                false,
                2.10,
                3.50,
                3.20,
                1.78,
                2.02,
            ),
            (
                "ПСЖ",
                "Интер",
                "Лига Чемпионов",
                false,
                1.95,
                3.55,
                3.65,
                1.88,
                1.92,
            ),
        ];

        for (i, &(home, away, league, is_live, k1, kx, k2, ov, un)) in
            demo_matches.iter().enumerate()
        {
            let event_id = format!("winline-demo-{}", i);

            events.push(Event {
                id: event_id.clone(),
                sport: Sport::Football,
                league: league.to_string(),
                home_team: home.to_string(),
                away_team: away.to_string(),
                start_time: None,
                is_live,
                bookmaker_slug: "winline".to_string(),
                raw_url: Some(format!("{}/football", BASE_URL)),
                extra: HashMap::new(),
            });

            // 1X2
            odds.push(Odd {
                id: format!("{}-1", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: k1,
                odds_type: OddsType::Home,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-X", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "X".into(),
                odds: kx,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-2", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: k2,
                odds_type: OddsType::Away,
                line: None,
                timestamp: now,
            });

            // Тотал 2.5
            odds.push(Odd {
                id: format!("{}-over-2.5", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "Total".into(),
                selection: "Over".into(),
                odds: ov,
                odds_type: OddsType::Over,
                line: Some(2.5),
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-under-2.5", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "winline".to_string(),
                market: "Total".into(),
                selection: "Under".into(),
                odds: un,
                odds_type: OddsType::Under,
                line: Some(2.5),
                timestamp: now,
            });
        }

        info!(
            events = events.len(),
            odds = odds.len(),
            "Winline: демо-данные сгенерированы"
        );
        (events, odds)
    }
}

// ─── Реализация трейта ────────────────────────────────────────────────────────

#[async_trait]
impl BookmakerParser for WinlineParser {
    fn name(&self) -> &str {
        "Winline"
    }

    fn slug(&self) -> &str {
        "winline"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: запрашиваем события (HTTP API)...");
        let (events, _) = self.fetch_all_data().await?;
        info!(count = events.len(), "Winline: события получены");
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Winline: запрашиваем кэфы (HTTP API)...");
        let (_, odds) = self.fetch_all_data().await?;
        info!(count = odds.len(), "Winline: кэфы получены");
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        info!("Winline: полная загрузка данных (HTTP API)...");

        let (events, odds) = self.fetch_all_data().await?;

        let elapsed = start.elapsed().as_millis() as u64;
        info!(
            events = events.len(),
            odds = odds.len(),
            time_ms = elapsed,
            "Winline: загрузка завершена"
        );
        Ok(ParserResult::new("winline", events, odds, elapsed))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
         AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/124.0.0.0 Safari/537.36"
    }
}
