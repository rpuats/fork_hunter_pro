use shared::{
    BookmakerBalance, DepositAllocationGuidance, DepositAllocationTarget, DepositUrgency,
};

pub struct DepositAllocator;

impl DepositAllocator {
    pub fn build_guidance(
        balances: &[BookmakerBalance],
        total_budget_limit: f64,
    ) -> DepositAllocationGuidance {
        let available_total: f64 = balances.iter().map(|b| b.available.max(0.0)).sum();
        let target_per_bookmaker = if balances.is_empty() {
            0.0
        } else {
            (total_budget_limit.max(available_total)) / balances.len() as f64
        };

        let mut targets: Vec<DepositAllocationTarget> = balances
            .iter()
            .map(|balance| {
                let gap = (target_per_bookmaker - balance.available).max(0.0);
                let transferable_in = balances
                    .iter()
                    .filter(|other| other.bookmaker != balance.bookmaker)
                    .map(|other| (other.available - target_per_bookmaker).max(0.0))
                    .sum::<f64>();
                let recommended = gap.min(transferable_in.max(gap));
                let urgency = if gap > target_per_bookmaker * 0.5 {
                    DepositUrgency::High
                } else if gap > target_per_bookmaker * 0.2 {
                    DepositUrgency::Medium
                } else {
                    DepositUrgency::Low
                };

                DepositAllocationTarget {
                    bookmaker: balance.bookmaker.clone(),
                    current_available: balance.available,
                    target_available: target_per_bookmaker,
                    recommended_deposit: recommended,
                    deposit_gap: gap,
                    urgency,
                    note: if gap <= 0.0 {
                        "already funded above target".into()
                    } else {
                        "fund if this bookmaker is needed for next execution window".into()
                    },
                }
            })
            .filter(|target| target.deposit_gap > 0.0)
            .collect();

        targets.sort_by(|a, b| {
            b.deposit_gap
                .partial_cmp(&a.deposit_gap)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_recommended: f64 = targets.iter().map(|target| target.recommended_deposit).sum();

        DepositAllocationGuidance {
            total_budget_limit,
            current_available_total: available_total,
            target_per_bookmaker,
            total_recommended_deposit: total_recommended,
            targets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prioritizes_underfunded_bookmakers() {
        let balances = vec![
            BookmakerBalance {
                bookmaker: "bk1".into(),
                balance: 15_000.0,
                exposure: 1_000.0,
                available: 14_000.0,
                recommended_deposit: 0.0,
                recommended_withdraw: 0.0,
            },
            BookmakerBalance {
                bookmaker: "bk2".into(),
                balance: 2_000.0,
                exposure: 500.0,
                available: 1_500.0,
                recommended_deposit: 0.0,
                recommended_withdraw: 0.0,
            },
            BookmakerBalance {
                bookmaker: "bk3".into(),
                balance: 3_000.0,
                exposure: 500.0,
                available: 2_500.0,
                recommended_deposit: 0.0,
                recommended_withdraw: 0.0,
            },
        ];

        let guidance = DepositAllocator::build_guidance(&balances, 18_000.0);
        assert_eq!(guidance.targets.first().unwrap().bookmaker, "bk2");
        assert!(guidance.total_recommended_deposit > 0.0);
    }
}
