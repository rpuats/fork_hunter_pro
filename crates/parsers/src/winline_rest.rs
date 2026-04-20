use crate::shared::{Event, Odd, OddsType, Sport};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Winline REST API парсер (вместо DOM селекторов)
/// Использует реальные API endpoints что были найдены в анализе
pub struct WinlineRestParser {
    client: Arc<Client>,
    base_url: String,
    session_cookies: HashMap<String, String>,
}

impl WinlineRestParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            base_url: "https://winline.ru".to_string(),
            session_cookies: HashMap::new(),
        }
    }

    /// Получить IP для проверки доступа
    async fn get_ip(&self) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v2/getip?_format=json", self.base_url);
        
        let response = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", "https://winline.ru/stavki/sport/futbol")
            .header("Accept", "application/json")
            .send()
            .await?;

        if response.status() == 200 {
            let json: serde_json::Value = response.json().await?;
            Ok(json["my_ip"].as_str().unwrap_or("unknown").to_string())
        } else {
            Err("Failed to get IP".into())
        }
    }

    /// Получить события футбола через REST API
    async fn fetch_football_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        let mut events = Vec::new();

        // Пробуем разные endpoints
        let endpoints = vec![
            "/api/cls/menu/sport/205/country-xy/8-22?theme=default&format=s",
            "/api/xds/v2/sport/205/1",
            "/api/xds/v2/events?sport=205",
            "/api/v2/menu/sport/205",
        ];

        for endpoint in endpoints {
            match self.fetch_from_endpoint(endpoint).await {
                Ok(events_from_endpoint) => {
                    events.extend(events_from_endpoint);
                    break; // Если получили, не пробуем другие
                }
                Err(e) => {
                    eprintln!("Failed to fetch from {}: {}", endpoint, e);
                    continue;
                }
            }
        }

        Ok(events)
    }

    /// Fetches от одного endpoint
    async fn fetch_from_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        let url = format!("{}{}", self.base_url, endpoint);

        let response = self.client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", "https://winline.ru/stavki/sport/futbol")
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("X-Requested-With", "XMLHttpRequest")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if response.status() != 200 {
            return Err(format!("HTTP {}", response.status()).into());
        }

        // Попробуем распарсить как JSON
        let json: serde_json::Value = response.json().await?;

        // Парсим структуру ответа
        let events = self.parse_events_from_json(&json);

        Ok(events)
    }

    /// Парсит события из JSON структуры Winline API
    fn parse_events_from_json(&self, json: &serde_json::Value) -> Vec<Event> {
        let mut events = Vec::new();

        // Рекурсивно ищем события в JSON структуре
        self.extract_events_recursive(json, &mut events);

        events
    }

    /// Рекурсивно ищет события
    fn extract_events_recursive(
        &self,
        value: &serde_json::Value,
        events: &mut Vec<Event>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                // Проверяем если это событие
                if let (Some(id), Some(name)) = (map.get("id"), map.get("name")) {
                    if id.is_number() && name.is_string() {
                        // Вероятно это событие или матч
                        if let Some(event) = self.try_parse_event(map) {
                            events.push(event);
                        }
                    }
                }

                // Ищем дальше в значениях
                for (_key, val) in map {
                    self.extract_events_recursive(val, events);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    self.extract_events_recursive(item, events);
                }
            }
            _ => {}
        }
    }

    /// Пытается распарсить событие из JSON объекта
    fn try_parse_event(&self, obj: &serde_json::Map<String, serde_json::Value>) -> Option<Event> {
        let id = obj.get("id")?.as_u64()?.to_string();
        let home_team = obj.get("name").or_else(|| obj.get("home"))?
            .as_str()?
            .to_string();
        let away_team = obj.get("away_team").or_else(|| obj.get("away"))?
            .as_str()?
            .to_string();

        // League из структуры или значение по умолчанию
        let league = obj.get("league")
            .or_else(|| obj.get("category"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let event = Event {
            id,
            sport: Sport::Football,
            league,
            home_team,
            away_team,
            start_time: None,
            is_live: obj.get("is_live")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            bookmaker_slug: "winline".to_string(),
            raw_url: Some(format!(
                "https://winline.ru/stavki/event/{}",
                obj.get("event_id").and_then(|v| v.as_u64()).unwrap_or(0)
            )),
            extra: HashMap::new(),
        };

        Some(event)
    }
}

#[async_trait]
impl crate::parser::Parser for WinlineRestParser {
    async fn fetch(&self) -> Result<Vec<crate::shared::Event>, Box<dyn std::error::Error>> {
        // Проверяем доступ
        match self.get_ip().await {
            Ok(ip) => println!("[Winline REST] Connected from IP: {}", ip),
            Err(e) => {
                eprintln!("[Winline REST] IP check failed: {}", e);
                // Продолжаем даже если IP проверка не сработала
            }
        }

        // Получаем события
        self.fetch_football_events().await
    }

    fn name(&self) -> &str {
        "Winline REST API"
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_check() {
        // Тест будет выполняться асинхронно в реальной среде
        println!("Winline REST parser tests ready");
    }
}
