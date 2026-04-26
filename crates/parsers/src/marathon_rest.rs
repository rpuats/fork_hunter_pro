/// Marathon REST API парсер
/// Работает с несколькими методами доступа к событиям
use reqwest::Client;
use shared::{Event, Sport};
use std::collections::HashMap;
use std::sync::Arc;

pub struct MarathonRestParser {
    client: Arc<Client>,
}

impl MarathonRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Получить события Marathon
    pub async fn fetch_events(&self) -> Result<Vec<Event>, String> {
        if let Ok(events) = self.fetch_via_api().await {
            return Ok(events);
        }

        if let Ok(events) = self.fetch_via_main_page().await {
            return Ok(events);
        }

        Err("No events from Marathon".to_string())
    }

    /// API метод
    async fn fetch_via_api(&self) -> Result<Vec<Event>, String> {
        let endpoints = vec![
            "https://marathonbet.com/api/v2/events",
            "https://marathonbet.com/api/v2/sports/1/events", // Football
            "https://marathonbet.com/api/v2/sports/2/events", // Hockey
        ];

        let mut all_events = Vec::new();

        for endpoint in endpoints {
            match self
                .client
                .get(endpoint)
                .header("X-Requested-With", "XMLHttpRequest")
                .send()
                .await
            {
                Ok(resp) if resp.status() == 200 => {
                    if let Ok(body) = resp.text().await {
                        all_events.extend(self.parse_api(&body));
                    }
                }
                _ => continue,
            }
        }

        if all_events.is_empty() {
            return Err("No events from API".to_string());
        }

        Ok(all_events)
    }

    /// HTML парсинг
    async fn fetch_via_main_page(&self) -> Result<Vec<Event>, String> {
        let resp = self
            .client
            .get("https://marathonbet.com/")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch: {}", e))?;

        let html = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read: {}", e))?;

        let events = self.extract_from_html(&html);

        if events.is_empty() {
            return Err("No events in HTML".to_string());
        }

        Ok(events)
    }

    /// Парсит JSON ответ API
    fn parse_api(&self, json_str: &str) -> Vec<Event> {
        let mut events = Vec::new();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Пытаемся разные пути в JSON
            let mut arr: Option<&Vec<serde_json::Value>> = None;

            if let Some(events_field) = json.get("events") {
                arr = events_field.as_array();
            } else if let Some(data) = json.get("data") {
                if let Some(events_field) = data.get("events") {
                    arr = events_field.as_array();
                }
            } else if let Some(a) = json.as_array() {
                arr = Some(a);
            }

            if let Some(items) = arr {
                for item in items {
                    if let Some(event) = self.build_event(item) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    /// Извлекает из HTML
    fn extract_from_html(&self, html: &str) -> Vec<Event> {
        let mut events = Vec::new();

        for line in html.lines() {
            if line.contains("event") && line.contains("data-") {
                if let Some(event) = self.parse_html_line(line) {
                    events.push(event);
                }
            }
        }

        events
    }

    /// Конструирует Event из JSON
    fn build_event(&self, obj: &serde_json::Value) -> Option<Event> {
        let id = obj
            .get("id")
            .or(obj.get("eventId"))
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())?;

        let home_team = obj
            .get("homeTeam")
            .or(obj.get("home"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let away_team = obj
            .get("awayTeam")
            .or(obj.get("away"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        Some(Event {
            id: id.clone(),
            sport: Sport::Football,
            league: obj
                .get("league")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            home_team,
            away_team,
            start_time: None,
            is_live: obj
                .get("is_live")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "marathon".to_string(),
            raw_url: Some(format!("https://marathonbet.com/event/{}", id)),
            extra: HashMap::new(),
        })
    }

    /// Парсит одну линию HTML
    fn parse_html_line(&self, line: &str) -> Option<Event> {
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
                    is_live: false,
                    bookmaker_slug: "marathon".to_string(),
                    raw_url: Some(format!("https://marathonbet.com/event/{}", id)),
                    extra: HashMap::new(),
                });
            }
        }

        None
    }
}
