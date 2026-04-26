/// Integration tests for enhanced express fork functionality
/// Tests combining hedge, reorder, breakeven, and cascade modules

#[cfg(test)]
mod integration_tests {
    use express_forks::{
        BreakEvenCalculator, CascadeSelector, CascadeStrategy, HedgeCalculator, HedgeStrategy,
        LegReorderer, ReorderStrategy,
    };

    #[test]
    fn test_full_pipeline_6leg_parlay() {
        // Step 1: Select cascade legs
        let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);

        let legs_data = vec![
            ("e1", 2.5, "EPL", 0.85, 4),
            ("e2", 2.3, "LaLiga", 0.90, 5),
            ("e3", 2.1, "Serie A", 0.75, 3),
            ("e4", 2.4, "Bundesliga", 0.88, 4),
            ("e5", 2.2, "Ligue 1", 0.82, 3),
            ("e6", 2.6, "Championship", 0.80, 2),
        ];

        let cascade_legs: Vec<_> = legs_data
            .iter()
            .map(|(id, odds, league, avail, bks)| {
                let mut backup = vec![];
                for i in 1..*bks {
                    backup.push(format!("bk{}", i + 1));
                }
                express_forks::cascade::CascadeLeg {
                    position: 0,
                    event_id: id.to_string(),
                    odds: *odds,
                    selection: "1".to_string(),
                    primary_bk: "bk1".to_string(),
                    backup_bks: backup,
                    availability_score: *avail,
                    league: league.to_string(),
                    event_time: None,
                }
            })
            .collect();

        let cascade_result = selector.select_cascade(cascade_legs, 6);
        assert!(cascade_result.is_some());
        let cascade = cascade_result.unwrap();
        assert_eq!(cascade.leg_count, 6);

        // Step 2: Reorder legs for optimal execution
        let reorder_legs: Vec<_> = cascade
            .selected_legs
            .iter()
            .map(|l| express_forks::reorder::ScheduledLeg {
                position: 0,
                event_id: l.event_id.clone(),
                odds: l.odds,
                market: "1X2".to_string(),
                selection: "1".to_string(),
                event_time_minutes: Some(60 + (l.odds as u32 * 10)),
                form_score: l.availability_score,
                bookmaker: l.primary_bk.clone(),
            })
            .collect();

        let reorderer = LegReorderer::new(ReorderStrategy::Smart);
        let reorder_result = reorderer.reorder(reorder_legs);
        assert_eq!(reorder_result.reordered.len(), 6);

        // Step 3: Break-even analysis
        let odds: Vec<f64> = cascade.selected_legs.iter().map(|l| l.odds).collect();
        let stake = 1000.0;
        let breakeven = BreakEvenCalculator::analyze(6, &odds, stake, None);

        assert_eq!(breakeven.leg_count, 6);
        assert!(breakeven.total_odds > 1.0);
        assert!(breakeven.risk_reward_ratio > 0.0);

        // Step 4: Hedge analysis
        let hedge_calc = HedgeCalculator::new(HedgeStrategy::DynamicByLegs);
        let oppositions = vec![
            (0, 2.0, "bk2".to_string()),
            (1, 1.95, "bk3".to_string()),
            (2, 2.05, "bk2".to_string()),
        ];

        let hedge_result = hedge_calc.analyze_hedge(cascade.total_odds, stake, 6, oppositions);
        assert!(hedge_result.total_hedge_stake > 0.0);

        // Verify all components work together
        println!("6-Leg Parlay Pipeline:");
        println!(
            "  Cascade: {} legs, {:.2} total odds",
            cascade.leg_count, cascade.total_odds
        );
        println!(
            "  Reorder: efficiency {:.1}%",
            reorder_result.efficiency_score * 100.0
        );
        println!(
            "  Break-even: ROI {:.1}%, Risk/Reward {:.2}x",
            breakeven.roi_percentage, breakeven.risk_reward_ratio
        );
        println!(
            "  Hedge: stake {:.0}, guaranteed profit {:.0}",
            hedge_result.total_hedge_stake, hedge_result.guaranteed_profit
        );
    }

    #[test]
    fn test_hedge_vs_no_hedge_comparison() {
        let odds = vec![2.5, 2.3, 2.2];
        let stake = 1000.0;

        // No hedge
        let be_no_hedge = BreakEvenCalculator::analyze(3, &odds, stake, None);
        let ev_no_hedge = BreakEvenCalculator::calculate_expected_value(&be_no_hedge);

        // With hedge (20%)
        let hedge_calc = HedgeCalculator::new(HedgeStrategy::Percentage(20.0));
        let hedge_odds = vec![(0, 1.95, "bk2".to_string()), (1, 1.90, "bk3".to_string())];
        let hedge_result =
            hedge_calc.analyze_hedge(odds.iter().product::<f64>(), stake, 3, hedge_odds);

        assert!(hedge_result.total_hedge_stake > 0.0);
        assert!(hedge_result.guaranteed_profit < be_no_hedge.best_case_profit);
    }

    #[test]
    fn test_reorder_impact_calculation() {
        let original_odds = vec![1.8, 2.0, 2.5];
        let original_cumulative: f64 = original_odds.iter().product();

        // Simulate best reordering (highest first)
        let mut best_odds = original_odds.clone();
        best_odds.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let best_cumulative: f64 = best_odds.iter().product();

        // Both should be equal since it's the same legs
        assert!((original_cumulative - best_cumulative).abs() < 0.01);

        let reorderer = LegReorderer::new(ReorderStrategy::HighestOddsFirst);
        let impact =
            reorderer.calculate_reorder_impact(original_cumulative, best_cumulative, 1000.0);
        assert!(impact >= 0.0);
    }

    #[test]
    fn test_7leg_ultra_high_risk_analysis() {
        let odds = vec![1.7, 1.75, 1.8, 1.85, 1.9, 1.95, 2.0];
        let stake = 1000.0;

        let analysis = BreakEvenCalculator::analyze(7, &odds, stake, None);

        // Verify extreme variance for 7-leg
        assert!(analysis.variance > 0.0);
        assert!(analysis.kelly_fraction <= 0.20);

        // Check scenarios
        assert!(!analysis.probability_matrix.is_empty());
        let all_win = analysis.probability_matrix.iter().find(|s| s.legs_won == 7);
        assert!(all_win.is_some());
    }

    #[test]
    fn test_kelly_criterion_responsible_sizing() {
        let odds = vec![2.5, 2.5, 2.5];
        let probs = vec![0.42, 0.42, 0.42]; // Below fair odds

        let analysis = BreakEvenCalculator::analyze(3, &odds, 1000.0, Some(&probs));

        // Kelly fraction should be conservative
        assert!(analysis.kelly_fraction > 0.0);
        assert!(analysis.kelly_fraction <= 0.20);

        // Calculate safe bet size
        let safe_bet = 1000.0 * analysis.kelly_fraction;
        assert!(safe_bet < 200.0); // Max 200 from 1000 stake
    }

    #[test]
    fn test_cascade_multi_bk_distribution() {
        let selector = CascadeSelector::new(CascadeStrategy::MultiBookmakerAvailability);

        let legs = vec![
            ("e1", 2.0, 0.95, 6),
            ("e2", 1.9, 0.88, 4),
            ("e3", 2.1, 0.92, 5),
            ("e4", 2.2, 0.60, 2),
            ("e5", 2.3, 0.85, 3),
        ];

        let cascade_legs: Vec<_> = legs
            .iter()
            .map(|(id, odds, avail, bks)| {
                let mut backup = vec![];
                for i in 1..*bks {
                    backup.push(format!("bk{}", i + 1));
                }
                express_forks::cascade::CascadeLeg {
                    position: 0,
                    event_id: id.to_string(),
                    odds: *odds,
                    selection: "1".to_string(),
                    primary_bk: "bk1".to_string(),
                    backup_bks: backup,
                    availability_score: *avail,
                    league: "Test".to_string(),
                    event_time: None,
                }
            })
            .collect();

        let result = selector.select_cascade(cascade_legs, 4);
        assert!(result.is_some());

        let cascade = result.unwrap();
        assert!(cascade.multi_bk_score > 0.5);
    }

    #[test]
    fn test_recommended_parlay_size() {
        // Test different leg counts
        for legs in 2..=7 {
            let odds = vec![2.0; legs];
            let analysis = BreakEvenCalculator::analyze(legs, &odds, 1000.0, None);

            let rec = BreakEvenCalculator::get_recommendation(&analysis);
            assert!(!rec.is_empty());

            // All-2.0 legs should have negative EV for bookmaker's margin
            let ev = BreakEvenCalculator::calculate_expected_value(&analysis);
            assert!(ev < 0.0); // Negative EV
        }
    }

    #[test]
    fn test_parlay_comparison_2v3_legs() {
        let odds_2leg = vec![2.5, 2.5];
        let odds_3leg = vec![2.0, 2.0, 2.0];

        let comparison = BreakEvenCalculator::compare_parlays(&odds_2leg, &odds_3leg, 1000.0);
        assert!(!comparison.is_empty());
        assert!(comparison.contains("superior"));
    }

    #[test]
    fn test_hedge_percentage_by_leg_count() {
        for leg_count in 2..=7 {
            let hedge_calc = HedgeCalculator::new(HedgeStrategy::DynamicByLegs);
            let odds = vec![2.0; leg_count];
            let oppositions: Vec<_> = (0..leg_count)
                .map(|i| (i, 1.95, format!("bk{}", i + 2)))
                .collect();

            let analysis = hedge_calc.analyze_hedge(
                odds.iter().product::<f64>(),
                1000.0,
                leg_count,
                oppositions,
            );

            let expected_hedge = 1000.0 * hedge_calc.get_hedge_percentage(leg_count) / 100.0;
            assert!(
                (analysis.total_hedge_stake - expected_hedge).abs() < 10.0,
                "Leg count: {}, stake: {:.0}, expected: {:.0}",
                leg_count,
                analysis.total_hedge_stake,
                expected_hedge
            );
        }
    }

    #[test]
    fn test_efficient_frontier_analysis() {
        // Test if adding more legs improves or worsens efficiency
        let mut odds_list = vec![];
        for legs in 2..=6 {
            let odds = vec![1.95; legs];
            odds_list.push((legs, odds));
        }

        let mut prev_roi = 0.0;
        for (leg_count, odds) in odds_list {
            let analysis = BreakEvenCalculator::analyze(leg_count, &odds, 1000.0, None);
            let roi = analysis.roi_percentage;

            println!(
                "{}-leg parlay: ROI {:.2}%, Var {:.0}",
                leg_count, roi, analysis.variance
            );

            // ROI should decrease with more legs (if same odds)
            assert!(roi <= prev_roi + 0.1 || prev_roi == 0.0);
            prev_roi = roi;
        }
    }

    #[test]
    fn test_decorrelated_cascades() {
        let selector = CascadeSelector::new(CascadeStrategy::DecorrelatedEvents);

        let legs = vec![
            ("e1", 2.0, "EPL", 0.8),
            ("e2", 1.95, "EPL", 0.75),
            ("e3", 2.1, "LaLiga", 0.85),
            ("e4", 2.05, "LaLiga", 0.80),
            ("e5", 2.15, "Serie A", 0.88),
            ("e6", 2.05, "Serie A", 0.82),
        ];

        let cascade_legs: Vec<_> = legs
            .iter()
            .map(
                |(id, odds, league, avail)| express_forks::cascade::CascadeLeg {
                    position: 0,
                    event_id: id.to_string(),
                    odds: *odds,
                    selection: "1".to_string(),
                    primary_bk: "bk1".to_string(),
                    backup_bks: vec!["bk2".to_string(), "bk3".to_string()],
                    availability_score: *avail,
                    league: league.to_string(),
                    event_time: None,
                },
            )
            .collect();

        let result = selector.select_cascade(cascade_legs, 4).unwrap();

        // Check that we didn't pick more than 2 from same league
        let mut league_count = std::collections::HashMap::new();
        for leg in &result.selected_legs {
            *league_count.entry(&leg.league).or_insert(0) += 1;
        }

        for count in league_count.values() {
            assert!(*count <= 2, "Too many legs from same league");
        }
    }

    #[test]
    fn test_stress_test_40_leg_combinations() {
        // Generate 40 events and test that algorithms don't crash
        let events_count = 40;
        let odds = vec![2.0; events_count];

        // Test break-even analysis
        let be = BreakEvenCalculator::analyze(7, &odds[0..7], 1000.0, None);
        assert_eq!(be.leg_count, 7);

        // Test hedge
        let hedge = HedgeCalculator::new(HedgeStrategy::Percentage(20.0));
        let oppositions: Vec<_> = (0..7).map(|i| (i, 1.95, format!("bk{}", i))).collect();
        let hedge_result = hedge.analyze_hedge(2.0_f64.powi(7), 1000.0, 7, oppositions);
        assert!(hedge_result.total_hedge_stake > 0.0);

        // Test reorder
        let reorder_legs: Vec<_> = (0..7)
            .map(|i| express_forks::reorder::ScheduledLeg {
                position: i,
                event_id: format!("e{}", i),
                odds: 2.0,
                market: "1X2".to_string(),
                selection: "1".to_string(),
                event_time_minutes: Some((i as u32) * 60),
                form_score: 0.8,
                bookmaker: "test".to_string(),
            })
            .collect();

        let reorderer = LegReorderer::new(ReorderStrategy::Smart);
        let reorder_result = reorderer.reorder(reorder_legs);
        assert_eq!(reorder_result.reordered.len(), 7);
    }

    #[test]
    fn test_parlay_edge_with_real_odds() {
        // Test with realistic bookmaker margins
        let odds = vec![1.95, 1.95, 1.95]; // Typical BK odds with ~2.6% margin each

        let edge = BreakEvenCalculator::calculate_parlay_edge(
            odds.iter().product::<f64>(),
            odds.iter().map(|o| 1.0 / o).product::<f64>(),
        );

        // Should be negative (BK advantage)
        assert!(edge < 0.0);
        println!("Edge with 1.95 odds: {:.2}%", edge * 100.0);
    }

    #[test]
    fn test_win_probability_scenarios() {
        let odds = vec![2.0, 2.0, 2.0];
        let probs = vec![0.52, 0.52, 0.52]; // Slight edge on each leg

        let analysis = BreakEvenCalculator::analyze(3, &odds, 1000.0, Some(&probs));

        // Scenario matrix should show different outcomes
        assert!(analysis.probability_matrix.len() > 1);

        // At least one scenario should be profitable
        let profitable = analysis
            .probability_matrix
            .iter()
            .any(|s| s.net_profit_loss > 0.0);
        assert!(profitable);
    }

    #[test]
    fn test_cascade_selection_algorithm_scalability() {
        // Test that cascade selector works efficiently with increasing leg count
        for target_legs in 2..=7 {
            let selector = CascadeSelector::new(CascadeStrategy::SmartOptimal);

            let cascade_legs: Vec<_> = (0..20)
                .map(|i| express_forks::cascade::CascadeLeg {
                    position: 0,
                    event_id: format!("e{}", i),
                    odds: 1.8 + (i as f64 * 0.05),
                    selection: "1".to_string(),
                    primary_bk: format!("bk{}", (i % 5) + 1),
                    backup_bks: vec![format!("bk{}", ((i + 1) % 5) + 1)],
                    availability_score: 0.7 + (i as f64 * 0.01),
                    league: match i % 5 {
                        0 => "EPL",
                        1 => "LaLiga",
                        2 => "Serie A",
                        3 => "Bundesliga",
                        _ => "Ligue 1",
                    }
                    .to_string(),
                    event_time: None,
                })
                .collect();

            let result = selector.select_cascade(cascade_legs, target_legs);
            assert!(result.is_some());
            assert_eq!(result.unwrap().leg_count, target_legs);
        }
    }
}
