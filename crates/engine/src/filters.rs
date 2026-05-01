//! Filter System - Advanced filtering for forks
//! Top/Extended/Other leagues, exclusions, profit ranges

use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use uuid::Uuid;
use crate::fork_finder::{Fork, ForkLeg, ForkType};

/// Leagues filter levels (like Forking)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LeaguesFilter {
    Top,      // Only top leagues
    Extended, // Top + second tier
    All,      // All leagues including minor
}

impl Default for LeaguesFilter {
    fn default() -> Self {
        LeaguesFilter::Extended
    }
}

/// Filter preset for fork detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    
    // Profit filters
    pub min_profit: Decimal,
    pub max_profit: Decimal,
    
    // Odds filters
    pub min_odds: Decimal,
    pub max_odds: Decimal,
    
    // Sports filter (empty = all)
    pub sports: Vec<String>,
    
    // Leagues filter
    pub leagues_filter: LeaguesFilter,
    pub custom_leagues: Vec<String>, // For manual selection
    
    // Bookmakers filter (empty = all)
    pub bookmakers: Vec<String>,
    pub excluded_bookmakers: Vec<String>,
    
    // Exclusions
    pub exclude_women: bool,
    pub exclude_youth: bool,
    pub exclude_friendly: bool,
    pub exclude_tennis_doubles: bool,
    pub exclude_esports: bool,
    pub exclude_cyber_sports: bool,
    
    // Time filters (for prematch)
    pub time_to_match_min_minutes: Option<u32>,
    pub time_to_match_max_minutes: Option<u32>,
    
    // Stake limits
    pub max_stake_per_leg: Decimal,
    pub max_total_stake: Decimal,
    pub max_percent_of_bankroll: Decimal,
    
    // Fork types
    pub enabled_fork_types: Vec<ForkType>,
    pub enable_negative_forks: bool,
    pub max_negative_profit: Decimal,
    
    // Live/prematch
    pub include_live: bool,
    pub include_prematch: bool,
    
    // Advanced
    pub max_age_seconds: u64, // Filter out stale forks
    pub require_verified: bool, // Only verified forks
}

impl Default for FilterPreset {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Standard".to_string(),
            description: "Standard filter for regular betting".to_string(),
            min_profit: Decimal::from(5) / Decimal::from(10), // 0.5%
            max_profit: Decimal::from(15),
            min_odds: Decimal::from(105) / Decimal::from(100), // 1.05
            max_odds: Decimal::from(50),
            sports: vec![],
            leagues_filter: LeaguesFilter::Extended,
            custom_leagues: vec![],
            bookmakers: vec![],
            excluded_bookmakers: vec![],
            exclude_women: true,
            exclude_youth: true,
            exclude_friendly: true,
            exclude_tennis_doubles: false,
            exclude_esports: false,
            exclude_cyber_sports: false,
            time_to_match_min_minutes: Some(5),  // At least 5 min before start
            time_to_match_max_minutes: None,
            max_stake_per_leg: Decimal::from(500000), // 500k RUB
            max_total_stake: Decimal::from(1000000), // 1M RUB
            max_percent_of_bankroll: Decimal::from(5) / Decimal::from(100), // 5%
            enabled_fork_types: vec![
                ForkType::MatchWinner1X2,
                ForkType::TotalOverUnder,
                ForkType::Handicap,
            ],
            enable_negative_forks: false,
            max_negative_profit: Decimal::from(-2),
            include_live: true,
            include_prematch: true,
            max_age_seconds: 60,
            require_verified: false,
        }
    }
}

impl FilterPreset {
    /// Create a bonus hunting preset (negative forks)
    pub fn bonus_hunting() -> Self {
        Self {
            id: "bonus".to_string(),
            name: "Bonus Hunting".to_string(),
            description: "For wagering bonuses with minimal loss".to_string(),
            min_profit: Decimal::from(-2),
            max_profit: Decimal::from(5) / Decimal::from(10), // 0.5%
            enable_negative_forks: true,
            max_negative_profit: Decimal::from(-2),
            exclude_friendly: true,
            include_live: false, // Prematch only for bonus hunting
            max_age_seconds: 300,
            ..Default::default()
        }
    }
    
    /// Create a high profit preset (rare but valuable forks)
    pub fn high_profit() -> Self {
        Self {
            id: "high_profit".to_string(),
            name: "High Profit".to_string(),
            description: "Only high-profit opportunities".to_string(),
            min_profit: Decimal::from(15) / Decimal::from(10), // 1.5%
            leagues_filter: LeaguesFilter::All,
            exclude_friendly: true,
            ..Default::default()
        }
    }
    
    /// Create a safe preset (only top leagues, verified forks)
    pub fn safe() -> Self {
        Self {
            id: "safe".to_string(),
            name: "Safe Mode".to_string(),
            description: "Conservative settings for beginners".to_string(),
            min_profit: Decimal::ONE,
            max_profit: Decimal::from(5),
            leagues_filter: LeaguesFilter::Top,
            exclude_women: true,
            exclude_youth: true,
            exclude_friendly: true,
            exclude_tennis_doubles: true,
            require_verified: true,
            max_age_seconds: 30,
            ..Default::default()
        }
    }
    
    /// Create a live-only preset
    pub fn live_only() -> Self {
        Self {
            id: "live".to_string(),
            name: "Live Only".to_string(),
            description: "Only live events".to_string(),
            include_live: true,
            include_prematch: false,
            max_age_seconds: 15, // Very fresh for live
            ..Default::default()
        }
    }
}

/// Filter engine
pub struct ForkFilter {
    presets: Vec<FilterPreset>,
}

impl ForkFilter {
    pub fn new() -> Self {
        Self {
            presets: vec![
                FilterPreset::default(),
                FilterPreset::bonus_hunting(),
                FilterPreset::high_profit(),
                FilterPreset::safe(),
                FilterPreset::live_only(),
            ],
        }
    }
    
    /// Apply filter to a fork
    pub fn apply(&self, fork: &Fork, preset: &FilterPreset) -> bool {
        // Profit range
        if fork.profit_percent < preset.min_profit || fork.profit_percent > preset.max_profit {
            return false;
        }
        
        // Negative forks check
        if fork.profit_percent < Decimal::ZERO {
            if !preset.enable_negative_forks {
                return false;
            }
            if fork.profit_percent < preset.max_negative_profit {
                return false;
            }
        }
        
        // Odds range for each leg
        for leg in &fork.legs {
            if leg.odds < preset.min_odds || leg.odds > preset.max_odds {
                return false;
            }
        }
        
        // Sports filter
        if !preset.sports.is_empty() && !preset.sports.contains(&fork.sport) {
            return false;
        }
        
        // Leagues filter
        match preset.leagues_filter {
            LeaguesFilter::Top => {
                if !TOP_LEAGUES.contains(&fork.league.as_str()) {
                    return false;
                }
            },
            LeaguesFilter::Extended => {
                if !EXTENDED_LEAGUES.contains(&fork.league.as_str()) {
                    return false;
                }
            },
            LeaguesFilter::All => {}
        }
        
        // Custom leagues (override)
        if !preset.custom_leagues.is_empty() {
            if !preset.custom_leagues.contains(&fork.league) {
                return false;
            }
        }
        
        // Bookmakers filter
        if !preset.bookmakers.is_empty() {
            for leg in &fork.legs {
                if !preset.bookmakers.contains(&leg.bookmaker_slug) {
                    return false;
                }
            }
        }
        
        // Excluded bookmakers
        for leg in &fork.legs {
            if preset.excluded_bookmakers.contains(&leg.bookmaker_slug) {
                return false;
            }
        }
        
        // Exclusions
        let league_lower = fork.league.to_lowercase();
        
        if preset.exclude_women {
            if league_lower.contains("women") ||
               league_lower.contains("w.") ||
               league_lower.contains("жен") ||
               league_lower.contains("wta") && fork.sport.to_lowercase() != "tennis" {
                return false;
            }
        }
        
        if preset.exclude_youth {
            if league_lower.contains("youth") ||
               league_lower.contains("u19") ||
               league_lower.contains("u20") ||
               league_lower.contains("u21") ||
               league_lower.contains("u23") ||
               league_lower.contains("молод") ||
               league_lower.contains("reserves") {
                return false;
            }
        }
        
        if preset.exclude_friendly {
            if league_lower.contains("friendly") ||
               league_lower.contains("товарищ") ||
               league_lower.contains("exhibition") {
                return false;
            }
        }
        
        if preset.exclude_tennis_doubles && fork.sport.to_lowercase() == "tennis" {
            if league_lower.contains("double") ||
               league_lower.contains("парный") ||
               league_lower.contains("doubles") {
                return false;
            }
        }
        
        if preset.exclude_esports {
            if fork.sport.to_lowercase().contains("esport") ||
               fork.sport.to_lowercase().contains("cyber") {
                return false;
            }
        }
        
        if preset.exclude_cyber_sports {
            if fork.sport.to_lowercase().contains("cyber") ||
               fork.sport.to_lowercase().contains("кибер") {
                return false;
            }
        }
        
        // Live/prematch
        if fork.is_live && !preset.include_live {
            return false;
        }
        if !fork.is_live && !preset.include_prematch {
            return false;
        }
        
        // Time to match (for prematch)
        if !fork.is_live {
            if let Some(match_time) = &fork.match_time {
                if let Some(minutes) = parse_minutes(match_time) {
                    if let Some(min) = preset.time_to_match_min_minutes {
                        if minutes < min {
                            return false;
                        }
                    }
                    if let Some(max) = preset.time_to_match_max_minutes {
                        if minutes > max {
                            return false;
                        }
                    }
                }
            }
        }
        
        // Age filter
        if fork.age_ms / 1000 > preset.max_age_seconds {
            return false;
        }
        
        // Fork type filter
        if !preset.enabled_fork_types.is_empty() {
            if !preset.enabled_fork_types.contains(&fork.fork_type) {
                return false;
            }
        }
        
        true
    }
    
    /// Filter a list of forks
    pub fn filter_forks(&self, forks: &[Fork], preset: &FilterPreset) -> Vec<Fork> {
        forks.iter()
            .filter(|f| self.apply(f, preset))
            .cloned()
            .collect()
    }
    
    /// Get all presets
    pub fn get_presets(&self) -> &[FilterPreset] {
        &self.presets
    }
    
    /// Add custom preset
    pub fn add_preset(&mut self, preset: FilterPreset) {
        self.presets.push(preset);
    }
}

impl Default for ForkFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Top leagues (high liquidity, reliable)
pub static TOP_LEAGUES: &[&str] = &[
    // Football
    "Premier League", "La Liga", "Serie A", "Bundesliga", "Ligue 1",
    "Champions League", "Europa League", "Europa Conference League",
    "РПЛ", "Russian Premier League",
    "FNL", "First Division",
    // Tennis
    "ATP", "WTA", "Grand Slam", 
    // Basketball
    "NBA", "Euroleague", "Eurocup",
    // Hockey
    "NHL", "KHL", "AHL",
    // Esports major tournaments
    "LCS", "LEC", "LCK", "LPL", "The International", "Major",
];

/// Extended leagues (good liquidity, slightly less reliable)
pub static EXTENDED_LEAGUES: &[&str] = &[
    // Football extended
    "Championship", "La Liga 2", "Serie B", "2. Bundesliga", "Ligue 2",
    "A-League", "MLS", "J1 League", "K-League",
    "Segunda Division", "Primeira Liga", "Eredivisie",
    "Scottish Premiership", "Championship",
    // Tennis extended
    "ATP Challenger", "ATP 250", "ATP 500", "WTA 250", "WTA 500",
    "ITF", 
    // Hockey extended
    "VHL", "SHL", "Liiga", "DEL", "NLA",
    // Basketball extended
    "ACB", "Lega Basket", "VTB League", "Liga ABA",
    // Include all top leagues
    "Premier League", "La Liga", "Serie A", "Bundesliga", "Ligue 1",
    "Champions League", "Europa League", "РПЛ", "FNL",
    "ATP", "WTA", "Grand Slam",
    "NBA", "Euroleague",
    "NHL", "KHL", "AHL",
];

/// Parse match time string to minutes
fn parse_minutes(time_str: &str) -> Option<u32> {
    // Parse strings like "15:30", "15h 30m", "in 15 minutes", "tomorrow"
    let lower = time_str.to_lowercase();
    
    // Try "HH:MM" format
    if let Some(pos) = lower.find(':') {
        if let Ok(hours) = lower[..pos].trim().parse::<u32>() {
            if let Ok(mins) = lower[pos+1..].split_whitespace().next()?.parse::<u32>() {
                return Some(hours * 60 + mins);
            }
        }
    }
    
    // Try "in X minutes"
    if lower.contains("in") && lower.contains("min") {
        let parts: Vec<&str> = lower.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "in" && i + 1 < parts.len() {
                if let Ok(mins) = parts[i + 1].parse::<u32>() {
                    return Some(mins);
                }
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_preset_allows_reasonable_fork() {
        let preset = FilterPreset::default();
        let filter = ForkFilter::new();
        
        let fork = Fork {
            id: Uuid::new_v4(),
            event_id: "test".to_string(),
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            league: "Premier League".to_string(),
            sport: "football".to_string(),
            is_live: false,
            match_time: Some("60".to_string()),
            profit_percent: Decimal::from_f64(1.5).unwrap(),
            legs: vec![
                ForkLeg {
                    bookmaker_slug: "Pari".to_string(),
                    market: "1X2".to_string(),
                    selection: "P1".to_string(),
                    odds: 2.10_f64,
                    event_id: "test".to_string(),
                    original_event_id: "test".to_string(),
                },
                ForkLeg {
                    bookmaker_slug: "Fonbet".to_string(),
                    market: "1X2".to_string(),
                    selection: "X2".to_string(),
                    odds: 1.95_f64,
                    event_id: "test".to_string(),
                    original_event_id: "test".to_string(),
                },
            ],
            fork_type: ForkType::MatchWinner12,
            created_at: chrono::Utc::now(),
            age_ms: 10,
        };
        
        assert!(filter.apply(&fork, &preset));
    }
    
    #[test]
    fn test_excludes_women_leagues() {
        let preset = FilterPreset {
            exclude_women: true,
            ..Default::default()
        };
        let filter = ForkFilter::new();
        
        let mut fork = create_test_fork();
        fork.league = "Women Premier League".to_string();
        
        assert!(!filter.apply(&fork, &preset));
    }
    
    #[test]
    fn test_excludes_low_profit() {
        let preset = FilterPreset {
            min_profit: Decimal::ONE,
            ..Default::default()
        };
        let filter = ForkFilter::new();
        
        let mut fork = create_test_fork();
        fork.profit_percent = Decimal::from(5) / Decimal::from(10); // 0.5%
        
        assert!(!filter.apply(&fork, &preset));
    }
    
    fn create_test_fork() -> Fork {
        Fork {
            id: Uuid::new_v4(),
            event_id: "test".to_string(),
            home_team: "Team A".to_string(),
            away_team: "Team B".to_string(),
            league: "Premier League".to_string(),
            sport: "football".to_string(),
            is_live: false,
            match_time: Some("60".to_string()),
            profit_percent: Decimal::from_f64(1.5).unwrap(),
            legs: vec![
                ForkLeg {
                    bookmaker_slug: "Pari".to_string(),
                    market: "1X2".to_string(),
                    selection: "P1".to_string(),
                    odds: 2.10_f64,
                    event_id: "test".to_string(),
                    original_event_id: "test".to_string(),
                },
                ForkLeg {
                    bookmaker_slug: "Fonbet".to_string(),
                    market: "1X2".to_string(),
                    selection: "X2".to_string(),
                    odds: 1.95_f64,
                    event_id: "test".to_string(),
                    original_event_id: "test".to_string(),
                },
            ],
            fork_type: ForkType::MatchWinner12,
            created_at: chrono::Utc::now(),
            age_ms: 10,
        }
    }
}
