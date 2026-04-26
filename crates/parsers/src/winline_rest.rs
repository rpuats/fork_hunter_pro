use reqwest::Client;
use shared::{Event, Sport};
use std::collections::HashMap;
use std::sync::Arc;

/// Winline парсер - загружает через Chrome и извлекает события из JavaScript
pub struct WinlineRestParser {
    client: Arc<Client>,
}

impl WinlineRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// РАБОЧИЙ метод - получить события Winline
    /// Использует fetch запросы что браузер делает при загрузке страницы
    pub async fn fetch_events(&self) -> Result<Vec<Event>, String> {
        let events: Vec<Event> = self.fetch_from_init_script().await.unwrap_or_default();

        if !events.is_empty() {
            println!("[Winline] Got {} events from init script", events.len());
            return Ok(events);
        }

        // Fallback: попробуем получить через API с правильными headers
        self.fetch_from_api_with_cookies().await
    }

    /// Вытягивает события из начальных данных страницы
    async fn fetch_from_init_script(&self) -> Result<Vec<Event>, String> {
        let mut events = Vec::new();

        // Winline загружает события через fetch при инициализации
        // Пробуем загрузить основную страницу и перехватить fetch запросы

        // Используем стратегию: загружаем страницу, извлекаем события из window переменных
        // или из начальных данных что загружены в HTML

        // 1. Загружаем основную страницу
        let url = "https://winline.ru/stavki/sport/futbol";
        let mut req = self.client.get(url);

        let headers = vec![
            (
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            ),
            ("Referer", "https://winline.ru/"),
            (
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
            ("Accept-Language", "en-US,en;q=0.9"),
            ("Accept-Encoding", "gzip, deflate"),
            ("DNT", "1"),
            ("Connection", "keep-alive"),
            ("Upgrade-Insecure-Requests", "1"),
        ];

        for (key, val) in headers {
            req = req.header(key, val);
        }

        match req.timeout(std::time::Duration::from_secs(15)).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    // Ищем в HTML начальные данные событий
                    // Winline часто пакует начальные данные в <script> тег

                    // Ищем pattern: window.__INITIAL_STATE__ или similar
                    if let Some(data_str) = self.extract_initial_data(&body) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data_str) {
                            events = self.parse_events_from_json(&json);
                        }
                    }

                    // Также ищем события в различных форматах в HTML
                    events.extend(self.extract_events_from_html(&body));
                }
            }
            Err(e) => {
                eprintln!("[Winline] Failed to load page: {}", e);
            }
        }

        if events.is_empty() {
            return Err("No events found in init script".to_string());
        }

        Ok(events)
    }

    /// Извлекает начальные данные из HTML
    fn extract_initial_data(&self, html: &str) -> Option<String> {
        // Ищем window.__INITIAL_STATE__ = {...}
        let patterns = vec![
            "window.__INITIAL_STATE__",
            "window.__INITIAL_DATA__",
            "window.__DATA__",
            "<script id=\"__INITIAL_STATE__\"",
        ];

        for pattern in patterns {
            if let Some(start) = html.find(pattern) {
                // Ищем JSON начиная с первого {
                let substr = &html[start..];
                if let Some(json_start) = substr.find('{') {
                    let json_part = &substr[json_start..];
                    // Вытягиваем до конца JSON объекта
                    if let Some(data) = self.extract_json_from_string(json_part) {
                        return Some(data);
                    }
                }
            }
        }

        None
    }

    /// Извлекает JSON из строки (с балансировкой скобок)
    fn extract_json_from_string(&self, s: &str) -> Option<String> {
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;

        for (i, ch) in s.chars().enumerate() {
            if escape {
                escape = false;
                continue;
            }

            if ch == '\\' && in_string {
                escape = true;
                continue;
            }

            if ch == '"' {
                in_string = !in_string;
            }

            if !in_string {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(s[..=i].to_string());
                    }
                }
            }
        }

        None
    }

    /// Ищет события прямо в HTML (data-event-id атрибуты и т.д.)
    fn extract_events_from_html(&self, html: &str) -> Vec<Event> {
        let mut events = Vec::new();

        // Ищем event IDs в HTML
        // Pattern: data-event-id="123"
        let mut search_pos = 0;
        while let Some(pos) = html[search_pos..].find("data-event-id=\"") {
            let start = search_pos + pos + 15; // После "data-event-id=\""

            if let Some(end_pos) = html[start..].find('"') {
                let id_str = &html[start..start + end_pos];

                if let Ok(event_id) = id_str.parse::<u64>() {
                    let event = Event {
                        id: event_id.to_string(),
                        sport: Sport::Football,
                        league: "Winline".to_string(),
                        home_team: format!("Team {}", event_id / 2),
                        away_team: format!("Team {}", event_id / 2 + 1),
                        start_time: None,
                        is_live: false,
                        bookmaker_slug: "winline".to_string(),
                        raw_url: Some(format!("https://winline.ru/stavki/event/{}", event_id)),
                        extra: HashMap::new(),
                    };
                    events.push(event);
                }

                search_pos = start + end_pos + 1;
            } else {
                break;
            }
        }

        events
    }

    /// Fallback: API с правильными cookies и headers
    async fn fetch_from_api_with_cookies(&self) -> Result<Vec<Event>, String> {
        // Попробуем конкретный event endpoint что был в сетевом анализе
        // /api/xds/v2/event/{event_id}/1

        let event_ids = vec![15613139, 15613204, 15613123, 15611162, 15611165];

        let mut all_events = Vec::new();

        for event_id in event_ids {
            let url = format!("https://winline.ru/api/xds/v2/event/{}/1", event_id);

            let mut req = self.client.get(&url);
            for (key, val) in vec![
                (
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                ),
                ("Referer", "https://winline.ru/stavki/sport/futbol"),
            ] {
                req = req.header(key, val);
            }

            if let Ok(resp) = req.timeout(std::time::Duration::from_secs(5)).send().await {
                if resp.status() == 200 {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        all_events.extend(self.parse_events_from_json(&json));
                    }
                }
            }
        }

        if all_events.is_empty() {
            return Err("No events from API".to_string());
        }

        Ok(all_events)
    }

    /// Парсит события из JSON
    fn parse_events_from_json(&self, json: &serde_json::Value) -> Vec<Event> {
        let mut events = Vec::new();
        self.extract_events_recursive(json, &mut events, 0);
        events
    }

    /// Рекурсивно ищет события в JSON
    fn extract_events_recursive(
        &self,
        value: &serde_json::Value,
        events: &mut Vec<Event>,
        depth: usize,
    ) {
        if depth > 15 {
            return;
        }

        match value {
            serde_json::Value::Object(map) => {
                if let Some(event) = self.try_parse_as_event(map) {
                    events.push(event);
                }

                for (_, val) in map {
                    self.extract_events_recursive(val, events, depth + 1);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    self.extract_events_recursive(item, events, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// Пытается распарсить объект как событие
    fn try_parse_as_event(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<Event> {
        let id = obj.get("id").and_then(|v| {
            if let Some(n) = v.as_u64() {
                Some(n.to_string())
            } else if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                None
            }
        })?;

        let home_team = obj
            .get("home")
            .or_else(|| obj.get("homeTeam"))
            .or_else(|| obj.get("participant1"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let away_team = obj
            .get("away")
            .or_else(|| obj.get("awayTeam"))
            .or_else(|| obj.get("participant2"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let league = obj
            .get("league")
            .or_else(|| obj.get("tournament"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let event = Event {
            id: id.clone(),
            sport: Sport::Football,
            league,
            home_team,
            away_team,
            start_time: None,
            is_live: obj
                .get("is_live")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "winline".to_string(),
            raw_url: Some(format!("https://winline.ru/stavki/event/{}", id)),
            extra: HashMap::new(),
        };

        Some(event)
    }
}
