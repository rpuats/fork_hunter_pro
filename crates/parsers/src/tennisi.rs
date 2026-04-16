use crate::base::{BookmakerParser, ParserResult};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Utc};
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};

const BOOKMAKER_SLUG: &str = "tennisi";
const BASE_URL: &str = "https://tennisi.bet";
const LIVE_CATEGORY_ID: &str = "29010669";
const LIVE_LINES_URL: &str = "https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1";
const SPORT_PAGE_URL_TEMPLATE: &str = "https://tennisi.bet/sport/{slug}";
const CATEGORY_INFO_URL_TEMPLATE: &str = "https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid={category_id}&more=today&lang=rus";

#[derive(Clone, Copy, Debug)]
struct PrematchProbe {
    slug: &'static str,
    sport: Sport,
}

const PREMATCH_PROBES: &[PrematchProbe] = &[
    PrematchProbe {
        slug: "football",
        sport: Sport::Football,
    },
    PrematchProbe {
        slug: "hockey",
        sport: Sport::Hockey,
    },
    PrematchProbe {
        slug: "tennis",
        sport: Sport::Tennis,
    },
    PrematchProbe {
        slug: "basketball",
        sport: Sport::Basketball,
    },
    PrematchProbe {
        slug: "volleyball",
        sport: Sport::Volleyball,
    },
    PrematchProbe {
        slug: "cybersport",
        sport: Sport::Esports,
    },
    PrematchProbe {
        slug: "pingpong",
        sport: Sport::TableTennis,
    },
    PrematchProbe {
        slug: "baseball",
        sport: Sport::Baseball,
    },
    PrematchProbe {
        slug: "handball",
        sport: Sport::Handball,
    },
    PrematchProbe {
        slug: "waterpolo",
        sport: Sport::WaterPolo,
    },
    PrematchProbe {
        slug: "futsal",
        sport: Sport::Futsal,
    },
    PrematchProbe {
        slug: "rugby",
        sport: Sport::Rugby,
    },
    PrematchProbe {
        slug: "box",
        sport: Sport::Boxing,
    },
    PrematchProbe {
        slug: "billiard",
        sport: Sport::Snooker,
    },
    PrematchProbe {
        slug: "races",
        sport: Sport::Motorsport,
    },
    PrematchProbe {
        slug: "amfootball",
        sport: Sport::Other,
    },
    PrematchProbe {
        slug: "other",
        sport: Sport::Other,
    },
    PrematchProbe {
        slug: "trends",
        sport: Sport::Other,
    },
];

#[derive(Debug)]
pub struct TennisiParser {
    client: Arc<Client>,
}

impl TennisiParser {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn fetch_text(
        &self,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .get(url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .header("Referer", BASE_URL)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Tennisi returned HTTP {} for {url}", response.status()).into());
        }

        Ok(response.text().await?)
    }

    async fn discover_category_id(
        &self,
        slug: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let sport_page_url = SPORT_PAGE_URL_TEMPLATE.replace("{slug}", slug);
        let html = self.fetch_text(&sport_page_url).await?;
        let regex = Regex::new(r"categoryid=(\d+)")?;
        let category_id = regex
            .captures(&html)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
            .ok_or_else(|| format!("Tennisi category id not found for sport {slug}"))?;
        Ok(category_id)
    }

    async fn fetch_prematch_probe(
        &self,
        probe: PrematchProbe,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let category_id = self.discover_category_id(probe.slug).await?;
        let url = CATEGORY_INFO_URL_TEMPLATE.replace("{category_id}", &category_id);
        let html = self.fetch_text(&url).await?;
        Ok(Self::parse_prematch_page(
            &html,
            probe.sport,
            probe.slug,
            &category_id,
        ))
    }

    async fn fetch_live_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let html = self.fetch_text(LIVE_LINES_URL).await?;
        Ok(Self::parse_live_page(&html))
    }

    fn parse_prematch_page(
        html: &str,
        fallback_sport: Sport,
        sport_slug: &str,
        category_id: &str,
    ) -> (Vec<Event>, Vec<Odd>) {
        Self::parse_table_page(
            html,
            fallback_sport,
            false,
            Some(sport_slug),
            Some(category_id),
        )
    }

    fn parse_live_page(html: &str) -> (Vec<Event>, Vec<Odd>) {
        Self::parse_table_page(html, Sport::Other, true, None, Some(LIVE_CATEGORY_ID))
    }

    fn parse_table_page(
        html: &str,
        fallback_sport: Sport,
        is_live: bool,
        sport_slug: Option<&str>,
        category_id: Option<&str>,
    ) -> (Vec<Event>, Vec<Odd>) {
        let document = Html::parse_document(html);
        let row_selector = Selector::parse("tr").expect("valid selector");
        let cell_selector = Selector::parse("th, td").expect("valid selector");
        let title_selector = Selector::parse("a[id^='evtl']").expect("valid selector");
        let link_selector = Selector::parse("a[href]").expect("valid selector");

        let mut events = Vec::new();
        let mut odds = Vec::new();
        let mut current_league = String::new();
        let mut current_date = None;
        let mut current_headers: Vec<String> = Vec::new();
        let mut seen = HashSet::new();

        for row in document.select(&row_selector) {
            let row_id = row.value().attr("id").unwrap_or_default();
            let expanded_cells = Self::expand_row_cells(&row, &cell_selector);

            if row_id.starts_with("el") {
                let Some(event_id_suffix) = row_id.strip_prefix("el") else {
                    continue;
                };

                let title = row
                    .select(&title_selector)
                    .next()
                    .map(|node| node.text().collect::<String>())
                    .map(|value| Self::normalize_whitespace(&value))
                    .unwrap_or_default();
                let title = Self::sanitize_event_title(&title);
                let Some((home_team, away_team)) = Self::split_match_title(&title) else {
                    continue;
                };
                if !Self::is_valid_competitor(&home_team) || !Self::is_valid_competitor(&away_team)
                {
                    continue;
                }

                let league = if current_league.is_empty() {
                    "Unknown".to_string()
                } else {
                    current_league.clone()
                };
                let sport = if is_live {
                    Self::detect_live_sport(&league, &title)
                } else {
                    fallback_sport
                };
                let event_id = format!("{BOOKMAKER_SLUG}-{event_id_suffix}");
                let dedupe_key = format!("{}|{}|{}|{}", is_live, league, home_team, away_team);
                if !seen.insert(dedupe_key) {
                    continue;
                }

                let raw_url = row
                    .select(&link_selector)
                    .next()
                    .and_then(|node| node.value().attr("href"))
                    .map(Self::absolute_url)
                    .or_else(|| Some(BASE_URL.to_string()));

                let start_time = if is_live {
                    None
                } else {
                    expanded_cells.get(1).and_then(|value| {
                        Self::parse_prematch_start_time(current_date.as_deref(), value)
                    })
                };

                events.push(Event {
                    id: event_id.clone(),
                    sport,
                    league: league.clone(),
                    home_team,
                    away_team,
                    start_time,
                    is_live,
                    bookmaker_slug: BOOKMAKER_SLUG.to_string(),
                    raw_url,
                    extra: HashMap::new(),
                });

                odds.extend(Self::extract_primary_odds(
                    &event_id,
                    &current_headers,
                    &expanded_cells,
                    sport,
                ));
                continue;
            }

            let non_empty_cells = expanded_cells
                .iter()
                .filter(|value| !value.is_empty())
                .count();

            if non_empty_cells == 1 {
                let text = expanded_cells
                    .iter()
                    .find(|value| !value.is_empty())
                    .cloned()
                    .unwrap_or_default();
                if is_live {
                    if !Self::is_generic_live_filter_row(&text) {
                        current_league = text;
                    }
                } else if Self::is_date_marker(&text) {
                    current_date = Some(text);
                } else if !text.is_empty() {
                    current_league = text;
                }
                continue;
            }

            let has_headers = row
                .select(&Selector::parse("th").expect("valid selector"))
                .next()
                .is_some();
            if has_headers || non_empty_cells >= 8 {
                current_headers.clear();
                for (index, (text, span)) in Self::row_cells_with_colspan(&row, &cell_selector)
                    .into_iter()
                    .enumerate()
                {
                    if is_live
                        && index == 0
                        && span >= 2
                        && !text.is_empty()
                        && !Self::looks_like_market_label(&text)
                    {
                        current_league = text.clone();
                        for _ in 0..span {
                            current_headers.push(String::new());
                        }
                    } else {
                        for _ in 0..span {
                            current_headers.push(text.clone());
                        }
                    }
                }
            }
        }

        debug!(
            sport = sport_slug.unwrap_or("live"),
            category_id = category_id.unwrap_or_default(),
            events = events.len(),
            odds = odds.len(),
            is_live,
            "Tennisi page parsed"
        );

        (events, odds)
    }

    fn expand_row_cells(row: &ElementRef<'_>, cell_selector: &Selector) -> Vec<String> {
        let mut values = Vec::new();
        for (text, span) in Self::row_cells_with_colspan(row, cell_selector) {
            for _ in 0..span {
                values.push(text.clone());
            }
        }
        values
    }

    fn row_cells_with_colspan(
        row: &ElementRef<'_>,
        cell_selector: &Selector,
    ) -> Vec<(String, usize)> {
        row.select(cell_selector)
            .map(|cell| {
                let text = Self::normalize_whitespace(&cell.text().collect::<Vec<_>>().join(" "));
                let span = cell
                    .value()
                    .attr("colspan")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(1);
                (text, span)
            })
            .collect()
    }

    fn extract_primary_odds(
        event_id: &str,
        headers: &[String],
        cells: &[String],
        sport: Sport,
    ) -> Vec<Odd> {
        let header_map: HashMap<String, usize> = headers
            .iter()
            .enumerate()
            .filter_map(|(index, header)| {
                let canonical = Self::canonical_header(header);
                (!canonical.is_empty()).then_some((canonical, index))
            })
            .collect();

        let now = Utc::now();
        let mut odds = Vec::new();

        let home =
            Self::header_value(cells, &header_map, &["П1", "1"]).and_then(Self::parse_odds_value);
        let draw =
            Self::header_value(cells, &header_map, &["X", "Х"]).and_then(Self::parse_odds_value);
        let away =
            Self::header_value(cells, &header_map, &["П2", "2"]).and_then(Self::parse_odds_value);

        if let (Some(home), Some(away)) = (home, away) {
            let three_way = draw.is_some()
                || matches!(
                    sport,
                    Sport::Football
                        | Sport::Hockey
                        | Sport::WaterPolo
                        | Sport::Handball
                        | Sport::Futsal
                );
            if three_way && draw.is_some() {
                odds.push(Self::make_odd(
                    event_id,
                    "1",
                    "1X2",
                    "1",
                    OddsType::Home,
                    home,
                    None,
                    now,
                ));
                odds.push(Self::make_odd(
                    event_id,
                    "X",
                    "1X2",
                    "X",
                    OddsType::Draw,
                    draw.unwrap_or_default(),
                    None,
                    now,
                ));
                odds.push(Self::make_odd(
                    event_id,
                    "2",
                    "1X2",
                    "2",
                    OddsType::Away,
                    away,
                    None,
                    now,
                ));
            } else {
                odds.push(Self::make_odd(
                    event_id,
                    "1",
                    "Moneyline",
                    "1",
                    OddsType::Home,
                    home,
                    None,
                    now,
                ));
                odds.push(Self::make_odd(
                    event_id,
                    "2",
                    "Moneyline",
                    "2",
                    OddsType::Away,
                    away,
                    None,
                    now,
                ));
            }
        }

        let handicap_home_line =
            Self::header_value(cells, &header_map, &["Ф1"]).and_then(Self::parse_line_value);
        let handicap_home_odds =
            Self::header_value(cells, &header_map, &["К1"]).and_then(Self::parse_odds_value);
        if let (Some(line), Some(value)) = (handicap_home_line, handicap_home_odds) {
            odds.push(Self::make_odd(
                event_id,
                "hc1",
                "Handicap",
                "1",
                OddsType::Handicap,
                value,
                Some(line),
                now,
            ));
        }

        let handicap_away_line =
            Self::header_value(cells, &header_map, &["Ф2"]).and_then(Self::parse_line_value);
        let handicap_away_odds =
            Self::header_value(cells, &header_map, &["К2"]).and_then(Self::parse_odds_value);
        if let (Some(line), Some(value)) = (handicap_away_line, handicap_away_odds) {
            odds.push(Self::make_odd(
                event_id,
                "hc2",
                "Handicap",
                "2",
                OddsType::Handicap,
                value,
                Some(line),
                now,
            ));
        }

        let total_line = Self::header_value(cells, &header_map, &["TM", "ТМ", "TOTAL"])
            .and_then(Self::parse_line_value);
        let under_odds = Self::header_value(cells, &header_map, &["<", "М", "U", "UNDER"])
            .and_then(Self::parse_odds_value);
        if let (Some(line), Some(value)) = (total_line, under_odds) {
            odds.push(Self::make_odd(
                event_id,
                "under",
                "Total",
                "Under",
                OddsType::Under,
                value,
                Some(line),
                now,
            ));
        }

        let over_odds = Self::header_value(cells, &header_map, &[">", "Б", "O", "OVER"])
            .and_then(Self::parse_odds_value);
        if let (Some(line), Some(value)) = (total_line, over_odds) {
            odds.push(Self::make_odd(
                event_id,
                "over",
                "Total",
                "Over",
                OddsType::Over,
                value,
                Some(line),
                now,
            ));
        }

        odds
    }

    fn header_value<'a>(
        cells: &'a [String],
        header_map: &HashMap<String, usize>,
        keys: &[&str],
    ) -> Option<&'a str> {
        keys.iter().find_map(|key| {
            header_map
                .get(&Self::canonical_header(key))
                .and_then(|index| cells.get(*index))
                .map(String::as_str)
        })
    }

    fn make_odd(
        event_id: &str,
        suffix: &str,
        market: &str,
        selection: &str,
        odds_type: OddsType,
        value: f64,
        line: Option<f64>,
        timestamp: DateTime<Utc>,
    ) -> Odd {
        Odd {
            id: format!("{event_id}-{suffix}"),
            event_id: event_id.to_string(),
            bookmaker_slug: BOOKMAKER_SLUG.to_string(),
            market: market.to_string(),
            selection: selection.to_string(),
            odds: value,
            odds_type,
            line,
            timestamp,
        }
    }

    fn canonical_header(value: &str) -> String {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != '\u{a0}')
            .collect::<String>()
            .to_uppercase()
    }

    fn parse_odds_value(value: &str) -> Option<f64> {
        let parsed = value.trim().replace(',', ".").parse::<f64>().ok()?;
        (1.01..=100.0).contains(&parsed).then_some(parsed)
    }

    fn parse_line_value(value: &str) -> Option<f64> {
        let normalized = value.trim().replace(',', ".");
        let parsed = normalized.parse::<f64>().ok()?;
        (parsed.abs() <= 1000.0).then_some(parsed)
    }

    fn normalize_whitespace(value: &str) -> String {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn sanitize_event_title(value: &str) -> String {
        let mut cleaned = value.to_string();
        for suffix in [
            ". Специальные ставки",
            " Специальные ставки",
            ". Спец. ставки",
            " Спец. ставки",
        ] {
            if cleaned.ends_with(suffix) {
                cleaned.truncate(cleaned.len().saturating_sub(suffix.len()));
            }
        }
        Self::normalize_whitespace(&cleaned)
    }

    fn split_match_title(title: &str) -> Option<(String, String)> {
        for separator in [" - ", " – ", " — ", " vs ", " VS ", " v "] {
            let parts: Vec<&str> = title.splitn(2, separator).collect();
            if parts.len() == 2 {
                let home = Self::normalize_whitespace(parts[0]);
                let away = Self::normalize_whitespace(parts[1]);
                if !home.is_empty() && !away.is_empty() && home != away {
                    return Some((home, away));
                }
            }
        }
        None
    }

    fn is_valid_competitor(name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.len() < 2 || trimmed.len() > 120 {
            return false;
        }

        let lower = trimmed.to_lowercase();
        let invalid_exact = [
            "live",
            "матч",
            "событие",
            "ставки",
            "specials",
            "специальные ставки",
            "unknown",
            "tbd",
            "n/a",
        ];
        if invalid_exact.iter().any(|item| lower == *item) {
            return false;
        }

        !trimmed
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-' || ch.is_whitespace())
    }

    fn absolute_url(path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else if path.starts_with('/') {
            format!("{BASE_URL}{path}")
        } else {
            format!("{BASE_URL}/rt/cgi/{path}")
        }
    }

    fn is_date_marker(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("сегодня")
            || lower.contains("завтра")
            || Regex::new(r"\b\d{1,2}[./]\d{1,2}(?:[./]\d{2,4})?\b")
                .expect("valid regex")
                .is_match(text)
    }

    fn parse_prematch_start_time(
        date_marker: Option<&str>,
        time_text: &str,
    ) -> Option<DateTime<Utc>> {
        let time = time_text.trim();
        let time_parts: Vec<&str> = time.split(':').collect();
        if time_parts.len() != 2 {
            return None;
        }
        let hour = time_parts[0].parse::<u32>().ok()?;
        let minute = time_parts[1].parse::<u32>().ok()?;
        let moscow = FixedOffset::east_opt(3 * 3600)?;
        let now = Utc::now().with_timezone(&moscow);

        let base_date = match date_marker.map(|value| value.to_lowercase()) {
            Some(marker) if marker.contains("сегодня") => now.date_naive(),
            Some(marker) if marker.contains("завтра") => now.date_naive() + Duration::days(1),
            Some(marker) => Self::parse_explicit_date(&marker, now.year())?,
            None => now.date_naive(),
        };

        let naive = base_date.and_hms_opt(hour, minute, 0)?;
        moscow
            .from_local_datetime(&naive)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
    }

    fn parse_explicit_date(marker: &str, default_year: i32) -> Option<NaiveDate> {
        let regex = Regex::new(r"(\d{1,2})[./](\d{1,2})(?:[./](\d{2,4}))?").expect("valid regex");
        let captures = regex.captures(marker)?;
        let day = captures.get(1)?.as_str().parse::<u32>().ok()?;
        let month = captures.get(2)?.as_str().parse::<u32>().ok()?;
        let year = captures
            .get(3)
            .and_then(|value| value.as_str().parse::<i32>().ok())
            .map(|value| if value < 100 { 2000 + value } else { value })
            .unwrap_or(default_year);
        NaiveDate::from_ymd_opt(year, month, day)
    }

    fn is_generic_live_filter_row(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("выключить") || lower.contains("включить") || lower == "- все + все"
    }

    fn looks_like_market_label(text: &str) -> bool {
        matches!(
            Self::canonical_header(text).as_str(),
            "№" | "ДАТА"
                | "СОБЫТИЕ"
                | "П1"
                | "П2"
                | "X"
                | "Х"
                | "1X"
                | "12"
                | "2X"
                | "Ф1"
                | "Ф2"
                | "К1"
                | "К2"
                | "TM"
                | "ТМ"
                | "<"
                | ">"
                | "ДОП"
        )
    }

    fn detect_live_sport(league: &str, title: &str) -> Sport {
        let probe = format!("{} {}", league, title).to_lowercase();
        if probe.contains("настоль")
            || probe.contains("table tennis")
            || probe.contains("ping pong")
        {
            Sport::TableTennis
        } else if probe.contains("теннис") || probe.contains("tennis") {
            Sport::Tennis
        } else if probe.contains("кибер")
            || probe.contains("fifa")
            || probe.contains("esports")
            || probe.contains("cyber")
        {
            Sport::Esports
        } else if probe.contains("баскет") || probe.contains("basket") {
            Sport::Basketball
        } else if probe.contains("хоккей") || probe.contains("hockey") {
            Sport::Hockey
        } else if probe.contains("волей") || probe.contains("volley") {
            Sport::Volleyball
        } else if probe.contains("водное поло") || probe.contains("water polo") {
            Sport::WaterPolo
        } else if probe.contains("гандбол") || probe.contains("handball") {
            Sport::Handball
        } else if probe.contains("бейсбол") || probe.contains("baseball") {
            Sport::Baseball
        } else if probe.contains("футзал") || probe.contains("futsal") {
            Sport::Futsal
        } else if probe.contains("регби") || probe.contains("rugby") {
            Sport::Rugby
        } else if probe.contains("бокс") || probe.contains("boxing") {
            Sport::Boxing
        } else if probe.contains("футбол") || probe.contains("soccer") || probe.contains("football")
        {
            Sport::Football
        } else {
            Sport::Other
        }
    }

    pub(crate) async fn fetch_runtime_data(
        &self,
    ) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
        let result = self.fetch_all().await?;
        Ok((result.events, result.odds))
    }
}

#[async_trait]
impl BookmakerParser for TennisiParser {
    fn name(&self) -> &str {
        "Tennisi"
    }

    fn slug(&self) -> &str {
        BOOKMAKER_SLUG
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
        let (events, _) = self.fetch_runtime_data().await?;
        Ok(events)
    }

    async fn fetch_odds(
        &self,
        _event_id: &str,
    ) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {
        let (_, odds) = self.fetch_runtime_data().await?;
        Ok(odds)
    }

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();
        let mut seen_events = HashSet::new();
        let mut seen_odds = HashSet::new();

        match self.fetch_live_runtime_data().await {
            Ok((events, odds)) => {
                for event in events {
                    if seen_events.insert(event.id.clone()) {
                        all_events.push(event);
                    }
                }
                for odd in odds {
                    if seen_odds.insert(odd.id.clone()) {
                        all_odds.push(odd);
                    }
                }
            }
            Err(error) => warn!(error = %error, "Tennisi live fetch failed"),
        }

        for probe in PREMATCH_PROBES {
            match self.fetch_prematch_probe(*probe).await {
                Ok((events, odds)) => {
                    for event in events {
                        if seen_events.insert(event.id.clone()) {
                            all_events.push(event);
                        }
                    }
                    for odd in odds {
                        if seen_odds.insert(odd.id.clone()) {
                            all_odds.push(odd);
                        }
                    }
                }
                Err(error) => {
                    warn!(error = %error, sport = probe.slug, "Tennisi prematch fetch failed")
                }
            }
        }

        let elapsed = started.elapsed().as_millis() as u64;
        let live_count = all_events.iter().filter(|event| event.is_live).count();
        let prematch_count = all_events.len().saturating_sub(live_count);
        info!(
            events = all_events.len(),
            live = live_count,
            prematch = prematch_count,
            odds = all_odds.len(),
            time_ms = elapsed,
            "Tennisi fetch complete"
        );

        Ok(ParserResult::new(
            BOOKMAKER_SLUG,
            all_events,
            all_odds,
            elapsed,
        ))
    }

    fn base_url(&self) -> &str {
        BASE_URL
    }

    fn user_agent(&self) -> &str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    }
}

#[cfg(test)]
mod tests {
    use super::TennisiParser;
    use shared::OddsType;
    use shared::Sport;

    #[test]
    fn parses_prematch_fixture_with_primary_markets() {
        let html = include_str!("../tests/fixtures/tennisi_prematch_fixture.html");

        let (events, odds) =
            TennisiParser::parse_prematch_page(html, Sport::Football, "football", "137");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "tennisi-2306692312");
        assert_eq!(events[0].league, "Футбол :: Синтетические матчи");
        assert_eq!(events[0].home_team, "Лацио");
        assert_eq!(events[0].away_team, "Лидс");
        assert!(!events[0].is_live);
        assert!(events[0].start_time.is_some());

        assert!(odds.iter().any(|odd| {
            odd.market == "1X2"
                && odd.selection == "1"
                && odd.odds_type == OddsType::Home
                && (odd.odds - 2.54).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.market == "Handicap"
                && odd.selection == "2"
                && odd.line == Some(0.0)
                && (odd.odds - 2.01).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.market == "Total"
                && odd.selection == "Under"
                && odd.line == Some(2.0)
                && (odd.odds - 1.76).abs() < f64::EPSILON
        }));
    }

    #[test]
    fn parses_live_fixture_and_detects_sport() {
        let html = include_str!("../tests/fixtures/tennisi_live_fixture.html");

        let (events, odds) = TennisiParser::parse_live_page(html);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "tennisi-2308764195");
        assert_eq!(events[0].sport, Sport::Esports);
        assert_eq!(
            events[0].league,
            "11. Киберфутбол. FIFA 25. TCSL (2 по 6 минут)"
        );
        assert!(events[0].is_live);

        assert!(odds.iter().any(|odd| {
            odd.market == "1X2"
                && odd.selection == "X"
                && odd.odds_type == OddsType::Draw
                && (odd.odds - 5.85).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.market == "Handicap"
                && odd.selection == "1"
                && odd.line == Some(-1.5)
                && (odd.odds - 1.83).abs() < f64::EPSILON
        }));
        assert!(odds.iter().any(|odd| {
            odd.market == "Total"
                && odd.selection == "Over"
                && odd.line == Some(3.5)
                && (odd.odds - 1.70).abs() < f64::EPSILON
        }));
    }
}
