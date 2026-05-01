//! Fork Finder Engine - Professional fork detection
//! Implements 1X2, totals, handicaps, BTTS, and corridor detection

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use shared::{Event, Odd};

/// Configuration for fork detection
#[derive(Debug, Clone)]
pub struct ForkConfig {
    /// Minimum profit percentage to report
    pub min_profit: Decimal,
    /// Maximum profit percentage (filter out errors)
    pub max_profit: Decimal,
    /// Minimum odds for any leg
    pub min_odds: Decimal,
    /// Maximum odds for any leg
    pub max_odds: Decimal,
    /// Enable corridor detection
    pub enable_corridors: bool,
    /// Enable negative forks (for bonus hunting)
    pub enable_negative_forks: bool,
    /// Max negative profit allowed (e.g., -2%)
    pub max_negative_profit: Decimal,
}

impl Default for ForkConfig {
    fn default() -> Self {
        Self {
            min_profit: Decimal::ZERO + Decimal::from(5) / Decimal::from(10), // 0.5%
            max_profit: Decimal::from(50), // Filter out obvious errors
            min_odds: Decimal::ONE + Decimal::ONE / Decimal::from(100),
            max_odds: Decimal::from(100),
            enable_corridors: true,
            enable_negative_forks: false,
            max_negative_profit: Decimal::from(-2),
        }
    }
}

/// A fork (arbitrage opportunity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fork {
    pub id: Uuid,
    pub event_id: String,
    pub home_team: String,
    pub away_team: String,
    pub league: String,
    pub sport: String,
    pub is_live: bool,
    pub start_time: Option<DateTime<Utc>>,
    pub profit_percent: Decimal,
    pub legs: Vec<ForkLeg>,
    pub fork_type: ForkType,
    pub created_at: DateTime<Utc>,
    pub age_ms: u64,
}

/// Single leg of a fork
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkLeg {
    pub bookmaker_slug: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub event_id: String,
    pub original_event_id: String,
}

/// Type of fork
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ForkType {
    MatchWinner1X2,  // P1-X-P2
    MatchWinner12,   // P1-P2 (no draw)
    TotalOverUnder,  // Over X - Under X
    Handicap,        // H1(X) - H2(X) or H1(X) - X - H2(Y)
    Btts,           // Both teams to score
    Corridor,       // Middle opportunity
    Custom(String),
}

/// Staking plan for a fork
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakePlan {
    pub total_stake: f64,
    pub stakes: Vec<LegStake>,
    pub guaranteed_profit: f64,
    pub roi_percent: f64,
}

/// Stake for a single leg
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegStake {
    pub leg_index: usize,
    pub bookmaker_slug: String,
    pub stake: f64,
    pub profit_if_wins: f64,
    pub roi_percent: f64,
}

/// Fork detection engine
pub struct ForkFinder {
    config: ForkConfig,
}

impl ForkFinder {
    pub fn new(config: ForkConfig) -> Self {
        Self { config }
    }

    /// Main entry point: find all forks in the given events and odds
    pub fn find_forks(&self, events: &[Event], all_odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        // Group odds by normalized event
        let odds_by_event = self.group_odds_by_event(all_odds);
        
        // Find forks for each event group
        for (event_key, event_odds) in odds_by_event {
            if let Some(event) = events.iter().find(|e| self.event_key(e) == event_key) {
                // Match winner forks (1X2 and 12)
                forks.extend(self.find_match_winner_forks(event, &event_odds));
                
                // Total forks
                forks.extend(self.find_total_forks(event, &event_odds));
                
                // Handicap forks
                forks.extend(self.find_handicap_forks(event, &event_odds));
                
                // BTTS forks
                forks.extend(self.find_btts_forks(event, &event_odds));
                
                // Corridors (if enabled)
                if self.config.enable_corridors {
                    forks.extend(self.find_corridors(event, &event_odds));
                }
            }
        }
        
        // Sort by profit (highest first)
        forks.sort_by(|a, b| b.profit_percent.cmp(&a.profit_percent));
        
        // Apply filters
        forks.retain(|f| self.apply_filters(f));
        
        forks
    }

    /// Find match winner forks (P1-X-P2 or P1-P2)
    fn find_match_winner_forks(&self, event: &Event, odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        // Group odds by bookmaker
        let odds_by_bk: HashMap<&str, Vec<&Odd>> = odds.iter()
            .fold(HashMap::new(), |mut acc, o| {
                acc.entry(&o.bookmaker_slug).or_default().push(o);
                acc
            });
        
        let bookmakers: Vec<&str> = odds_by_bk.keys().copied().collect();
        
        // Compare all pairs of bookmakers
        for i in 0..bookmakers.len() {
            for j in (i+1)..bookmakers.len() {
                let bk1 = bookmakers[i];
                let bk2 = bookmakers[j];
                let odds1 = odds_by_bk.get(bk1).cloned().unwrap_or_default();
                let odds2 = odds_by_bk.get(bk2).cloned().unwrap_or_default();
                
                // Try 3-way (P1-X-P2)
                if let (Some(p1), Some(x), Some(p2)) = (
                    self.find_odds(&odds1, &["1", "P1", "home", "Home"]),
                    self.find_odds(&odds2, &["X", "draw", "Draw", "ничья"]),
                    self.find_odds(&odds2, &["2", "P2", "away", "Away"]),
                ) {
                    if let Some(fork) = self.calculate_fork_3way(
                        event, p1, x, p2,
                        bk1.to_string(), bk2.to_string(), bk2.to_string(),
                        "P1".to_string(), "X".to_string(), "P2".to_string(),
                    ) {
                        forks.push(fork);
                    }
                }
                
                // Try 2-way (P1-P2) - no draw
                if let (Some(p1), Some(p2)) = (
                    self.find_odds(&odds1, &["1", "P1", "home", "Home"]),
                    self.find_odds(&odds2, &["2", "P2", "away", "Away"]),
                ) {
                    if let Some(fork) = self.calculate_fork_2way(
                        event, p1, p2,
                        bk1.to_string(), bk2.to_string(),
                        "P1".to_string(), "P2".to_string(),
                        ForkType::MatchWinner12,
                    ) {
                        forks.push(fork);
                    }
                }
            }
        }
        
        forks
    }

    /// Find total forks (Over X - Under X)
    fn find_total_forks(&self, event: &Event, odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        // Extract total odds
        let total_odds: Vec<(&str, &str, f64, f64)> = odds.iter()
            .filter_map(|o| {
                // Parse markets like "Over(2.5)", "Under(2.5)", "TO 2.5", "TU 2.5"
                let market_lower = o.market.to_lowercase();
                if let Some(val) = self.parse_total_value(&market_lower, &o.selection) {
                    Some((&o.bookmaker_slug[..], &o.market[..], val, o.odds))
                } else {
                    None
                }
            })
            .collect();
        
        // Group by total value
        let mut by_value: HashMap<String, Vec<(&str, &str, f64)>> = HashMap::new();
        for (bk, market, val, odds) in total_odds {
            let key = format!("{:.2}", val);
            by_value.entry(key).or_default().push((bk, market, odds));
        }
        
        // For each total value, find over/under pairs
        for (_value, value_odds) in by_value {
            let overs: Vec<_> = value_odds.iter()
                .filter(|(_, m, _)| m.to_lowercase().contains("over") || m.to_lowercase().starts_with("to "))
                .collect();
            let unders: Vec<_> = value_odds.iter()
                .filter(|(_, m, _)| m.to_lowercase().contains("under") || m.to_lowercase().starts_with("tu "))
                .collect();
            
            for (bk1, market1, over_odds) in &overs {
                for (bk2, market2, under_odds) in &unders {
                    if bk1 == bk2 { continue; }
                    
                    if let Some(fork) = self.calculate_fork_2way(
                        event, *over_odds, *under_odds,
                        bk1.to_string(), bk2.to_string(),
                        format!("Over {}", _value),
                        format!("Under {}", _value),
                        ForkType::TotalOverUnder,
                    ) {
                        forks.push(fork);
                    }
                }
            }
        }
        
        forks
    }

    /// Find handicap forks
    fn find_handicap_forks(&self, event: &Event, odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        // Look for handicap markets like "H1(-0.5)", "H2(+0.5)"
        let handicap_odds: Vec<(&str, &str, f64)> = odds.iter()
            .filter_map(|o| {
                let market_lower = o.market.to_lowercase();
                if market_lower.contains("handicap") || 
                   market_lower.starts_with("h1(") || 
                   market_lower.starts_with("h2(") {
                    Some((&o.bookmaker_slug[..], &o.selection[..], o.odds))
                } else {
                    None
                }
            })
            .collect();
        
        // Group by handicap value
        let mut by_value: HashMap<String, Vec<(&str, &str, f64)>> = HashMap::new();
        for (bk, selection, odds) in &handicap_odds {
            let key = selection.to_lowercase().replace(" ", "_");
            by_value.entry(key).or_default().push((*bk, *selection, *odds));
        }
        
        // Find opposite handicaps
        for (key, odds_list) in by_value {
            if key.contains("-0.5") || key.contains("+0.5") {
                // Look for complementary handicaps
                let h1_odds: Vec<_> = odds_list.iter()
                    .filter(|(_, s, _)| s.to_lowercase().contains("h1") || s.to_lowercase().contains("home"))
                    .collect();
                let h2_odds: Vec<_> = odds_list.iter()
                    .filter(|(_, s, _)| s.to_lowercase().contains("h2") || s.to_lowercase().contains("away"))
                    .collect();
                
                for (bk1, sel1, odds1) in &h1_odds {
                    for (bk2, sel2, odds2) in &h2_odds {
                        if bk1 == bk2 { continue; }
                        
                        if let Some(fork) = self.calculate_fork_2way(
                            event, *odds1, *odds2,
                            bk1.to_string(), bk2.to_string(),
                            sel1.to_string(), sel2.to_string(),
                            ForkType::Handicap,
                        ) {
                            forks.push(fork);
                        }
                    }
                }
            }
        }
        
        forks
    }

    /// Find BTTS (Both Teams To Score) forks
    fn find_btts_forks(&self, event: &Event, odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        let yes_odds: Vec<_> = odds.iter()
            .filter(|o| o.market.to_lowercase().contains("btts") || 
                       o.market.to_lowercase().contains("both teams to score"))
            .filter(|o| o.selection.to_lowercase() == "yes" || o.selection.to_lowercase() == "да")
            .collect();
        
        let no_odds: Vec<_> = odds.iter()
            .filter(|o| o.market.to_lowercase().contains("btts") || 
                       o.market.to_lowercase().contains("both teams to score"))
            .filter(|o| o.selection.to_lowercase() == "no" || o.selection.to_lowercase() == "нет")
            .collect();
        
        for o1 in &yes_odds {
            for o2 in &no_odds {
                if o1.bookmaker_slug == o2.bookmaker_slug { continue; }
                
                if let Some(fork) = self.calculate_fork_2way(
                    event, o1.odds, o2.odds,
                    o1.bookmaker_slug.clone(), o2.bookmaker_slug.clone(),
                    "BTTS Yes".to_string(), "BTTS No".to_string(),
                    ForkType::Btts,
                ) {
                    forks.push(fork);
                }
            }
        }
        
        forks
    }

    /// Find corridors (middle opportunities)
    fn find_corridors(&self, event: &Event, odds: &[Odd]) -> Vec<Fork> {
        let mut forks = Vec::new();
        
        // For totals: look for Over X and Under Y where X < Y
        let total_odds: Vec<(&str, &str, f64, f64)> = odds.iter()
            .filter_map(|o| {
                let market_lower = o.market.to_lowercase();
                if let Some(val) = self.parse_total_value(&market_lower, &o.selection) {
                    Some((&o.bookmaker_slug[..], &o.market[..], val, o.odds))
                } else {
                    None
                }
            })
            .collect();
        
        for (bk1, market1, val1, odds1) in &total_odds {
            for (bk2, market2, val2, odds2) in &total_odds {
                if bk1 == bk2 { continue; }
                
                // Check if this forms a corridor
                // Over X + Under Y where X < Y creates a middle
                if market1.to_lowercase().contains("over") && 
                   market2.to_lowercase().contains("under") &&
                   val1 < val2 {
                    // Calculate corridor width
                    let width = val2 - val1;
                    if width >= 0.5 { // Meaningful corridor
                        // Calculate if there's profit or minimal loss
                        let odds1_dec = Decimal::from_f64(*odds1).unwrap_or(Decimal::ONE);
                        let odds2_dec = Decimal::from_f64(*odds2).unwrap_or(Decimal::ONE);
                        let sum_inverses = Decimal::ONE / odds1_dec + Decimal::ONE / odds2_dec;
                        
                        // For corridors, we want sum close to 1 (small loss) with profit potential
                        if sum_inverses < Decimal::TWO { // At least some recovery
                            let fork = Fork {
                                id: Uuid::new_v4(),
                                event_id: event.id.clone(),
                                home_team: event.home_team.clone(),
                                away_team: event.away_team.clone(),
                                league: event.league.clone(),
                                sport: event.sport.to_string(),
                                is_live: event.is_live,
                                start_time: event.start_time.clone(),
                                profit_percent: Decimal::ZERO, // Corridors have different metrics
                                legs: vec![
                                    ForkLeg {
                                        bookmaker_slug: bk1.to_string(),
                                        market: market1.to_string(),
                                        selection: format!("Over {}", val1),
                                        odds: *odds1,
                                        event_id: event.id.clone(),
                                        original_event_id: event.id.clone(),
                                    },
                                    ForkLeg {
                                        bookmaker_slug: bk2.to_string(),
                                        market: market2.to_string(),
                                        selection: format!("Under {}", val2),
                                        odds: *odds2,
                                        event_id: event.id.clone(),
                                        original_event_id: event.id.clone(),
                                    },
                                ],
                                fork_type: ForkType::Corridor,
                                created_at: Utc::now(),
                                age_ms: 0,
                            };
                            forks.push(fork);
                        }
                    }
                }
            }
        }
        
        forks
    }

    /// Calculate a 2-way fork
    fn calculate_fork_2way(
        &self,
        event: &Event,
        odds1: f64,
        odds2: f64,
        bk1: String,
        bk2: String,
        selection1: String,
        selection2: String,
        fork_type: ForkType,
    ) -> Option<Fork> {
        // Check odds limits
        let min_odds_f: f64 = self.config.min_odds.to_f64().unwrap_or(1.01);
        let max_odds_f: f64 = self.config.max_odds.to_f64().unwrap_or(100.0);
        
        if odds1 < min_odds_f || odds1 > max_odds_f ||
           odds2 < min_odds_f || odds2 > max_odds_f {
            return None;
        }
        
        // Calculate sum of inverses: 1/odds1 + 1/odds2
        let sum_inverses = 1.0 / odds1 + 1.0 / odds2;
        
        // Check if this is a valid fork (sum < 1 for positive, or negative forks enabled)
        let profit_f64 = if sum_inverses < 1.0 {
            // Positive fork
            (1.0 - sum_inverses) / sum_inverses * 100.0
        } else if self.config.enable_negative_forks && 
                  sum_inverses < 1.0 - self.config.max_negative_profit.to_f64().unwrap_or(-2.0) / 100.0 {
            // Negative fork (for bonus hunting)
            (1.0 - sum_inverses) / sum_inverses * 100.0
        } else {
            return None;
        };
        
        let profit = Decimal::try_from(profit_f64).unwrap_or(Decimal::ZERO);
        
        // Check profit limits
        if profit < self.config.min_profit || profit > self.config.max_profit {
            return None;
        }
        
        Some(Fork {
            id: Uuid::new_v4(),
            event_id: event.id.clone(),
            home_team: event.home_team.clone(),
            away_team: event.away_team.clone(),
            league: event.league.clone(),
            sport: event.sport.to_string(),
            is_live: event.is_live,
            start_time: event.start_time,
            profit_percent: profit,
            legs: vec![
                ForkLeg {
                    bookmaker_slug: bk1,
                    market: fork_type.to_string(),
                    selection: selection1,
                    odds: odds1,
                    event_id: event.id.clone(),
                    original_event_id: event.id.clone(),
                },
                ForkLeg {
                    bookmaker_slug: bk2,
                    market: fork_type.to_string(),
                    selection: selection2,
                    odds: odds2,
                    event_id: event.id.clone(),
                    original_event_id: event.id.clone(),
                },
            ],
            fork_type,
            created_at: chrono::Utc::now(),
            age_ms: 0,
        })
    }

    /// Calculate a 3-way fork
    fn calculate_fork_3way(
        &self,
        event: &Event,
        odds1: f64,
        odds_x: f64,
        odds2: f64,
        bk1: String,
        bk_x: String,
        bk2: String,
        selection1: String,
        selection_x: String,
        selection2: String,
    ) -> Option<Fork> {
        // Check odds limits
        let min_odds_f: f64 = self.config.min_odds.to_f64().unwrap_or(1.01);
        let max_odds_f: f64 = self.config.max_odds.to_f64().unwrap_or(100.0);
        
        if odds1 < min_odds_f || odds1 > max_odds_f ||
           odds_x < min_odds_f || odds_x > max_odds_f ||
           odds2 < min_odds_f || odds2 > max_odds_f {
            return None;
        }
        
        // Calculate sum of inverses
        let sum_inverses = 1.0 / odds1 + 1.0 / odds_x + 1.0 / odds2;
        
        // Check if valid fork
        let profit_f64 = if sum_inverses < 1.0 {
            (1.0 - sum_inverses) / sum_inverses * 100.0
        } else if self.config.enable_negative_forks && 
                  sum_inverses < 1.0 - self.config.max_negative_profit.to_f64().unwrap_or(-2.0) / 100.0 {
            (1.0 - sum_inverses) / sum_inverses * 100.0
        } else {
            return None;
        };
        
        let profit = Decimal::try_from(profit_f64).unwrap_or(Decimal::ZERO);
        
        // Check profit limits
        if profit < self.config.min_profit || profit > self.config.max_profit {
            return None;
        }
        
        Some(Fork {
            id: Uuid::new_v4(),
            event_id: event.id.clone(),
            home_team: event.home_team.clone(),
            away_team: event.away_team.clone(),
            league: event.league.clone(),
            sport: event.sport.to_string(),
            is_live: event.is_live,
            start_time: event.start_time,
            profit_percent: profit,
            legs: vec![
                ForkLeg {
                    bookmaker_slug: bk1,
                    market: "1X2".to_string(),
                    selection: selection1,
                    odds: odds1,
                    event_id: event.id.clone(),
                    original_event_id: event.id.clone(),
                },
                ForkLeg {
                    bookmaker_slug: bk_x,
                    market: "1X2".to_string(),
                    selection: selection_x,
                    odds: odds_x,
                    event_id: event.id.clone(),
                    original_event_id: event.id.clone(),
                },
                ForkLeg {
                    bookmaker_slug: bk2,
                    market: "1X2".to_string(),
                    selection: selection2,
                    odds: odds2,
                    event_id: event.id.clone(),
                    original_event_id: event.id.clone(),
                },
            ],
            fork_type: ForkType::MatchWinner1X2,
            created_at: chrono::Utc::now(),
            age_ms: 0,
        })
    }

    /// Calculate stakes for a fork using equal profit strategy
    pub fn calculate_equal_profit_stakes(&self, fork: &Fork, total_stake: f64) -> StakePlan {
        let odds_f64: Vec<f64> = fork.legs.iter().map(|l| l.odds).collect();
        let stakes_f64 = calculate_stakes_equal_profit_f64(&odds_f64, total_stake);
        
        let guaranteed_profit = stakes_f64[0] * fork.legs[0].odds - total_stake;
        let roi = guaranteed_profit / total_stake * 100.0;
        
        StakePlan {
            total_stake,
            stakes: stakes_f64.into_iter().enumerate().map(|(i, s_f64)| {
                let profit_if_wins = s_f64 * fork.legs[i].odds - total_stake;
                let roi_percent = profit_if_wins / s_f64 * 100.0;
                LegStake {
                    leg_index: i,
                    bookmaker_slug: fork.legs[i].bookmaker_slug.clone(),
                    stake: s_f64,
                    profit_if_wins,
                    roi_percent,
                }
            }).collect(),
            guaranteed_profit,
            roi_percent: roi,
        }
    }

    /// Apply filters to a fork
    fn apply_filters(&self, fork: &Fork) -> bool {
        // Profit range
        if fork.profit_percent < self.config.min_profit || 
           fork.profit_percent > self.config.max_profit {
            return false;
        }
        
        // Odds range
        let min_odds_f: f64 = self.config.min_odds.to_f64().unwrap_or(1.01);
        let max_odds_f: f64 = self.config.max_odds.to_f64().unwrap_or(100.0);
        for leg in &fork.legs {
            if leg.odds < min_odds_f || leg.odds > max_odds_f {
                return false;
            }
        }
        
        true
    }

    /// Group odds by normalized event
    fn group_odds_by_event(&self, odds: &[Odd]) -> HashMap<String, Vec<Odd>> {
        let mut result: HashMap<String, Vec<Odd>> = HashMap::new();
        for odd in odds {
            // Use event_id as the key (should already be normalized)
            result.entry(odd.event_id.clone()).or_insert_with(Vec::new).push(odd.clone());
        }
        result
    }

    /// Get event key for grouping
    fn event_key(&self, event: &Event) -> String {
        // Create a normalized key based on teams and start time
        format!("{}_{}_{}", 
            event.sport.to_string().to_lowercase(),
            event.home_team.to_lowercase().replace(" ", "_"),
            event.away_team.to_lowercase().replace(" ", "_")
        )
    }

    /// Find odds matching any of the given selection names
    fn find_odds(&self, odds: &[&Odd], selections: &[&str]) -> Option<f64> {
        for odd in odds {
            let sel_lower = odd.selection.to_lowercase();
            for sel in selections {
                if sel_lower == sel.to_lowercase() {
                    return Some(odd.odds);
                }
            }
        }
        None
    }

    /// Parse total value from market string
    fn parse_total_value(&self, market: &str, selection: &str) -> Option<f64> {
        // Try patterns like "Over(2.5)", "Under 2.5", "TO 2.5", "TU 2.5"
        let combined = format!("{} {}", market, selection);
        
        // Extract number using regex-like logic
        let cleaned: String = combined.chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        
        cleaned.parse().ok()
    }
}

impl ForkType {
    fn to_string(&self) -> String {
        match self {
            ForkType::MatchWinner1X2 => "1X2".to_string(),
            ForkType::MatchWinner12 => "12".to_string(),
            ForkType::TotalOverUnder => "Total".to_string(),
            ForkType::Handicap => "Handicap".to_string(),
            ForkType::Btts => "BTTS".to_string(),
            ForkType::Corridor => "Corridor".to_string(),
            ForkType::Custom(s) => s.clone(),
        }
    }
}

/// Calculate stakes for equal profit distribution (f64 version)
fn calculate_stakes_equal_profit_f64(odds: &[f64], total_stake: f64) -> Vec<f64> {
    let sum_inverses: f64 = odds.iter()
        .map(|o| 1.0 / o)
        .sum();
    
    odds.iter()
        .map(|o| {
            let inverse = 1.0 / o;
            total_stake * inverse / sum_inverses
        })
        .collect()
}

/// Additional staking strategies
pub mod staking {
    use super::*;
    
    /// Proportional stakes (bet more on higher odds)
    pub fn proportional(odds: &[f64], total_stake: f64) -> Vec<f64> {
        let sum_odds: f64 = odds.iter().sum();
        odds.iter()
            .map(|o| total_stake * o / sum_odds)
            .collect()
    }
    
    /// Fixed amount per leg
    pub fn fixed_amount(count: usize, amount: f64) -> Vec<f64> {
        vec![amount; count]
    }
    
    /// Kelly Criterion stakes
    pub fn kelly(odds: &[f64], probabilities: &[f64], bankroll: f64, fraction: f64) -> Vec<f64> {
        odds.iter()
            .zip(probabilities.iter())
            .map(|(o, p)| {
                // Kelly formula: (bp - q) / b
                // where b = odds - 1, p = probability, q = 1 - p
                let b = o - 1.0;
                let q = 1.0 - p;
                let kelly = (b * p - q) / b;
                
                // Apply fraction and cap at reasonable percentage
                let kelly_capped = kelly.min(0.25);
                bankroll * kelly_capped * fraction
            })
            .collect()
    }
    
    /// Flat percentage of bankroll per leg
    pub fn flat_percentage(count: usize, bankroll: f64, percent: f64) -> Vec<f64> {
        let stake_per_leg = bankroll * percent;
        vec![stake_per_leg; count]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_fork_2way() {
        let config = ForkConfig::default();
        let finder = ForkFinder::new(config);
        
        // 2.1 * 1.9 = 3.99, sum inverses = 0.476 + 0.526 = 1.002 (not a fork)
        // 2.2 * 1.9 = 4.18, sum inverses = 0.455 + 0.526 = 0.981 (fork!)
        let odds1 = 2.20_f64;
        let odds2 = 1.90_f64;
        
        let inverse1 = 1.0 / odds1;
        let inverse2 = 1.0 / odds2;
        let sum = inverse1 + inverse2;
        
        assert!(sum < 1.0, "Should be a valid fork");
        
        let profit = (1.0 - sum) / sum * 100.0;
        assert!(profit > 0.0, "Profit should be positive");
    }
    
    #[test]
    fn test_calculate_stakes() {
        let odds = vec![2.20_f64, 1.90_f64];
        let total = 10000.0_f64;
        
        let stakes = calculate_stakes_equal_profit_f64(&odds, total);
        
        assert_eq!(stakes.len(), 2);
        assert!(stakes[0] > 0.0);
        assert!(stakes[1] > 0.0);
        
        // Higher odds = lower stake
        assert!(stakes[0] < stakes[1]);
    }
}
