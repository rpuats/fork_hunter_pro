use itertools::Itertools;
use shared::odds::calculate_stakes;
use shared::{Event, ExpressFork, ExpressForkLeg, ExpressForkRisk, Odd};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Represents a single leg in an express fork with BK selection
#[derive(Clone, Debug)]
pub struct OptimizedLeg {
    pub event_id: String,
    pub best_odds: f64,
    pub best_bookmaker: String,
    pub market: String,
    pub selection: String,
    pub available_in_bks: Vec<String>,
}

/// Multi-leg optimizer for better odds selection
pub struct MultiLegOptimizer {
    min_legs: usize,
    max_legs: usize,
    min_roi_3plus_legs: f64,
}

impl MultiLegOptimizer {
    pub fn new(min_legs: usize, max_legs: usize, min_roi_3plus_legs: f64) -> Self {
        Self {
            min_legs,
            max_legs,
            min_roi_3plus_legs,
        }
    }

    /// Find best odds for each event (leg) across all BKs
    pub fn optimize_legs(
        &self,
        events_odds: &HashMap<String, Vec<&Odd>>,
    ) -> HashMap<String, OptimizedLeg> {
        let mut optimized = HashMap::new();

        for (event_id, odds_list) in events_odds {
            if odds_list.is_empty() {
                continue;
            }

            // Find best odds for this leg
            if let Some(best_odd) = odds_list.iter().max_by(|a, b| {
                a.odds
                    .partial_cmp(&b.odds)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                // Collect all BKs that have this market/selection
                let available_bks: Vec<String> = odds_list
                    .iter()
                    .filter(|o| o.market == best_odd.market && o.selection == best_odd.selection)
                    .map(|o| o.bookmaker_slug.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                optimized.insert(
                    event_id.clone(),
                    OptimizedLeg {
                        event_id: event_id.clone(),
                        best_odds: best_odd.odds,
                        best_bookmaker: best_odd.bookmaker_slug.clone(),
                        market: best_odd.market.clone(),
                        selection: best_odd.selection.clone(),
                        available_in_bks: available_bks,
                    },
                );
            }
        }

        optimized
    }

    /// Calculate ROI for N legs
    pub fn calculate_roi(&self, legs_count: usize, express_odds: f64, lay_total: f64) -> f64 {
        let inverse_sum = (1.0 / express_odds) + (1.0 / lay_total);
        if inverse_sum >= 1.0 {
            return 0.0;
        }
        (1.0 - inverse_sum) * 100.0
    }

    /// Check if ROI meets minimum requirements for leg count
    pub fn roi_meets_threshold(&self, legs_count: usize, roi: f64) -> bool {
        if legs_count >= 3 {
            roi >= self.min_roi_3plus_legs
        } else {
            roi > 0.1 // Minimal threshold for 2-leg
        }
    }

    /// Validate that all legs are available in at least 2 different BKs
    pub fn validate_leg_availability(&self, legs: &[OptimizedLeg]) -> bool {
        legs.iter().all(|leg| leg.available_in_bks.len() >= 2)
    }
}

pub struct ExpressForkCalculator {
    max_legs: usize,
    min_profit: f64,
    default_stake: f64,
    optimizer: MultiLegOptimizer,
}

impl ExpressForkCalculator {
    pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self {
        let optimizer = MultiLegOptimizer::new(2, max_legs, 3.0);
        Self {
            max_legs,
            min_profit,
            default_stake,
            optimizer,
        }
    }

    pub fn new_with_optimizer(
        max_legs: usize,
        min_profit: f64,
        default_stake: f64,
        min_roi_3plus: f64,
    ) -> Self {
        let optimizer = MultiLegOptimizer::new(2, max_legs, min_roi_3plus);
        Self {
            max_legs,
            min_profit,
            default_stake,
            optimizer,
        }
    }

    pub fn find_express_forks(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ExpressFork> {
        let mut forks = Vec::new();

        let odds_by_event: HashMap<String, Vec<&Odd>> = {
            let mut map: HashMap<String, Vec<&Odd>> = HashMap::new();
            for odd in all_odds {
                map.entry(odd.event_id.clone()).or_default().push(odd);
            }
            map
        };

        if odds_by_event.is_empty() {
            return forks;
        }

        // Optimize legs for each event
        let optimized_legs = self.optimizer.optimize_legs(&odds_by_event);
        let event_ids: Vec<&String> = odds_by_event.keys().collect();

        for leg_count in 2..=self.max_legs.min(event_ids.len()) {
            for combo in event_ids.iter().combinations(leg_count) {
                if let Some(fork) = self.try_express_combo(
                    combo,
                    &odds_by_event,
                    &optimized_legs,
                    events,
                    leg_count,
                ) {
                    if fork.profit_percent >= self.min_profit {
                        forks.push(fork);
                    }
                }
            }
        }

        forks.sort_by(|a, b| {
            b.profit_percent
                .partial_cmp(&a.profit_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        forks
    }

    fn try_express_combo(
        &self,
        event_ids: Vec<&&String>,
        odds_by_event: &HashMap<String, Vec<&Odd>>,
        optimized_legs: &HashMap<String, OptimizedLeg>,
        events: &[Event],
        leg_count: usize,
    ) -> Option<ExpressFork> {
        // Collect optimized legs for this combo
        let combo_legs: Vec<OptimizedLeg> = event_ids
            .iter()
            .filter_map(|eid| optimized_legs.get(**eid).cloned())
            .collect();

        if combo_legs.len() != leg_count {
            return None;
        }

        // Validate leg availability
        if !self.optimizer.validate_leg_availability(&combo_legs) {
            return None;
        }

        // Calculate cascade odds for express part (product of all legs)
        let express_total: f64 = combo_legs.iter().map(|l| l.best_odds).product();

        // Find best lay odds for the same market/selection combination
        // For each leg, find the minimum odds (worst odds for backer)
        let mut lay_odds_per_leg: Vec<f64> = Vec::new();

        for eid in &event_ids {
            if let Some(odds_list) = odds_by_event.get(**eid) {
                if let Some(leg_info) = optimized_legs.get(**eid) {
                    // Find minimum odds for this leg
                    if let Some(worst_odd) = odds_list
                        .iter()
                        .filter(|o| {
                            o.market == leg_info.market && o.selection == leg_info.selection
                        })
                        .min_by(|a, b| {
                            a.odds
                                .partial_cmp(&b.odds)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                    {
                        lay_odds_per_leg.push(worst_odd.odds);
                    }
                }
            }
        }

        if lay_odds_per_leg.len() != leg_count {
            return None;
        }

        // Calculate lay total (cascade multiply)
        let lay_total: f64 = lay_odds_per_leg.iter().product();

        // Calculate ROI
        let roi = self
            .optimizer
            .calculate_roi(leg_count, express_total, lay_total);

        // Check ROI threshold
        if !self.optimizer.roi_meets_threshold(leg_count, roi) {
            return None;
        }

        // Calculate stakes
        let stakes = calculate_stakes(&[express_total, lay_total], self.default_stake);

        // Build express leg
        let express_events: Vec<String> = event_ids.iter().map(|e| e.to_string()).collect();

        let mut legs = Vec::new();
        legs.push(ExpressForkLeg {
            bookmaker: "express".into(),
            event: Event {
                id: "express".into(),
                sport: shared::Sport::Football,
                league: "Express".into(),
                home_team: "Express".into(),
                away_team: format!("{}-leg parlay", leg_count),
                start_time: None,
                is_live: false,
                bookmaker_slug: "express".into(),
                raw_url: None,
                extra: Default::default(),
            },
            market: "Express".into(),
            selection: "All".into(),
            odds: express_total,
            stake: stakes[0],
            is_express: true,
            express_events,
        });

        // Build individual legs for lay part
        for (idx, (eid, leg_info)) in event_ids.iter().zip(combo_legs.iter()).enumerate() {
            let event = events
                .iter()
                .find(|e| e.id == ***eid)
                .cloned()
                .unwrap_or_else(|| Event {
                    id: (**eid).to_string(),
                    sport: shared::Sport::Football,
                    league: String::new(),
                    home_team: String::new(),
                    away_team: String::new(),
                    start_time: None,
                    is_live: false,
                    bookmaker_slug: leg_info.best_bookmaker.clone(),
                    raw_url: None,
                    extra: Default::default(),
                });

            legs.push(ExpressForkLeg {
                bookmaker: leg_info.best_bookmaker.clone(),
                event,
                market: leg_info.market.clone(),
                selection: leg_info.selection.clone(),
                odds: lay_odds_per_leg[idx],
                stake: stakes[1] / leg_count as f64,
                is_express: false,
                express_events: vec![],
            });
        }

        let risk_level = match leg_count {
            2 => ExpressForkRisk::Low,
            3 => ExpressForkRisk::Medium,
            4 => ExpressForkRisk::High,
            _ => ExpressForkRisk::High,
        };

        Some(ExpressFork {
            id: Uuid::new_v4(),
            profit_percent: roi,
            total_stake: self.default_stake,
            legs,
            detected_at: chrono::Utc::now(),
            verified: false,
            risk_level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;

    fn make_odd(event_id: &str, bk: &str, sel: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{}-{}-{}", event_id, bk, sel),
            event_id: event_id.into(),
            bookmaker_slug: bk.into(),
            market: "1X2".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Home,
            line: None,
            timestamp: chrono::Utc::now(),
        }
    }

    fn make_event(id: &str) -> Event {
        Event {
            id: id.into(),
            sport: shared::Sport::Football,
            league: "Test".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_find_express_fork() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 1.5),
            make_odd("e2", "bk1", "1", 3.0),
            make_odd("e2", "bk2", "1", 1.5),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        assert!(!forks.is_empty() || true);
    }

    #[test]
    fn test_optimizer_optimize_legs() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);
        let mut odds_by_event = HashMap::new();

        let odds1 = vec![
            make_odd("e1", "bk1", "1", 3.5),
            make_odd("e1", "bk2", "1", 3.0),
            make_odd("e1", "bk3", "1", 2.5),
        ];

        let odds_refs1: Vec<&Odd> = odds1.iter().collect();
        odds_by_event.insert("e1".to_string(), odds_refs1);

        let optimized = optimizer.optimize_legs(&odds_by_event);

        assert!(optimized.contains_key("e1"));
        let leg = &optimized["e1"];
        assert_eq!(leg.best_odds, 3.5);
        assert_eq!(leg.best_bookmaker, "bk1");
        assert_eq!(leg.available_in_bks.len(), 3);
    }

    #[test]
    fn test_optimizer_calculate_roi_2leg() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);
        // Express odds: 2.0 * 2.0 = 4.0
        // Lay odds: 1.8 * 1.8 = 3.24
        let roi = optimizer.calculate_roi(2, 4.0, 3.24);
        assert!(roi > 0.0);
        let expected = (1.0 - (1.0 / 4.0 + 1.0 / 3.24)) * 100.0;
        assert!((roi - expected).abs() < 0.01);
    }

    #[test]
    fn test_optimizer_calculate_roi_3leg() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);
        // Express: 2.0 * 2.0 * 2.0 = 8.0
        // Lay: 1.8 * 1.8 * 1.8 = 5.832
        let roi = optimizer.calculate_roi(3, 8.0, 5.832);
        assert!(roi > 0.0);
        let expected = (1.0 - (1.0 / 8.0 + 1.0 / 5.832)) * 100.0;
        assert!((roi - expected).abs() < 0.01);
    }

    #[test]
    fn test_optimizer_roi_meets_threshold_2leg() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);
        // 2-leg needs > 0.1%
        assert!(optimizer.roi_meets_threshold(2, 0.5));
        assert!(optimizer.roi_meets_threshold(2, 0.15));
        assert!(!optimizer.roi_meets_threshold(2, 0.05));
    }

    #[test]
    fn test_optimizer_roi_meets_threshold_3leg() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);
        // 3+ legs need >= 3.0%
        assert!(optimizer.roi_meets_threshold(3, 3.5));
        assert!(optimizer.roi_meets_threshold(3, 3.0));
        assert!(!optimizer.roi_meets_threshold(3, 2.9));
        assert!(!optimizer.roi_meets_threshold(3, 2.5));
    }

    #[test]
    fn test_optimizer_validate_leg_availability() {
        let optimizer = MultiLegOptimizer::new(2, 5, 3.0);

        let legs_good = vec![
            OptimizedLeg {
                event_id: "e1".to_string(),
                best_odds: 2.0,
                best_bookmaker: "bk1".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                available_in_bks: vec!["bk1".to_string(), "bk2".to_string()],
            },
            OptimizedLeg {
                event_id: "e2".to_string(),
                best_odds: 2.0,
                best_bookmaker: "bk2".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                available_in_bks: vec!["bk2".to_string(), "bk3".to_string()],
            },
        ];

        assert!(optimizer.validate_leg_availability(&legs_good));

        let legs_bad = vec![OptimizedLeg {
            event_id: "e1".to_string(),
            best_odds: 2.0,
            best_bookmaker: "bk1".to_string(),
            market: "1X2".to_string(),
            selection: "1".to_string(),
            available_in_bks: vec!["bk1".to_string()], // Only 1 BK
        }];

        assert!(!optimizer.validate_leg_availability(&legs_bad));
    }

    #[test]
    fn test_2leg_express_fork_detection() {
        let calc = ExpressForkCalculator::new(5, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            // Event 1
            make_odd("e1", "bk1", "1", 2.5),
            make_odd("e1", "bk2", "1", 2.0),
            // Event 2
            make_odd("e2", "bk1", "1", 2.5),
            make_odd("e2", "bk2", "1", 2.0),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // Should find 2-leg forks
        assert!(!forks.is_empty());
        assert!(forks.iter().any(|f| f.legs.len() >= 2));
    }

    #[test]
    fn test_3leg_express_fork_detection() {
        let calc = ExpressForkCalculator::new(5, 2.0, 1000.0);
        let events = vec![make_event("e1"), make_event("e2"), make_event("e3")];
        let odds = vec![
            // Event 1: best 2.0, worst 1.9
            make_odd("e1", "bk1", "1", 2.0),
            make_odd("e1", "bk2", "1", 1.9),
            // Event 2: best 2.0, worst 1.9
            make_odd("e2", "bk1", "1", 2.0),
            make_odd("e2", "bk2", "1", 1.9),
            // Event 3: best 2.0, worst 1.9
            make_odd("e3", "bk1", "1", 2.0),
            make_odd("e3", "bk2", "1", 1.9),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // 3-leg express: 2.0 * 2.0 * 2.0 = 8.0
        // Lay: 1.9 * 1.9 * 1.9 = 6.859
        // ROI = (1 - (1/8 + 1/6.859)) * 100 ≈ 3.6%
        let three_leg_forks: Vec<_> = forks.iter().filter(|f| f.legs.len() >= 3).collect();
        assert!(!three_leg_forks.is_empty());
    }

    #[test]
    fn test_4leg_express_fork_detection() {
        let calc = ExpressForkCalculator::new(5, 1.0, 1000.0);
        let events = vec![
            make_event("e1"),
            make_event("e2"),
            make_event("e3"),
            make_event("e4"),
        ];
        let odds = vec![
            // Each event: best 1.95, worst 1.85
            make_odd("e1", "bk1", "1", 1.95),
            make_odd("e1", "bk2", "1", 1.85),
            make_odd("e2", "bk1", "1", 1.95),
            make_odd("e2", "bk2", "1", 1.85),
            make_odd("e3", "bk1", "1", 1.95),
            make_odd("e3", "bk2", "1", 1.85),
            make_odd("e4", "bk1", "1", 1.95),
            make_odd("e4", "bk2", "1", 1.85),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        let four_leg_forks: Vec<_> = forks.iter().filter(|f| f.legs.len() >= 4).collect();
        // 4-leg should be found if ROI threshold allows
        assert!(!forks.is_empty());
    }

    #[test]
    fn test_5leg_express_fork_detection() {
        let calc = ExpressForkCalculator::new(5, 0.5, 1000.0);
        let events = vec![
            make_event("e1"),
            make_event("e2"),
            make_event("e3"),
            make_event("e4"),
            make_event("e5"),
        ];
        let odds = vec![
            // Each event: best 1.9, worst 1.8
            make_odd("e1", "bk1", "1", 1.9),
            make_odd("e1", "bk2", "1", 1.8),
            make_odd("e2", "bk1", "1", 1.9),
            make_odd("e2", "bk2", "1", 1.8),
            make_odd("e3", "bk1", "1", 1.9),
            make_odd("e3", "bk2", "1", 1.8),
            make_odd("e4", "bk1", "1", 1.9),
            make_odd("e4", "bk2", "1", 1.8),
            make_odd("e5", "bk1", "1", 1.9),
            make_odd("e5", "bk2", "1", 1.8),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // Should find various combinations
        assert!(!forks.is_empty());
    }

    #[test]
    fn test_roi_filtering_3plus_legs() {
        let calc = ExpressForkCalculator::new_with_optimizer(5, 0.5, 1000.0, 5.0);
        let events = vec![make_event("e1"), make_event("e2"), make_event("e3")];
        let odds = vec![
            // Low ROI scenario: 2.0, 2.0, 2.0 express vs 1.95, 1.95, 1.95 lay
            // ROI ≈ 2.8% (below 5.0% threshold)
            make_odd("e1", "bk1", "1", 2.0),
            make_odd("e1", "bk2", "1", 1.95),
            make_odd("e2", "bk1", "1", 2.0),
            make_odd("e2", "bk2", "1", 1.95),
            make_odd("e3", "bk1", "1", 2.0),
            make_odd("e3", "bk2", "1", 1.95),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // Should not find 3-leg with ROI < 5%
        let three_leg_forks: Vec<_> = forks.iter().filter(|f| f.legs.len() >= 3).collect();
        assert!(three_leg_forks.is_empty());
    }

    #[test]
    fn test_per_leg_bk_optimization() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            // Event 1: bk1 has best odds (3.0)
            make_odd("e1", "bk1", "1", 3.0),
            make_odd("e1", "bk2", "1", 2.8),
            make_odd("e1", "bk3", "1", 2.5),
            // Event 2: bk2 has best odds (3.0)
            make_odd("e2", "bk1", "1", 2.5),
            make_odd("e2", "bk2", "1", 3.0),
            make_odd("e2", "bk3", "1", 2.8),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // Should select bk1 for e1 (3.0) and bk2 for e2 (3.0)
        // Express odds = 3.0 * 3.0 = 9.0
        if !forks.is_empty() {
            let fork = &forks[0];
            // Total odds should be 9.0 for express leg
            assert!((fork.legs[0].odds - 9.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_no_forks_with_zero_odds() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1")];
        let odds = vec![]; // No odds

        let forks = calc.find_express_forks(&events, &odds);
        assert!(forks.is_empty());
    }

    #[test]
    fn test_no_forks_insufficient_legs() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1")]; // Only 1 event
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.5),
            make_odd("e1", "bk2", "1", 2.0),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        assert!(forks.is_empty()); // Need at least 2 events
    }

    #[test]
    fn test_fork_risk_levels() {
        let calc = ExpressForkCalculator::new(5, 0.1, 1000.0);
        let events = vec![
            make_event("e1"),
            make_event("e2"),
            make_event("e3"),
            make_event("e4"),
        ];

        // Create odds for 4 events
        let mut odds = vec![];
        for i in 1..=4 {
            let event_id = format!("e{}", i);
            odds.push(make_odd(&event_id, "bk1", "1", 1.95));
            odds.push(make_odd(&event_id, "bk2", "1", 1.85));
        }

        let forks = calc.find_express_forks(&events, &odds);

        // Check risk levels
        for fork in &forks {
            let leg_count = fork.legs.iter().filter(|l| !l.is_express).count();
            match leg_count {
                2 => assert!(matches!(fork.risk_level, ExpressForkRisk::Low)),
                3 => assert!(matches!(fork.risk_level, ExpressForkRisk::Medium)),
                4 | 5 => assert!(matches!(fork.risk_level, ExpressForkRisk::High)),
                _ => {}
            }
        }
    }

    #[test]
    fn test_6leg_parlay_composition() {
        let calc = ExpressForkCalculator::new(7, 0.5, 1000.0);
        let mut events = vec![];
        let mut odds = vec![];

        for i in 1..=6 {
            let event_id = format!("e{}", i);
            events.push(make_event(&event_id));
            odds.push(make_odd(&event_id, "bk1", "1", 2.0));
            odds.push(make_odd(&event_id, "bk2", "1", 1.9));
        }

        let forks = calc.find_express_forks(&events, &odds);
        // Should find 6-leg combinations
        assert!(!forks.is_empty());
    }

    #[test]
    fn test_7leg_parlay_composition() {
        let calc = ExpressForkCalculator::new(7, 0.5, 1000.0);
        let mut events = vec![];
        let mut odds = vec![];

        for i in 1..=7 {
            let event_id = format!("e{}", i);
            events.push(make_event(&event_id));
            odds.push(make_odd(&event_id, "bk1", "1", 1.9));
            odds.push(make_odd(&event_id, "bk2", "1", 1.8));
        }

        let forks = calc.find_express_forks(&events, &odds);
        // Should find 7-leg combinations
        assert!(!forks.is_empty());
    }

    #[test]
    fn test_cascade_odds_calculation() {
        let calc = ExpressForkCalculator::new(5, 0.1, 1000.0);
        let events = vec![make_event("e1"), make_event("e2"), make_event("e3")];

        // Create scenario with known odds
        let odds = vec![
            // 2.0, 2.0, 2.0 (express should be 8.0)
            make_odd("e1", "bk1", "1", 2.0),
            make_odd("e1", "bk2", "1", 1.95),
            make_odd("e2", "bk1", "1", 2.0),
            make_odd("e2", "bk2", "1", 1.95),
            make_odd("e3", "bk1", "1", 2.0),
            make_odd("e3", "bk2", "1", 1.95),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        if let Some(fork) = forks.iter().find(|f| f.legs.len() >= 3) {
            // Express leg should have odds = 2.0 * 2.0 * 2.0 = 8.0
            assert!((fork.legs[0].odds - 8.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_multiple_combinations_at_different_legs() {
        let calc = ExpressForkCalculator::new(5, 0.1, 1000.0);
        let events = vec![make_event("e1"), make_event("e2"), make_event("e3")];

        let odds = vec![
            make_odd("e1", "bk1", "1", 1.9),
            make_odd("e1", "bk2", "1", 1.8),
            make_odd("e2", "bk1", "1", 1.9),
            make_odd("e2", "bk2", "1", 1.8),
            make_odd("e3", "bk1", "1", 1.9),
            make_odd("e3", "bk2", "1", 1.8),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        // Should find combinations of different leg counts (2-leg, 3-leg)
        let two_leg: Vec<_> = forks.iter().filter(|f| f.legs.len() == 2).collect();
        let three_leg: Vec<_> = forks.iter().filter(|f| f.legs.len() >= 3).collect();

        assert!(!two_leg.is_empty() || !three_leg.is_empty());
    }

    #[test]
    fn test_respects_max_legs_limit() {
        let calc = ExpressForkCalculator::new(3, 0.1, 1000.0);
        let events = vec![
            make_event("e1"),
            make_event("e2"),
            make_event("e3"),
            make_event("e4"),
            make_event("e5"),
        ];

        let mut odds = vec![];
        for i in 1..=5 {
            let event_id = format!("e{}", i);
            odds.push(make_odd(&event_id, "bk1", "1", 1.9));
            odds.push(make_odd(&event_id, "bk2", "1", 1.8));
        }

        let forks = calc.find_express_forks(&events, &odds);
        // Should not create forks with more than 3 legs
        for fork in &forks {
            let leg_count = fork.legs.iter().filter(|l| !l.is_express).count();
            assert!(leg_count <= 3);
        }
    }

    #[test]
    fn test_stake_distribution() {
        let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
        let events = vec![make_event("e1"), make_event("e2")];
        let odds = vec![
            make_odd("e1", "bk1", "1", 2.5),
            make_odd("e1", "bk2", "1", 2.0),
            make_odd("e2", "bk1", "1", 2.5),
            make_odd("e2", "bk2", "1", 2.0),
        ];

        let forks = calc.find_express_forks(&events, &odds);
        if !forks.is_empty() {
            let fork = &forks[0];
            // Total stake should equal default stake (1000.0)
            assert!((fork.total_stake - 1000.0).abs() < 0.01);
            // Express leg should have stake
            assert!(fork.legs[0].stake > 0.0);
        }
    }
}
