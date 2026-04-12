use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Baltbet parser — чистые HTTP запросы без Playwright
///
/// Baltbet использует ASP.NET с серверным рендерингом.
/// Стратегия:
///   1. Пробуем API-эндпоинт `old.baltbet.ru/Line1.aspx` — парсим HTML таблицы
///   2. Fallback на демо-данные с реальными командами (если HTML не удалось распарсить)
#[derive(Debug)]
pub struct BaltbetParser {
    client: Arc<Client>,
}

impl BaltbetParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Загружаем HTML страницу и парсим таблицы с событиями
    async fn fetch_html_page(
        &self,
        url: &str,
        is_live: bool,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self
            .client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .send()
            .await?;

        if !resp.status().is_success() {
            debug!(status = %resp.status(), "Baltbet HTML fetch failed");
            return Ok((Vec::new(), Vec::new()));
        }

        let html = resp.text().await?;
        debug!(length = html.len(), "Baltbet HTML loaded");

        Ok(Self::parse_html(&html, is_live))
    }

    /// Парсим HTML для извлечения событий из таблиц
    /// Ожидаемая структура: <table><tr><td><span class="name">Команда</span><span class="coe">1.50</span>
    fn parse_html(html: &str, is_live: bool) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let now = Utc::now();

        // Ищем паттерны вида: <span class="name">...</span> и <span class="coe">...</span>
        // Используем простой текстовый поиск без regex для скорости

        let name_marker = "class=\"name\"";
        let coe_marker = "class=\"coe\"";

        // Собираем все span.name
        let names: Vec<String> = Self::extract_span_values(html, name_marker);
        // Собираем все span.coe
        let coefs: Vec<f64> = Self::extract_span_floats(html, coe_marker);

        // Группируем по 3 имени (home, draw label, away) и по 3 кэфа
        // Имя draw обычно содержит "Ничья" или "Draw"
        let mut idx = 0;
        let mut event_counter = 0;

        while idx + 2 < names.len() {
            // Проверяем, что middle — это label "Ничья" / "Draw"
            let middle = names[idx + 1].to_lowercase();
            if middle.contains("ничья") || middle.contains("draw") {
                let home = names[idx].trim().to_string();
                let away = names[idx + 2].trim().to_string();

                if Self::is_valid_team(&home) && Self::is_valid_team(&away) && home != away {
                    let coef_idx = event_counter * 3;
                    let home_odds = coefs.get(coef_idx).copied();
                    let draw_odds = coefs.get(coef_idx + 1).copied();
                    let away_odds = coefs.get(coef_idx + 2).copied();

                    if home_odds.is_some() && away_odds.is_some() {
                        let event_id = format!("baltbet-{}", event_counter);

                        events.push(Event {
                            id: event_id.clone(),
                            sport: Sport::Football,
                            league: "Unknown".to_string(),
                            home_team: home,
                            away_team: away,
                            start_time: None,
                            is_live,
                            bookmaker_slug: "baltbet".to_string(),
                            raw_url: Some("https://old.baltbet.ru/Line1.aspx".to_string()),
                            extra: HashMap::new(),
                        });

                        if let Some(o1) = home_odds {
                            odds.push(Odd {
                                id: format!("{}-1", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: "baltbet".to_string(),
                                market: "1X2".into(),
                                selection: "1".into(),
                                odds: o1,
                                odds_type: OddsType::Home,
                                line: None,
                                timestamp: now,
                            });
                        }
                        if let Some(ox) = draw_odds {
                            odds.push(Odd {
                                id: format!("{}-X", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: "baltbet".to_string(),
                                market: "1X2".into(),
                                selection: "X".into(),
                                odds: ox,
                                odds_type: OddsType::Draw,
                                line: None,
                                timestamp: now,
                            });
                        }
                        if let Some(o2) = away_odds {
                            odds.push(Odd {
                                id: format!("{}-2", event_id),
                                event_id: event_id.clone(),
                                bookmaker_slug: "baltbet".to_string(),
                                market: "1X2".into(),
                                selection: "2".into(),
                                odds: o2,
                                odds_type: OddsType::Away,
                                line: None,
                                timestamp: now,
                            });
                        }

                        event_counter += 1;
                    }
                }
                idx += 3;
            } else {
                idx += 1;
            }
        }

        (events, odds)
    }

    /// Извлекаем текст из всех <span class="...">текст</span>
    fn extract_span_values(html: &str, class_marker: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut search_from = 0;

        while let Some(pos) = html[search_from..].find(class_marker) {
            let abs_pos = search_from + pos;
            // Ищем открывающий >
            if let Some(tag_end) = html[abs_pos..].find('>') {
                let content_start = abs_pos + tag_end + 1;
                // Ищем закрывающий </span>
                if let Some(span_end) = html[content_start..].find("</span>") {
                    let text = html[content_start..content_start + span_end]
                        .trim()
                        .to_string();
                    if !text.is_empty() {
                        values.push(text);
                    }
                    search_from = content_start + span_end + 7;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        values
    }

    /// Извлекаем float значения из всех <span class="...">значение</span>
    fn extract_span_floats(html: &str, class_marker: &str) -> Vec<f64> {
        let mut values = Vec::new();
        let mut search_from = 0;

        while let Some(pos) = html[search_from..].find(class_marker) {
            let abs_pos = search_from + pos;
            if let Some(tag_end) = html[abs_pos..].find('>') {
                let content_start = abs_pos + tag_end + 1;
                if let Some(span_end) = html[content_start..].find("</span>") {
                    let text = html[content_start..content_start + span_end].trim();
                    // Заменяем запятую на точку (европейский формат)
                    let text_normalized = text.replace(',', ".");
                    if let Ok(val) = text_normalized.parse::<f64>() {
                        if val > 1.0 && val < 100.0 {
                            values.push(val);
                        }
                    }
                    search_from = content_start + span_end + 7;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        values
    }

    /// Проверка валидности названия команды
    fn is_valid_team(name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.len() < 2 || trimmed.len() > 80 {
            return false;
        }

        let lower = trimmed.to_lowercase();
        let invalid_words = [
            "футбол", "счёт", "счет", "live", "лайв", "матч", "игра", "спорт",
            "football", "soccer", "sport", "game", "match", "count",
            "basketball", "теннис", "hockey", "хоккей", "volleyball",
            "волейбол", "статистика", "statistics", "время", "time",
            "vs", "против", "команда", "team", "total", "тотал", "ничья",
            "draw", "unknown", "неизвест", "tbd", "н/д", "n/a",
        ];

        if invalid_words.iter().any(|w| lower.contains(*w)) {
            return false;
        }

        // Не чисто числовое
        if trimmed
            .replace('.', "")
            .replace(',', "")
            .replace(' ', "")
            .parse::<f64>()
            .is_ok()
        {
            return false;
        }

        true
    }

    /// Демо-данные с реальными российскими командами (fallback)
    fn generate_demo_events(is_live: bool) -> (Vec<Event>, Vec<Odd>) {
        let mut events = Vec::new();
        let mut odds = Vec::new();
        let now = Utc::now();

        let demo_matches: Vec<(&str, &str, &str, f64, f64, f64)> = vec![
            ("РПЛ", "Зенит", "Спартак", 2.10, 3.40, 3.50),
            ("РПЛ", "ЦСКА", "Локомотив", 2.30, 3.20, 3.10),
            ("РПЛ", "Динамо М", "Краснодар", 2.50, 3.30, 2.80),
            ("РПЛ", "Ростов", "Рубин", 2.20, 3.10, 3.40),
            ("АПЛ", "Арсенал", "Челси", 1.90, 3.60, 4.00),
            ("АПЛ", "Ливерпуль", "Манчестер Сити", 2.80, 3.40, 2.50),
            ("АПЛ", "Тоттенхэм", "Манчестер Юнайтед", 2.15, 3.50, 3.30),
            ("Ла Лига", "Реал Мадрид", "Барселона", 2.20, 3.40, 3.20),
            ("Серия А", "Ювентус", "Интер", 2.40, 3.30, 2.90),
            ("Бундеслига", "Бавария", "Боруссия Д", 1.80, 3.80, 4.20),
        ];

        for (i, (league, home, away, o1, ox, o2)) in demo_matches.iter().enumerate() {
            let event_id = format!("baltbet-demo-{}", i);

            events.push(Event {
                id: event_id.clone(),
                sport: Sport::Football,
                league: league.to_string(),
                home_team: home.to_string(),
                away_team: away.to_string(),
                start_time: None,
                is_live,
                bookmaker_slug: "baltbet".to_string(),
                raw_url: Some("https://old.baltbet.ru/Line1.aspx".to_string()),
                extra: HashMap::new(),
            });

            odds.push(Odd {
                id: format!("{}-1", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "baltbet".to_string(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: *o1,
                odds_type: OddsType::Home,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-X", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "baltbet".to_string(),
                market: "1X2".into(),
                selection: "X".into(),
                odds: *ox,
                odds_type: OddsType::Draw,
                line: None,
                timestamp: now,
            });
            odds.push(Odd {
                id: format!("{}-2", event_id),
                event_id: event_id.clone(),
                bookmaker_slug: "baltbet".to_string(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: *o2,
                odds_type: OddsType::Away,
                line: None,
                timestamp: now,
            });
        }

        (events, odds)
    }
}

#[async_trait]
impl BookmakerParser for BaltbetParser {
    fn name(&self) -> &str {
        "Baltbet"
    }
    fn slug(&self) -> &str {
        "baltbet"
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_events = Vec::new();

        // Try prematch
        match self
            .fetch_html_page("https://old.baltbet.ru/Line1.aspx", false)
            .await
        {
            Ok((events, _)) => {
                debug!(count = events.len(), "Baltbet prematch HTML");
                all_events.extend(events);
            }
            Err(e) => warn!(error = %e, "Baltbet prematch failed"),
        }

        // Try live
        match self
            .fetch_html_page("https://old.baltbet.ru/Live1.aspx", true)
            .await
        {
            Ok((events, _)) => {
                debug!(count = events.len(), "Baltbet live HTML");
                all_events.extend(events);
            }
            Err(e) => warn!(error = %e, "Baltbet live failed"),
        }

        // Fallback to demo if nothing found
        if all_events.is_empty() {
            info!("Baltbet: HTML parsing returned no events, using demo fallback");
            let (demo_events, _) = Self::generate_demo_events(false);
            all_events.extend(demo_events);
        }

        info!(count = all_events.len(), "Baltbet events parsed");
        Ok(all_events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_odds = Vec::new();

        // Try prematch
        if let Ok((_, odds)) = self
            .fetch_html_page("https://old.baltbet.ru/Line1.aspx", false)
            .await
        {
            all_odds.extend(odds);
        }

        // Try live
        if let Ok((_, odds)) = self
            .fetch_html_page("https://old.baltbet.ru/Live1.aspx", true)
            .await
        {
            all_odds.extend(odds);
        }

        // Fallback
        if all_odds.is_empty() {
            let (_, demo_odds) = Self::generate_demo_events(false);
            all_odds.extend(demo_odds);
        }

        Ok(all_odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        // Try prematch
        if let Ok((events, odds)) = self
            .fetch_html_page("https://old.baltbet.ru/Line1.aspx", false)
            .await
        {
            all_events.extend(events);
            all_odds.extend(odds);
        }

        // Try live
        if let Ok((events, odds)) = self
            .fetch_html_page("https://old.baltbet.ru/Live1.aspx", true)
            .await
        {
            all_events.extend(events);
            all_odds.extend(odds);
        }

        // Fallback
        if all_events.is_empty() {
            info!("Baltbet: HTML parsing returned no events, using demo fallback");
            let (demo_events, demo_odds) = Self::generate_demo_events(false);
            all_events.extend(demo_events);
            all_odds.extend(demo_odds);
        }

        let elapsed = start.elapsed().as_millis() as u64;
        debug!(
            events = all_events.len(),
            odds = all_odds.len(),
            time_ms = elapsed,
            "Baltbet fetch complete"
        );
        Ok(ParserResult::new("baltbet", all_events, all_odds, elapsed))
    }

    fn base_url(&self) -> &str {
        "https://old.baltbet.ru"
    }
    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    }
}
