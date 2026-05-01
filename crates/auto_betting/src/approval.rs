use shared::{
    BookmakerExecutionMode, StakeValidationDecision, StakeValidationRequest, StakeValidationResult,
    Surebet, SurebetLeg,
};

use crate::registry::ExecutionRegistry;
use crate::validator::StakeValidator;

pub const PARI_ROLLOUT_BOOKMAKER: &str = "pari";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalGateDecision {
    AllowDryRun,
    AllowSubmission,
    RequireOperatorApproval,
    Reject,
}

#[derive(Debug, Clone)]
pub struct RankedLegPlan {
    pub leg: SurebetLeg,
    pub rank: usize,
    pub score: i32,
    pub validation: StakeValidationResult,
    pub executable: bool,
    pub dry_run_ready: bool,
    pub placement_requested: bool,
    pub decision: ApprovalGateDecision,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SurebetExecutionPlan {
    pub executable: bool,
    pub ranked_legs: Vec<RankedLegPlan>,
}

impl SurebetExecutionPlan {
    pub fn blocking_reasons(&self) -> Vec<String> {
        self.ranked_legs
            .iter()
            .filter(|leg| {
                !matches!(
                    leg.decision,
                    ApprovalGateDecision::AllowDryRun | ApprovalGateDecision::AllowSubmission
                )
            })
            .flat_map(|leg| {
                leg.reasons
                    .iter()
                    .map(|reason| format!("{}#{}: {reason}", leg.leg.bookmaker, leg.rank))
            })
            .collect()
    }
}

pub async fn build_surebet_execution_plan(
    registry: &ExecutionRegistry,
    surebet: &Surebet,
) -> Result<SurebetExecutionPlan, String> {
    let mut ranked_legs = Vec::with_capacity(surebet.legs.len());

    for leg in &surebet.legs {
        let capability = registry.get_capability(&leg.bookmaker);
        let account = registry.get_account(&leg.bookmaker);
        let balance_refresh = registry.refresh_balance_snapshot(&leg.bookmaker).await?;
        let mut validation = StakeValidator::validate(&StakeValidationRequest {
            bookmaker: leg.bookmaker.clone(),
            desired_stake: leg.stake,
            min_stake: None,
            max_stake: None,
            bookmaker_available_balance: balance_refresh
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.available_balance),
            bankroll_available_balance: None,
            allow_auto_adjust: true,
        });
        let mut reasons = Vec::new();

        let Some(account) = account else {
            validation.decision = StakeValidationDecision::Reject;
            reasons.push("bookmaker account is not configured".into());
            ranked_legs.push(RankedLegPlan {
                leg: leg.clone(),
                rank: 0,
                score: -100,
                validation,
                executable: false,
                dry_run_ready: false,
                placement_requested: false,
                decision: ApprovalGateDecision::Reject,
                reasons,
            });
            continue;
        };

        if !account.enabled {
            validation.decision = StakeValidationDecision::Reject;
            reasons.push("bookmaker account is disabled".into());
        }

        if matches!(account.mode, BookmakerExecutionMode::Disabled) {
            validation.decision = StakeValidationDecision::Reject;
            reasons.push("bookmaker account mode is disabled".into());
        }

        if capability.requires_session && !balance_refresh.session_status.authenticated {
            validation.decision = StakeValidationDecision::Reject;
            reasons.push(
                balance_refresh
                    .session_status
                    .detail
                    .clone()
                    .unwrap_or_else(|| "bookmaker session is not authenticated".into()),
            );
        }

        if capability.supports_balance_snapshot && balance_refresh.snapshot.is_none() {
            validation.decision = StakeValidationDecision::Reject;
            reasons.push(
                balance_refresh
                    .detail
                    .clone()
                    .unwrap_or_else(|| "bookmaker balance snapshot is unavailable".into()),
            );
        }

        if matches!(validation.decision, StakeValidationDecision::Reject) {
            reasons.extend(validation.reasons.iter().cloned());
        }

        let executable = account.enabled
            && !matches!(account.mode, BookmakerExecutionMode::Disabled)
            && (!capability.requires_session || balance_refresh.session_status.authenticated)
            && (!capability.supports_balance_snapshot || balance_refresh.snapshot.is_some())
            && !matches!(validation.decision, StakeValidationDecision::Reject);
        let dry_run_ready =
            executable && capability.supports_dry_run && account.mode.allows_dry_run();
        let placement_requested =
            account.mode.allows_submission_path() && capability.supports_bet_placement;

        let decision = if !dry_run_ready {
            if reasons.is_empty() {
                reasons.push("dry-run path is not ready for execution".into());
            }
            ApprovalGateDecision::Reject
        } else if leg.bookmaker == PARI_ROLLOUT_BOOKMAKER && placement_requested {
            reasons.push(
                "pari rollout gate is active: operator approval is required and coupon submit stays disabled"
                    .into(),
            );
            ApprovalGateDecision::RequireOperatorApproval
        } else if placement_requested
            && capability.supports_real_money
            && capability.supports_bet_placement
            && account.mode.allows_submission_path()
        {
            ApprovalGateDecision::AllowSubmission
        } else {
            ApprovalGateDecision::AllowDryRun
        };

        let mut score = 0;
        if dry_run_ready {
            score += 100;
        }
        if balance_refresh.session_status.authenticated || !capability.requires_session {
            score += 25;
        }
        if balance_refresh.snapshot.is_some() || !capability.supports_balance_snapshot {
            score += 20;
        }
        if account.enabled {
            score += 10;
        }
        if placement_requested {
            score -= 15;
        }
        if matches!(decision, ApprovalGateDecision::RequireOperatorApproval) {
            score -= 40;
        }
        if matches!(decision, ApprovalGateDecision::Reject) {
            score -= 100;
        }

        ranked_legs.push(RankedLegPlan {
            leg: leg.clone(),
            rank: 0,
            score,
            validation,
            executable,
            dry_run_ready,
            placement_requested,
            decision,
            reasons,
        });
    }

    ranked_legs.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.leg.stake.total_cmp(&left.leg.stake))
            .then_with(|| left.leg.bookmaker.cmp(&right.leg.bookmaker))
    });

    for (index, leg) in ranked_legs.iter_mut().enumerate() {
        leg.rank = index + 1;
    }

    let executable = ranked_legs.iter().all(|leg| {
        matches!(
            leg.decision,
            ApprovalGateDecision::AllowDryRun | ApprovalGateDecision::AllowSubmission
        )
    });

    Ok(SurebetExecutionPlan {
        executable,
        ranked_legs,
    })
}
