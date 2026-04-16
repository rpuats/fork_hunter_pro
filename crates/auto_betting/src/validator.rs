use shared::{StakeValidationDecision, StakeValidationRequest, StakeValidationResult};

pub struct StakeValidator;

impl StakeValidator {
    pub fn validate(request: &StakeValidationRequest) -> StakeValidationResult {
        let mut reasons = Vec::new();
        let mut adjusted_stake = request.desired_stake;

        if !request.desired_stake.is_finite() {
            reasons.push("stake must be finite".to_string());
            return StakeValidationResult {
                decision: StakeValidationDecision::Reject,
                adjusted_stake: 0.0,
                reasons,
            };
        }

        if let Some(min_stake) = request.min_stake {
            if !min_stake.is_finite() || min_stake < 0.0 {
                reasons.push("minimum stake must be a finite non-negative value".to_string());
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake: 0.0,
                    reasons,
                };
            }
        }

        if let Some(max_stake) = request.max_stake {
            if !max_stake.is_finite() || max_stake < 0.0 {
                reasons.push("maximum stake must be a finite non-negative value".to_string());
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake: 0.0,
                    reasons,
                };
            }
        }

        if let (Some(min_stake), Some(max_stake)) = (request.min_stake, request.max_stake) {
            if min_stake > max_stake {
                reasons.push(format!(
                    "bookmaker minimum stake {:.2} exceeds maximum {:.2}",
                    min_stake, max_stake
                ));
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake: 0.0,
                    reasons,
                };
            }
        }

        if let Some(bookmaker_available) = request.bookmaker_available_balance {
            if !bookmaker_available.is_finite() || bookmaker_available < 0.0 {
                reasons.push(
                    "bookmaker available balance must be a finite non-negative value".to_string(),
                );
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake: 0.0,
                    reasons,
                };
            }
        }

        if let Some(bankroll_available) = request.bankroll_available_balance {
            if !bankroll_available.is_finite() || bankroll_available < 0.0 {
                reasons.push(
                    "bankroll available balance must be a finite non-negative value".to_string(),
                );
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake: 0.0,
                    reasons,
                };
            }
        }

        if request.desired_stake <= 0.0 {
            reasons.push("stake must be positive".to_string());
            return StakeValidationResult {
                decision: StakeValidationDecision::Reject,
                adjusted_stake: 0.0,
                reasons,
            };
        }

        if let Some(min_stake) = request.min_stake {
            if adjusted_stake < min_stake {
                if request.allow_auto_adjust {
                    adjusted_stake = min_stake;
                    reasons.push(format!(
                        "stake increased to bookmaker minimum {:.2}",
                        min_stake
                    ));
                } else {
                    reasons.push(format!(
                        "stake {:.2} below bookmaker minimum {:.2}",
                        adjusted_stake, min_stake
                    ));
                    return StakeValidationResult {
                        decision: StakeValidationDecision::Reject,
                        adjusted_stake,
                        reasons,
                    };
                }
            }
        }

        if let Some(max_stake) = request.max_stake {
            if adjusted_stake > max_stake {
                if request.allow_auto_adjust {
                    adjusted_stake = max_stake;
                    reasons.push(format!(
                        "stake reduced to bookmaker maximum {:.2}",
                        max_stake
                    ));
                } else {
                    reasons.push(format!(
                        "stake {:.2} above bookmaker maximum {:.2}",
                        adjusted_stake, max_stake
                    ));
                    return StakeValidationResult {
                        decision: StakeValidationDecision::Reject,
                        adjusted_stake,
                        reasons,
                    };
                }
            }
        }

        if let Some(bookmaker_available) = request.bookmaker_available_balance {
            if adjusted_stake > bookmaker_available {
                if request.allow_auto_adjust && bookmaker_available > 0.0 {
                    adjusted_stake = adjusted_stake.min(bookmaker_available);
                    reasons.push(format!(
                        "stake capped by available bookmaker balance {:.2}",
                        bookmaker_available
                    ));
                } else {
                    reasons.push(format!(
                        "stake {:.2} exceeds available bookmaker balance {:.2}",
                        adjusted_stake, bookmaker_available
                    ));
                    return StakeValidationResult {
                        decision: StakeValidationDecision::Reject,
                        adjusted_stake,
                        reasons,
                    };
                }
            }
        }

        if let Some(bankroll_available) = request.bankroll_available_balance {
            if adjusted_stake > bankroll_available {
                if request.allow_auto_adjust && bankroll_available > 0.0 {
                    adjusted_stake = adjusted_stake.min(bankroll_available);
                    reasons.push(format!(
                        "stake capped by bankroll allowance {:.2}",
                        bankroll_available
                    ));
                } else {
                    reasons.push(format!(
                        "stake {:.2} exceeds bankroll allowance {:.2}",
                        adjusted_stake, bankroll_available
                    ));
                    return StakeValidationResult {
                        decision: StakeValidationDecision::Reject,
                        adjusted_stake,
                        reasons,
                    };
                }
            }
        }

        if adjusted_stake <= 0.0 {
            reasons.push("no executable stake remains after constraints".to_string());
            return StakeValidationResult {
                decision: StakeValidationDecision::Reject,
                adjusted_stake: 0.0,
                reasons,
            };
        }

        if let Some(min_stake) = request.min_stake {
            if adjusted_stake < min_stake {
                reasons.push(format!(
                    "available executable stake {:.2} remains below bookmaker minimum {:.2}",
                    adjusted_stake, min_stake
                ));
                return StakeValidationResult {
                    decision: StakeValidationDecision::Reject,
                    adjusted_stake,
                    reasons,
                };
            }
        }

        let decision = if (adjusted_stake - request.desired_stake).abs() < f64::EPSILON {
            StakeValidationDecision::Accept
        } else {
            StakeValidationDecision::Adjust
        };

        StakeValidationResult {
            decision,
            adjusted_stake,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjusts_up_to_minimum() {
        let request = StakeValidationRequest {
            bookmaker: "pari".into(),
            desired_stake: 80.0,
            min_stake: Some(100.0),
            max_stake: Some(500.0),
            bookmaker_available_balance: Some(1_000.0),
            bankroll_available_balance: Some(1_000.0),
            allow_auto_adjust: true,
        };

        let result = StakeValidator::validate(&request);
        assert!(matches!(result.decision, StakeValidationDecision::Adjust));
        assert_eq!(result.adjusted_stake, 100.0);
    }

    #[test]
    fn rejects_when_above_max_without_adjustments() {
        let request = StakeValidationRequest {
            bookmaker: "fonbet".into(),
            desired_stake: 900.0,
            min_stake: Some(50.0),
            max_stake: Some(500.0),
            bookmaker_available_balance: Some(1_000.0),
            bankroll_available_balance: Some(1_000.0),
            allow_auto_adjust: false,
        };

        let result = StakeValidator::validate(&request);
        assert!(matches!(result.decision, StakeValidationDecision::Reject));
    }

    #[test]
    fn caps_by_lowest_balance_constraint() {
        let request = StakeValidationRequest {
            bookmaker: "olimp".into(),
            desired_stake: 600.0,
            min_stake: Some(100.0),
            max_stake: Some(800.0),
            bookmaker_available_balance: Some(550.0),
            bankroll_available_balance: Some(480.0),
            allow_auto_adjust: true,
        };

        let result = StakeValidator::validate(&request);
        assert!(matches!(result.decision, StakeValidationDecision::Adjust));
        assert_eq!(result.adjusted_stake, 480.0);
    }

    #[test]
    fn rejects_when_effective_stake_falls_below_minimum_after_caps() {
        let request = StakeValidationRequest {
            bookmaker: "pari".into(),
            desired_stake: 120.0,
            min_stake: Some(100.0),
            max_stake: Some(500.0),
            bookmaker_available_balance: Some(80.0),
            bankroll_available_balance: Some(1_000.0),
            allow_auto_adjust: true,
        };

        let result = StakeValidator::validate(&request);
        assert!(matches!(result.decision, StakeValidationDecision::Reject));
        assert_eq!(result.adjusted_stake, 80.0);
        assert!(result
            .reasons
            .iter()
            .any(|reason| reason.contains("below bookmaker minimum")));
    }

    #[test]
    fn rejects_non_finite_stake_requests() {
        let request = StakeValidationRequest {
            bookmaker: "pari".into(),
            desired_stake: f64::NAN,
            min_stake: Some(10.0),
            max_stake: Some(500.0),
            bookmaker_available_balance: Some(1_000.0),
            bankroll_available_balance: Some(1_000.0),
            allow_auto_adjust: true,
        };

        let result = StakeValidator::validate(&request);
        assert!(matches!(result.decision, StakeValidationDecision::Reject));
        assert_eq!(result.adjusted_stake, 0.0);
        assert_eq!(result.reasons, vec!["stake must be finite"]);
    }
}
