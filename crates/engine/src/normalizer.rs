use once_cell::sync::Lazy;
use regex::Regex;
use shared::Event;
use std::collections::HashMap;

static TEAM_ALIASES: Lazy<HashMap<&str, Vec<&str>>> = Lazy::new(|| {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    // Испанские
    map.insert("Real Madrid", vec!["Реал Мадрид", "Реал", "Real Madrid CF", "Real"]);
    map.insert("Barcelona", vec!["Барселона", "Барса", "FC Barcelona", "Barça"]);
    map.insert("Atletico Madrid", vec!["Атлетико", "Атлетико Мадрид", "Atlético"]);
    // Английские
    map.insert("Manchester United", vec!["Манчестер Юнайтед", "Ман Юнайтед", "Man Utd", "MUFC"]);
    map.insert("Manchester City", vec!["Манчестер Сити", "Ман Сити", "Man City", "MCFC"]);
    map.insert("Liverpool", vec!["Ливерпуль", "LFC"]);
    map.insert("Chelsea", vec!["Челси", "CFC"]);
    map.insert("Arsenal", vec!["Арсенал", "AFC", "Арсенал Лондон"]);
    map.insert("Tottenham", vec!["Тоттенхэм", "Spurs", "Тоттенхэм Хотспур"]);
    map.insert("Newcastle United", vec!["Ньюкасл", "Newcastle"]);
    // Немецкие
    map.insert("Bayern Munich", vec!["Бавария", "Bayern", "FC Bayern", "Бавария Мюнхен"]);
    map.insert("Borussia Dortmund", vec!["Боруссия Дортмунд", "BVB", "Боруссия Д"]);
    map.insert("RB Leipzig", vec!["РБ Лейпциг", "Leipzig", "Лейпциг"]);
    // Французские
    map.insert("PSG", vec!["ПСЖ", "Paris Saint-Germain", "Пари Сен-Жермен", "Париж"]);
    map.insert("Olympique Marseille", vec!["Олимпик Марсель", "Марсель", "OM"]);
    // Итальянские
    map.insert("Juventus", vec!["Ювентус", "Juve"]);
    map.insert("AC Milan", vec!["Милан", "ACM"]);
    map.insert("Inter Milan", vec!["Интер", "Inter", "Интер Милан"]);
    map.insert("AS Roma", vec!["Рома", "AS Roma", "А Рома"]);
    map.insert("Napoli", vec!["Наполи", "SSC Napoli"]);
    // Российские
    map.insert("CSKA Moscow", vec!["ЦСКА", "ЦСКА Москва", "PFC CSKA", "ЦСКА М"]);
    map.insert("Spartak Moscow", vec!["Спартак", "Спартак Москва", "FC Spartak"]);
    map.insert("Zenit", vec!["Зенит", "Зенит СПб", "FC Zenit", "Зенит Санкт-Петербург"]);
    map.insert("Lokomotiv Moscow", vec!["Локомотив", "Локо Москва", "FC Lokomotiv", "Локомотив М"]);
    map.insert("Dynamo Moscow", vec!["Динамо Москва", "Динамо М", "FC Dynamo"]);
    map.insert("Krasnodar", vec!["Краснодар", "FC Krasnodar"]);
    map.insert("Rostov", vec!["Ростов", "FC Rostov"]);
    map
});

static CLEANUP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Zа-яА-Я0-9\s\-]").unwrap());
static EXTRA_SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

/// Вычисление расстояния Левенштейна
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    
    if m == 0 { return n; }
    if n == 0 { return m; }
    
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i-1] == b_chars[j-1] { 0 } else { 1 };
            dp[i][j] = std::cmp::min(
                std::cmp::min(dp[i-1][j] + 1, dp[i][j-1] + 1),
                dp[i-1][j-1] + cost,
            );
        }
    }
    
    dp[m][n]
}

/// Проверка fuzzy совпадения с порогом расстояния
fn fuzzy_match(input: &str, candidates: &[&str], max_dist: usize) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut best = None;
    let mut best_dist = usize::MAX;
    
    for candidate in candidates {
        let cand_lower = candidate.to_lowercase();
        let dist = levenshtein(&input_lower, &cand_lower);
        if dist <= max_dist && dist < best_dist {
            best_dist = dist;
            best = Some(candidate.to_string());
        }
    }
    
    best
}

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

        // 1. Точное совпадение
        if let Some(canonical) = self.aliases.get(&lower) {
            return canonical.clone();
        }

        // 2. Частичное совпадение (contains)
        for (alias, canonical) in &self.aliases {
            if lower.contains(alias) || alias.contains(&lower) {
                return canonical.clone();
            }
        }

        // 3. Fuzzy matching с порогами
        let all_aliases: Vec<&str> = self.aliases.keys().map(|s| s.as_str()).collect();
        let max_dist = if cleaned.len() <= 4 { 1 } else if cleaned.len() <= 8 { 2 } else { 3 };
        
        if let Some(fuzzy) = fuzzy_match(&lower, &all_aliases, max_dist) {
            if let Some(canonical) = self.aliases.get(&fuzzy.to_lowercase()) {
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
        let lower = league.trim().to_lowercase();
        match lower.as_str() {
            // Russian Premier League
            "рпл" | "rpl" | "russian premier league" | "россия" | "российская премьер-лига" 
            => "Russian Premier League".into(),
            
            // English Premier League  
            "апл" | "epl" | "premier league" | "english premier league" | "англия"
            | "английская премьер-лига" | "английская премьер лига"
            => "Premier League".into(),
            
            // Spanish La Liga
            "ла лига" | "la liga" | "primera division" | "испания" | "примера"
            => "La Liga".into(),
            
            // German Bundesliga
            "бундеслига" | "bundesliga" | "германия"
            => "Bundesliga".into(),
            
            // Italian Serie A
            "серия а" | "serie a" | "италия"
            => "Serie A".into(),
            
            // French Ligue 1
            "лига 1" | "ligue 1" | "франция"
            => "Ligue 1".into(),
            
            // UEFA Champions League
            "лч" | "ucl" | "champions league" | "uefa champions league" | "лига чемпионов"
            => "UEFA Champions League".into(),
            
            // UEFA Europa League
            "ле" | "uel" | "europa league" | "uefa europa league" | "лига европы"
            => "UEFA Europa League".into(),
            
            // Russian Cup
            "кубок россии" | "russian cup"
            => "Russian Cup".into(),
            
            // Fallback: return original trimmed
            _ => league.trim().to_string(),
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

    #[test]
    fn test_fuzzy_matching() {
        let norm = Normalizer::new();
        // Опечатки
        assert_eq!(norm.normalize_team("Манчестр Юнайтед"), "Manchester United");
        assert_eq!(norm.normalize_team("Реал Мадри"), "Real Madrid");
        // Сокращения
        assert_eq!(norm.normalize_team("Барса"), "Barcelona");
        assert_eq!(norm.normalize_team("LFC"), "Liverpool");
        // Разные регистры
        assert_eq!(norm.normalize_team("real"), "Real Madrid");
        assert_eq!(norm.normalize_team("MAN UTD"), "Manchester United");
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
    }
}
