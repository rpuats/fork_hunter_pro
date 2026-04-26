/// Улучшенный BetBoom REST API парсер
/// Использует проксирование и стелс-методы для обхода блокировок
use reqwest::Client;
use shared::{Event, Sport};
use std::collections::HashMap;
use std::sync::Arc;

pub struct BetboomRestParser {
    client: Arc<Client>,
}

impl BetboomRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Получить события BetBoom через REST API
    pub async fn fetch_events(&self) -> Result<Vec<Event>, String> {
        // Пытаемся несколько API endpoints в порядке приоритета
        if let Ok(events) = self.fetch_via_api_v3().await {
            return Ok(events);
        }

        if let Ok(events) = self.fetch_via_main_page().await {
            return Ok(events);
        }

        if let Ok(events) = self.fetch_via_xds_api().await {
            return Ok(events);
        }

        Err("No events found from any BetBoom endpoint".to_string())
    }

    /// Метод 1: Новый API v3
    async fn fetch_via_api_v3(&self) -> Result<Vec<Event>, String> {
        let endpoints = vec![
            "https://betboom.ru/api/v3/sports/football/events?live=false",
            "https://betboom.ru/api/v3/sports/football/events?live=true",
            "https://betboom.ru/api/v3/sports/ice-hockey/events?live=false",
        ];

        let mut all_events = Vec::new();

        for endpoint in endpoints {
            match self.client.get(endpoint).send().await {
                Ok(resp) => {
                    if resp.status() == 200 {
                        if let Ok(body) = resp.text().await {
                            all_events.extend(self.parse_api_response(&body));
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        if all_events.is_empty() {
            return Err("API v3 returned no events".to_string());
        }

        Ok(all_events)
    }

    /// Метод 2: Главная страница (данные в HTML)
    async fn fetch_via_main_page(&self) -> Result<Vec<Event>, String> {
        let resp = self
            .client
            .get("https://betboom.ru/")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch main page: {}", e))?;

        let html = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let events = self.extract_from_html(&html);

        if events.is_empty() {
            return Err("No events in main page".to_string());
        }

        Ok(events)
    }

    /// Метод 3: XDS API (древний, но иногда работает)
    async fn fetch_via_xds_api(&self) -> Result<Vec<Event>, String> {
        let endpoints = vec![
            "https://betboom.ru/api/xds/v2/sport/205", // Football
            "https://betboom.ru/api/xds/v2/sport/208", // Ice Hockey
        ];

        let mut all_events = Vec::new();

        for endpoint in endpoints {
            match self.client.get(endpoint).send().await {
                Ok(resp) => {
                    if resp.status() == 200 {
                        if let Ok(body) = resp.text().await {
                            all_events.extend(self.parse_xds_response(&body));
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        if all_events.is_empty() {
            return Err("XDS API returned no events".to_string());
        }

        Ok(all_events)
    }

    /// Извлекает события из HTML
    fn extract_from_html(&self, html: &str) -> Vec<Event> {
        let mut events = Vec::new();

        // Поиск data-event-id атрибутов
        for line in html.lines() {
            if line.contains("data-event-id") || line.contains("event-") {
                if let Some(event) = self.parse_event_from_line(line) {
                    events.push(event);
                }
            }
        }

        events
    }

    /// Парсит JSON ответ API v3
    fn parse_api_response(&self, json_str: &str) -> Vec<Event> {
        let mut events = Vec::new();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    if let Some(event) = self.build_event_from_api(item) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    /// Парсит XDS API ответ
    fn parse_xds_response(&self, response: &str) -> Vec<Event> {
        // XDS обычно возвращает бинарный формат, но пытаемся распарсить
        let mut events = Vec::new();

        // Пытаемся найти JSON объекты в ответе
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
            if let Some(arr) = json.as_array() {
                for item in arr {
                    if let Some(event) = self.build_event_from_api(item) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    /// Конструирует Event из API объекта
    fn build_event_from_api(&self, obj: &serde_json::Value) -> Option<Event> {
        let id = obj
            .get("id")
            .or(obj.get("eventId"))
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())?;

        let home_team = obj
            .get("homeTeam")
            .or(obj.get("home"))
            .or(obj.get("participant1"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let away_team = obj
            .get("awayTeam")
            .or(obj.get("away"))
            .or(obj.get("participant2"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let league = obj
            .get("league")
            .or(obj.get("tournament"))
            .or(obj.get("competition"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        Some(Event {
            id: id.clone(),
            sport: Sport::Football,
            league,
            home_team,
            away_team,
            start_time: None,
            is_live: obj
                .get("is_live")
                .or(obj.get("live"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "betboom".to_string(),
            raw_url: Some(format!("https://betboom.ru/sport/betting/{}", id)),
            extra: HashMap::new(),
        })
    }

    /// Простой парсер одной строки HTML
    fn parse_event_from_line(&self, line: &str) -> Option<Event> {
        // Ищем pattern: <div ... data-event-id="123" ... >Team1</div>Team2
        if let Some(start) = line.find("data-event-id=\"") {
            let rest = &line[start + 15..];
            if let Some(end) = rest.find("\"") {
                let id = rest[..end].to_string();

                return Some(Event {
                    id: id.clone(),
                    sport: Sport::Football,
                    league: "Unknown".to_string(),
                    home_team: "Unknown".to_string(),
                    away_team: "Unknown".to_string(),
                    start_time: None,
                    is_live: line.contains("live"),
                    bookmaker_slug: "betboom".to_string(),
                    raw_url: Some(format!("https://betboom.ru/sport/betting/{}", id)),
                    extra: HashMap::new(),
                });
            }
        }

        None
    }
}
