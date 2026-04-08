use chrono::Utc;
use shared::Odd;
use shared::MirrorLine;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct MirrorDetector {
    tolerance: f64,
}

impl MirrorDetector {
    pub fn new(tolerance: f64) -> Self {
        Self { tolerance }
    }

    pub fn find_mirrors(&self, all_odds: &[Odd]) -> Vec<MirrorLine> {
        let mut mirrors = Vec::new();
        let by_market_line = self.group_by_market_line(all_odds);

        for (_key, odds) in &by_market_line {
            if odds.len() < 2 { continue; }

            for (i, odd_a) in odds.iter().enumerate() {
                for odd_b in odds.iter().skip(i + 1) {
                    if odd_a.bookmaker_slug == odd_b.bookmaker_slug { continue; }

                    if let (Some(line_a), Some(line_b)) = (odd_a.line, odd_b.line) {
                        if (line_a - line_b).abs() < self.tolerance {
                            mirrors.push(MirrorLine {
                                id: Uuid::new_v4(),
                                market: odd_a.market.clone(),
                                line: line_a,
                                bookmaker_a: odd_a.bookmaker_slug.clone(),
                                odds_a: odd_a.odds,
                                bookmaker_b: odd_b.bookmaker_slug.clone(),
                                odds_b: odd_b.odds,
                                detected_at: Utc::now(),
                            });
                        }
                    }
                }
            }
        }

        mirrors
    }

    pub fn is_mirror(&self, odds_a: &Odd, odds_b: &Odd) -> bool {
        if odds_a.market != odds_b.market { return false; }
        if odds_a.selection != odds_b.selection { return false; }
        match (odds_a.line, odds_b.line) {
            (Some(a), Some(b)) => (a - b).abs() < self.tolerance,
            (None, None) => true,
            _ => false,
        }
    }

    fn group_by_market_line<'a>(&self, all_odds: &'a [Odd]) -> HashMap<String, Vec<&'a Odd>> {
        let mut map: HashMap<String, Vec<&'a Odd>> = HashMap::new();
        for odd in all_odds {
            let key = format!("{}|{}|{}", odd.market, odd.selection, odd.line.map(|l| l.to_string()).unwrap_or_else(|| "none".into()));
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;

    fn make_odd(bk: &str, sel: &str, odds: f64, line: Option<f64>) -> Odd {
        Odd {
            id: format!("{}-{}", bk, sel),
            event_id: "evt1".into(),
            bookmaker_slug: bk.into(),
            market: "Total".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Over,
            line,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_find_mirror() {
        let detector = MirrorDetector::new(0.1);
        let odds = vec![
            make_odd("bk1", "Over", 1.90, Some(2.5)),
            make_odd("bk2", "Over", 2.10, Some(2.5)),
        ];
        let mirrors = detector.find_mirrors(&odds);
        assert!(!mirrors.is_empty());
        assert!((mirrors[0].line - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_no_mirror_different_lines() {
        let detector = MirrorDetector::new(0.1);
        let odds = vec![
            make_odd("bk1", "Over", 1.90, Some(2.5)),
            make_odd("bk2", "Over", 2.10, Some(3.5)),
        ];
        let mirrors = detector.find_mirrors(&odds);
        assert!(mirrors.is_empty());
    }
}
