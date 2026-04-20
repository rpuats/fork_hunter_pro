/// Melbet REST API парсер
/// Поддерживает основные методы доступа

use reqwest::Client;
use shared::{Event, Sport};
use std::collections::HashMap;
use std::sync::Arc;

pub struct MelbetRestParser {
    client: Arc<Client>,
}

impl MelbetRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Получить события Melbet
    pub async fn fetch_events(&self) -> Result<Vec<Event>, String> {
        if let Ok(events) = self.fetch_via_api().await {
            return Ok(events);
        }
        
        if let Ok(events) = self.fetch_via_graphql().await {
            return Ok(events);
        }
        
        if let Ok(events) = self.fetch_via_main_page().await {
            return Ok(events);
        }
        
        Err("No events from Melbet".to_string())
    }

    /// API метод
    async fn fetch_via_api(&self) -> Result<Vec<Event>, String> {
        let endpoints = vec![
            "https://melbet.com/api/v1/events",
            "https://melbet.com/api/v1/sports/football",
            "https://melbet.com/api/betting/events",
        ];
        
        let mut all_events = Vec::new();
        
        for endpoint in endpoints {
            match self.client.get(endpoint).send().await {
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

    /// GraphQL метод
    async fn fetch_via_graphql(&self) -> Result<Vec<Event>, String> {
        let query = r#"
        {
            events(sport: "football", limit: 100) {
                id
                homeTeam
                awayTeam
                league
                startTime
                isLive
            }
        }
        "#;
        
        match self.client
            .post("https://melbet.com/api/graphql")
            .body(query)
            .send()
            .await
        {
            Ok(resp) if resp.status() == 200 => {
                if let Ok(body) = resp.text().await {
                    let events = self.parse_api(&body);
                    if !events.is_empty() {
                        return Ok(events);
                    }
                }
            }
            _ => {}
        }
        
        Err("GraphQL failed".to_string())
    }

    /// HTML парсинг
    async fn fetch_via_main_page(&self) -> Result<Vec<Event>, String> {
        let resp = self.client
            .get("https://melbet.com/")
            .send()
            .await
            .map_err(|e| format!("Failed: {}", e))?;
        
        let html = resp.text().await
            .map_err(|e| format!("Failed: {}", e))?;
        
        let events = self.extract_from_html(&html);
        
        if events.is_empty() {
            return Err("No events".to_string());
        }
        
        Ok(events)
    }

    /// Парсит JSON
    fn parse_api(&self, json_str: &str) -> Vec<Event> {
        let mut events = Vec::new();
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Пытаемся разные структуры JSON
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
            if let Some(event) = self.parse_html_line(line) {
                events.push(event);
            }
        }
        
        events
    }

    /// Конструирует Event
    fn build_event(&self, obj: &serde_json::Value) -> Option<Event> {
        let id = obj.get("id")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())?;
        
        let home_team = obj.get("homeTeam")
            .or(obj.get("home"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;
        
        let away_team = obj.get("awayTeam")
            .or(obj.get("away"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;
        
        Some(Event {
            id: id.clone(),
            sport: Sport::Football,
            league: obj.get("league")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            home_team,
            away_team,
            start_time: None,
            is_live: obj.get("is_live")
                .or(obj.get("isLive"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "melbet".to_string(),
            raw_url: Some(format!("https://melbet.com/betting/{}", id)),
            extra: HashMap::new(),
        })
    }

    /// Парсит HTML линию
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
                    bookmaker_slug: "melbet".to_string(),
                    raw_url: Some(format!("https://melbet.com/betting/{}", id)),
                    extra: HashMap::new(),
                });
            }
        }
        
        None
    }
}
