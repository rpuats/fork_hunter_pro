use once_cell::sync::Lazy;
use regex::Regex;
use shared::Event;
use std::collections::HashMap;

static TEAM_ALIASES: Lazy<HashMap<&str, Vec<&str>>> = Lazy::new(|| {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    map.insert("Real Madrid", vec!["Реал Мадрид", "Реал", "Real Madrid CF"]);
    map.insert("Barcelona", vec!["Барселона", "Барса", "FC Barcelona"]);
    map.insert("Manchester United", vec!["Манчестер Юнайтед", "Ман Юнайтед", "Man Utd", "MUFC"]);
    map.insert("Manchester City", vec!["Манчестер Сити", "Ман Сити", "Man City", "MCFC"]);
    map.insert("Liverpool", vec!["Ливерпуль", "LFC"]);
    map.insert("Chelsea", vec!["Челси", "CFC"]);
    map.insert("Arsenal", vec!["Арсенал", "AFC"]);
    map.insert("Bayern Munich", vec!["Бавария", "Bayern", "FC Bayern"]);
    map.insert("PSG", vec!["ПСЖ", "Paris Saint-Germain", "Пари Сен-Жермен"]);
    map.insert("Juventus", vec!["Ювентус", "Juve"]);
    map.insert("AC Milan", vec!["Милан", "ACM"]);
    map.insert("Inter Milan", vec!["Интер", "Inter"]);
    map.insert("CSKA Moscow", vec!["ЦСКА", "ЦСКА Москва", "PFC CSKA"]);
    map.insert("Spartak Moscow", vec!["Спартак", "Спартак Москва", "FC Spartak"]);
    map.insert("Zenit", vec!["Зенит", "Зенит СПб", "FC Zenit"]);
    map.insert("Lokomotiv Moscow", vec!["Локомотив", "Локо Москва", "FC Lokomotiv"]);
    map
});

static CLEANUP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Zа-яА-Я0-9\s\-]").unwrap());
static EXTRA_SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

#[derive(Clone)]
pub struct Normalizer {
    aliases: HashMap<String, String>,
}

impl Normalizer {
    pub fn new() -> Self {
        let mut aliases: HashMap<String, String> = HashMap::new();
        for (canonical, alias_list) in TEAM_ALIASES.iter() {
            for alias in alias_list.iter() {
                aliases.insert(alias.to_lowercase(), canonical.to_string());
            }
            aliases.insert(canonical.to_lowercase(), canonical.to_string());
        }

        Self { aliases }
    }

    pub fn normalize_team(&self, team: &str) -> String {
        let cleaned = self.clean_team_name(team);
        let lower = cleaned.to_lowercase();

        if let Some(canonical) = self.aliases.get(&lower) {
            return canonical.clone();
        }

        for (alias, canonical) in &self.aliases {
            if lower.contains(alias) || alias.contains(&lower) {
                return canonical.clone();
            }
        }

        cleaned
    }

    pub fn normalize_event(&self, event: Event) -> Event {
        Event {
            id: event.id,
            sport: event.sport,
            league: self.normalize_league(&event.league),
            home_team: self.normalize_team(&event.home_team),
            away_team: self.normalize_team(&event.away_team),
            start_time: event.start_time,
            is_live: event.is_live,
            bookmaker_slug: event.bookmaker_slug,
            raw_url: event.raw_url,
            extra: event.extra,
        }
    }

    pub fn normalize_league(&self, league: &str) -> String {
        let league = league.trim().to_string();
        match league.to_lowercase().as_str() {
            "рпл" | "rpl" | "russian premier league" => "Russian Premier League".into(),
            "апл" | "epl" | "premier league" | "english premier league" => "Premier League".into(),
            "ла лига" | "la liga" | "primera division" => "La Liga".into(),
            "бундеслига" | "bundesliga" => "Bundesliga".into(),
            "серия а" | "serie a" => "Serie A".into(),
            "лига 1" | "ligue 1" => "Ligue 1".into(),
            "лч" | "ucl" | "champions league" | "uefa champions league" => "UEFA Champions League".into(),
            "ле" | "uel" | "europa league" | "uefa europa league" => "UEFA Europa League".into(),
            _ => league,
        }
    }

    pub fn events_match(&self, event_a: &Event, event_b: &Event) -> bool {
        if event_a.sport != event_b.sport {
            return false;
        }

        let home_a = self.normalize_team(&event_a.home_team);
        let away_a = self.normalize_team(&event_a.away_team);
        let home_b = self.normalize_team(&event_b.home_team);
        let away_b = self.normalize_team(&event_b.away_team);

        (home_a == home_b && away_a == away_b) || (home_a == away_b && away_a == home_b)
    }

    fn clean_team_name(&self, name: &str) -> String {
        let name = CLEANUP_RE.replace_all(name, "");
        let name = EXTRA_SPACE_RE.replace_all(&name, " ");
        name.trim().to_string()
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(home: &str, away: &str) -> Event {
        Event {
            id: "test".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: home.into(),
            away_team: away.into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_normalize_team_russian() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Реал Мадрид"), "Real Madrid");
        assert_eq!(norm.normalize_team("Барселона"), "Barcelona");
        assert_eq!(norm.normalize_team("Зенит"), "Zenit");
    }

    #[test]
    fn test_normalize_team_english() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Man Utd"), "Manchester United");
        assert_eq!(norm.normalize_team("Man City"), "Manchester City");
    }

    #[test]
    fn test_events_match() {
        let norm = Normalizer::new();
        let event_a = make_event("Реал Мадрид", "Барселона");
        let event_b = make_event("Real Madrid", "Barcelona");
        assert!(norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_events_not_match_different_sport() {
        let norm = Normalizer::new();
        let mut event_a = make_event("Team A", "Team B");
        event_a.sport = Sport::Football;
        let mut event_b = make_event("Team A", "Team B");
        event_b.sport = Sport::Tennis;
        assert!(!norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_normalize_league() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("рпл"), "Russian Premier League");
        assert_eq!(norm.normalize_league("ЛЧ"), "UEFA Champions League");
        assert_eq!(norm.normalize_league("Unknown League"), "Unknown League");
    }

    #[test]
    fn test_clean_team_name() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Team (FC)"), "Team FC");
        assert_eq!(norm.normalize_team("  Extra   Spaces  "), "Extra Spaces");
    }
}
