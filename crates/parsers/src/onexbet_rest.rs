/// Улучшенный 1xBet/1xStavka REST API парсер
/// Работает с обходом защиты через несколько методов

use reqwest::Client;
use shared::{Event, Sport};
use std::collections::HashMap;
use std::sync::Arc;

pub struct OnexbetRestParser {
    client: Arc<Client>,
}

impl OnexbetRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Получить события 1xBet
    pub async fn fetch_events(&self) -> Result<Vec<Event>, String> {
        // 1xBet использует GraphQL API, но также доступны обычные endpoints
        
        if let Ok(events) = self.fetch_via_bff_api().await {
            return Ok(events);
        }
        
        if let Ok(events) = self.fetch_via_sports_api().await {
            return Ok(events);
        }
        
        if let Ok(events) = self.fetch_via_main_page().await {
            return Ok(events);
        }
        
        Err("No events found from any 1xBet endpoint".to_string())
    }

    /// Метод 1: BFF API (Backend For Frontend)
    async fn fetch_via_bff_api(&self) -> Result<Vec<Event>, String> {
        let endpoints = vec![
            ("https://1xstavka.ru/api/bff/v1/events/all", "ru"),
            ("https://1xbet.com/api/bff/v1/events/all", "en"),
            ("https://1xstavka.ru/api/sport/events/live", "ru"),
        ];
        
        let mut all_events = Vec::new();
        
        for (endpoint, _lang) in endpoints {
            match self.client
                .get(endpoint)
                .header("X-Requested-With", "XMLHttpRequest")
                .send()
                .await
            {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(body) = resp.text().await {
                        all_events.extend(self.parse_bff_response(&body));
                    }
                }
                _ => continue,
            }
        }
        
        if all_events.is_empty() {
            return Err("BFF API returned no events".to_string());
        }
        
        Ok(all_events)
    }

    /// Метод 2: Sports API
    async fn fetch_via_sports_api(&self) -> Result<Vec<Event>, String> {
        let sport_ids = vec![1, 2, 3, 4, 5]; // Football, Hockey, Basketball, Tennis, etc.
        let mut all_events = Vec::new();
        
        for sport_id in sport_ids {
            let url = format!("https://1xstavka.ru/api/sports/{}/list", sport_id);
            match self.client.get(&url).send().await {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(body) = resp.text().await {
                        all_events.extend(self.parse_sports_response(&body));
                    }
                }
                _ => continue,
            }
        }
        
        if all_events.is_empty() {
            return Err("Sports API returned no events".to_string());
        }
        
        Ok(all_events)
    }

    /// Метод 3: Главная страница (парсинг DOM)
    async fn fetch_via_main_page(&self) -> Result<Vec<Event>, String> {
        let resp = self.client
            .get("https://1xstavka.ru/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch: {}", e))?;
        
        let html = resp.text().await
            .map_err(|e| format!("Failed to read: {}", e))?;
        
        let events = self.extract_from_html(&html);
        
        if events.is_empty() {
            return Err("No events in main page".to_string());
        }
        
        Ok(events)
    }

    /// Парсит BFF API ответ
    fn parse_bff_response(&self, json_str: &str) -> Vec<Event> {
        let mut events = Vec::new();
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Структура может быть разной, пытаемся разные пути
            let objects = vec![
                json.get("events"),
                json.get("data").and_then(|d| d.get("events")),
                json.get("result"),
            ];
            
            for obj in objects {
                if let Some(arr) = obj.and_then(|o| o.as_array()) {
                    for item in arr {
                        if let Some(event) = self.build_event_from_api(item) {
                            events.push(event);
                        }
                    }
                }
            }
        }
        
        events
    }

    /// Парсит Sports API ответ
    fn parse_sports_response(&self, json_str: &str) -> Vec<Event> {
        let mut events = Vec::new();
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
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

    /// Извлекает события из HTML
    fn extract_from_html(&self, html: &str) -> Vec<Event> {
        let mut events = Vec::new();
        
        // Поиск data-event-id или event ID в разных форматах
        for line in html.lines() {
            if line.contains("data-event-id") || 
               line.contains("eventId") || 
               line.contains("event-") {
                if let Some(event) = self.parse_event_from_line(line) {
                    events.push(event);
                }
            }
        }
        
        events
    }

    /// Конструирует Event из API объекта
    fn build_event_from_api(&self, obj: &serde_json::Value) -> Option<Event> {
        let id = obj.get("id")
            .or(obj.get("eventId"))
            .or(obj.get("event_id"))
            .and_then(|v| {
                if let Some(n) = v.as_u64() {
                    Some(n.to_string())
                } else if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    None
                }
            })?;
        
        let home_team = obj.get("homeTeam")
            .or(obj.get("home"))
            .or(obj.get("team1"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;
        
        let away_team = obj.get("awayTeam")
            .or(obj.get("away"))
            .or(obj.get("team2"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;
        
        let league = obj.get("league")
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
            is_live: obj.get("is_live")
                .or(obj.get("live"))
                .or(obj.get("inLive"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "1xbet".to_string(),
            raw_url: Some(format!("https://1xstavka.ru/betting/{}", id)),
            extra: HashMap::new(),
        })
    }

    /// Простой парсер одной строки HTML
    fn parse_event_from_line(&self, line: &str) -> Option<Event> {
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
                    bookmaker_slug: "1xbet".to_string(),
                    raw_url: Some(format!("https://1xstavka.ru/betting/{}", id)),
                    extra: HashMap::new(),
                });
            }
        }
        
        None
    }
}
