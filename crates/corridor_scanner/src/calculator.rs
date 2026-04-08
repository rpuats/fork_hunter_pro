use shared::odds::decimal_to_implied_probability;
use std::collections::HashMap;

pub struct CorridorCalculator;

impl CorridorCalculator {
    pub fn find_corridors(
        all_odds: &[shared::Odd],
        min_corridor_size: f64,
    ) -> Vec<shared::CorridorOpportunity> {
        let mut corridors = Vec::new();
        let by_market = Self::group_by_market(all_odds);

        for (_market_key, odds) in &by_market {
            let over_odds: Vec<_> = odds.iter()
                .filter(|o| Self::is_over(o))
                .collect();
            let under_odds: Vec<_> = odds.iter()
                .filter(|o| Self::is_under(o))
                .collect();

            for &over in &over_odds {
                for &under in &under_odds {
                    if over.bookmaker_slug == under.bookmaker_slug {
                        continue;
                    }
                    if let (Some(line_over), Some(line_under)) = (over.line, under.line) {
                        if line_over > line_under {
                            let corridor_size = line_over - line_under;
                            if corridor_size >= min_corridor_size {
                                let double_win_prob = Self::calc_double_win_probability(over.odds, under.odds);
                                let expected_roi = Self::calc_expected_roi(over.odds, under.odds, corridor_size);

                                corridors.push(shared::CorridorOpportunity {
                                    id: uuid::Uuid::new_v4(),
                                    sport: over.market.contains("football").then(|| shared::Sport::Football).unwrap_or(shared::Sport::Other),
                                    league: String::new(),
                                    home_team: String::new(),
                                    away_team: String::new(),
                                    start_time: None,
                                    is_live: false,
                                    bookmaker_a: over.bookmaker_slug.clone(),
                                    bookmaker_b: under.bookmaker_slug.clone(),
                                    market: over.market.clone(),
                                    line_a: line_over,
                                    odds_a: over.odds,
                                    line_b: line_under,
                                    odds_b: under.odds,
                                    corridor_size,
                                    double_win_probability: double_win_prob,
                                    expected_roi,
                                    detected_at: chrono::Utc::now(),
                                });
                            }
                        }
                    }
                }
            }
        }

        corridors.sort_by(|a, b| b.expected_roi.partial_cmp(&a.expected_roi).unwrap_or(std::cmp::Ordering::Equal));
        corridors
    }

    pub fn find_asian_handicap_corridors(
        all_odds: &[shared::Odd],
        min_corridor_size: f64,
    ) -> Vec<shared::CorridorOpportunity> {
        let mut corridors = Vec::new();
        let by_market = Self::group_by_market(all_odds);

        for (_market_key, odds) in &by_market {
            let ah_odds: Vec<_> = odds.iter()
                .filter(|o| o.market.to_lowercase().contains("asian") || o.market.to_lowercase().contains("азиат"))
                .collect();

            for (i, odd_a) in ah_odds.iter().enumerate() {
                for odd_b in ah_odds.iter().skip(i + 1) {
                    if odd_a.bookmaker_slug == odd_b.bookmaker_slug { continue; }
                    if let (Some(line_a), Some(line_b)) = (odd_a.line, odd_b.line) {
                        if (line_a - line_b).abs() > 0.0 && (line_a - line_b).abs() <= 1.0 {
                            let corridor_size = (line_a - line_b).abs();
                            if corridor_size >= min_corridor_size {
                                corridors.push(shared::CorridorOpportunity {
                                    id: uuid::Uuid::new_v4(),
                                    sport: shared::Sport::Football,
                                    league: String::new(),
                                    home_team: String::new(),
                                    away_team: String::new(),
                                    start_time: None,
                                    is_live: false,
                                    bookmaker_a: odd_a.bookmaker_slug.clone(),
                                    bookmaker_b: odd_b.bookmaker_slug.clone(),
                                    market: odd_a.market.clone(),
                                    line_a,
                                    odds_a: odd_a.odds,
                                    line_b,
                                    odds_b: odd_b.odds,
                                    corridor_size,
                                    double_win_probability: 0.0,
                                    expected_roi: 0.0,
                                    detected_at: chrono::Utc::now(),
                                });
                            }
                        }
                    }
                }
            }
        }

        corridors
    }

    fn group_by_market<'a>(all_odds: &'a [shared::Odd]) -> HashMap<String, Vec<&'a shared::Odd>> {
        let mut map: HashMap<String, Vec<&'a shared::Odd>> = HashMap::new();
        for odd in all_odds {
            if odd.line.is_some() {
                let key = format!("{}|{}", odd.market, odd.event_id);
                map.entry(key).or_default().push(odd);
            }
        }
        map
    }

    fn is_over(odd: &shared::Odd) -> bool {
        let sel = odd.selection.to_lowercase();
        sel.contains("over") || sel.contains("больше") || sel.contains("тб")
    }

    fn is_under(odd: &shared::Odd) -> bool {
        let sel = odd.selection.to_lowercase();
        sel.contains("under") || sel.contains("меньше") || sel.contains("тм")
    }

    fn calc_double_win_probability(odds_a: f64, odds_b: f64) -> f64 {
        let prob_a = 1.0 - decimal_to_implied_probability(odds_a);
        let prob_b = 1.0 - decimal_to_implied_probability(odds_b);
        (prob_a + prob_b).min(1.0) * 100.0
    }

    fn calc_expected_roi(odds_a: f64, odds_b: f64, _corridor_size: f64) -> f64 {
        let stake = 1000.0;
        let stake_a = stake / 2.0;
        let stake_b = stake / 2.0;
        let payout_a = stake_a * odds_a;
        let payout_b = stake_b * odds_b;
        let avg_payout = (payout_a + payout_b) / 2.0;
        ((avg_payout - stake) / stake) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;

    fn make_odd(event_id: &str, bk: &str, sel: &str, odds: f64, line: f64) -> shared::Odd {
        shared::Odd {
            id: format!("{}-{}-{}", event_id, bk, sel),
            event_id: event_id.into(),
            bookmaker_slug: bk.into(),
            market: "Total".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Over,
            line: Some(line),
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_find_corridor() {
        // Over 3.5 у bk1 и Under 2.5 у bk2 => corridor_size = 3.5 - 2.5 = 1.0
        let odds = vec![
            make_odd("e1", "bk1", "Over", 1.90, 3.5),
            make_odd("e1", "bk2", "Under", 1.90, 2.5),
        ];
        let corridors = CorridorCalculator::find_corridors(&odds, 0.5);
        assert!(!corridors.is_empty());
        assert!((corridors[0].corridor_size - 1.0).abs() < 0.01);
    }
}
