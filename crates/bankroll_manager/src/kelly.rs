pub struct KellyCalculator;

impl KellyCalculator {
    /// Рассчитывает долю Келли.
    /// `prob` — истинная вероятность события (0.0-1.0)
    /// `odds` — десятичный коэффициент
    pub fn full_kelly(prob: f64, odds: f64) -> f64 {
        if odds <= 1.0 || prob <= 0.0 {
            return 0.0;
        }
        let q = 1.0 - prob;
        let b = odds - 1.0;
        if b <= 0.0 {
            return 0.0;
        }
        let kelly = (b * prob - q) / b;
        kelly.max(0.0)
    }

    pub fn fractional_kelly(edge: f64, odds: f64, fraction: f64) -> f64 {
        let full = Self::full_kelly(edge, odds);
        full * fraction
    }

    pub fn optimal_stake(
        bankroll: f64,
        edge: f64,
        odds: f64,
        kelly_fraction: f64,
        max_exposure_percent: f64,
    ) -> f64 {
        let kelly_stake = bankroll * Self::fractional_kelly(edge, odds, kelly_fraction);
        let max_stake = bankroll * (max_exposure_percent / 100.0);
        kelly_stake.min(max_stake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_kelly_positive() {
        // Истинная вероятность 50%, коэффициент 2.10 (подразумевается 47.6%)
        // Это валуйная ставка — Kelly должен быть положительным
        let stake = KellyCalculator::full_kelly(0.50, 2.10);
        assert!(stake > 0.0);
        // Проверим значение: b=1.1, q=0.5, kelly = (1.1*0.5 - 0.5)/1.1 = 0.05/1.1 ≈ 0.045
        assert!((stake - 0.0454).abs() < 0.001);
    }

    #[test]
    fn test_full_kelly_negative() {
        // Истинная вероятность 40%, коэффициент 1.80 (подразумевается 55.6%)
        // Это невалуйная ставка — Kelly должен быть 0
        let stake = KellyCalculator::full_kelly(0.40, 1.80);
        assert!(stake == 0.0);
    }

    #[test]
    fn test_fractional_kelly() {
        let full = KellyCalculator::full_kelly(0.50, 2.10);
        let frac = KellyCalculator::fractional_kelly(0.50, 2.10, 0.25);
        assert!((frac - full * 0.25).abs() < 0.0001);
    }

    #[test]
    fn test_optimal_stake_capped() {
        let stake = KellyCalculator::optimal_stake(10000.0, 0.50, 2.10, 0.25, 2.0);
        assert!(stake <= 200.0);
    }
}
