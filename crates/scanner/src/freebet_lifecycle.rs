use anyhow::Result;
use bankroll_manager::manager::BankrollManager;
use bonus_hunter::hunter::BonusHunter;
use chrono::Utc;
use persistence::freebet_lifecycle::FreebetLifecycleStore;
use shared::models::{
    BonusInfo, BookmakerBalance, FreebetAutoRolloverDraft, FreebetAutoRolloverStatus,
    FreebetBookmakerAllocation, FreebetConversionPlan, FreebetFundingReadiness,
    FreebetLifecycleAction, FreebetLifecycleActionStatus, FreebetLifecycleStage,
    FreebetLifecycleState, FreebetOpportunity, FreebetProgressStatus, FreebetRolloverProgress,
};
use std::collections::HashMap;

pub fn build_recommended_freebet_plan(
    bonus_hunter: &BonusHunter,
    opportunity: &FreebetOpportunity,
) -> FreebetConversionPlan {
    bonus_hunter.build_freebet_plan(&shared::FreebetPlanRequest {
        freebet_bookmaker: opportunity.bookmaker.clone(),
        qualifying_bookmaker: opportunity.bookmaker.clone(),
        hedge_bookmaker: opportunity.hedge_bookmaker.clone(),
        market: opportunity.market.clone(),
        qualifying_selection: opportunity.selection.clone(),
        freebet_selection: opportunity.selection.clone(),
        hedge_selection: opportunity.hedge_selection.clone(),
        freebet_amount: opportunity.freebet_amount,
        qualifying_odds: opportunity.back_odds,
        back_odds: opportunity.back_odds,
        lay_odds: opportunity.lay_odds,
        estimated_qualifying_loss: 0.0,
        exchange_like_hedge: false,
    })
}

pub fn build_rollover_progress(
    bonus: &BonusInfo,
    bonus_plan: Option<&shared::BonusPlan>,
) -> Option<FreebetRolloverProgress> {
    let required_turnover = bonus_plan
        .map(|plan| plan.wager_required)
        .unwrap_or_else(|| bonus.amount * bonus.wager_requirement)
        .max(0.0);
    if required_turnover <= 0.0 {
        return None;
    }

    let completed_turnover = bonus_plan
        .map(|plan| plan.wager_done)
        .unwrap_or(bonus.wager_progress)
        .clamp(0.0, required_turnover);
    let remaining_turnover = (required_turnover - completed_turnover).max(0.0);
    let progress_percent = if required_turnover > 0.0 {
        ((completed_turnover / required_turnover) * 100.0).min(100.0)
    } else {
        100.0
    };

    let status = if completed_turnover <= 0.0 {
        FreebetProgressStatus::NotStarted
    } else if remaining_turnover <= 0.0 {
        FreebetProgressStatus::Completed
    } else {
        FreebetProgressStatus::InProgress
    };

    Some(FreebetRolloverProgress {
        required_turnover,
        completed_turnover,
        remaining_turnover,
        progress_percent,
        status,
    })
}

fn build_freebet_allocation(
    bookmaker: &str,
    balances: &HashMap<String, BookmakerBalance>,
    deposits: &HashMap<String, shared::DepositAllocationTarget>,
) -> Option<FreebetBookmakerAllocation> {
    let balance = balances.get(bookmaker);
    let deposit = deposits.get(bookmaker);

    if balance.is_none() && deposit.is_none() {
        return None;
    }

    Some(FreebetBookmakerAllocation {
        bookmaker: bookmaker.to_string(),
        available_balance: balance.map(|item| item.available),
        recommended_deposit: deposit.map(|item| item.recommended_deposit),
        deposit_gap: deposit.map(|item| item.deposit_gap),
        urgency: deposit.as_ref().map(|item| item.urgency.clone()),
        note: deposit
            .map(|item| item.note.clone())
            .unwrap_or_else(|| "no extra deposit guidance required".into()),
    })
}

pub fn infer_freebet_stage(
    opportunity: Option<&FreebetOpportunity>,
    bonus: Option<&BonusInfo>,
    plan: Option<&FreebetConversionPlan>,
    rollover: Option<&FreebetRolloverProgress>,
) -> FreebetLifecycleStage {
    if rollover
        .map(|item| item.status == FreebetProgressStatus::Completed)
        .unwrap_or(false)
    {
        return FreebetLifecycleStage::RolloverCompleted;
    }

    if rollover
        .map(|item| item.status == FreebetProgressStatus::InProgress)
        .unwrap_or(false)
    {
        return FreebetLifecycleStage::RolloverInProgress;
    }

    if plan.is_some() {
        return FreebetLifecycleStage::Planned;
    }

    if let Some(bonus) = bonus {
        return match bonus.status {
            shared::BonusStatus::Available => FreebetLifecycleStage::Available,
            shared::BonusStatus::Claimed => FreebetLifecycleStage::Qualified,
            shared::BonusStatus::Wagering => FreebetLifecycleStage::RolloverInProgress,
            shared::BonusStatus::Completed => FreebetLifecycleStage::RolloverCompleted,
            shared::BonusStatus::Expired | shared::BonusStatus::Rejected => {
                if opportunity.is_some() {
                    FreebetLifecycleStage::Discovered
                } else {
                    FreebetLifecycleStage::Available
                }
            }
        };
    }

    let _ = opportunity;
    FreebetLifecycleStage::Discovered
}

fn infer_next_milestone(
    lifecycle_stage: &FreebetLifecycleStage,
    auto_rollover: Option<&FreebetAutoRolloverDraft>,
) -> String {
    match auto_rollover.map(|item| &item.status) {
        Some(FreebetAutoRolloverStatus::AwaitingFunding) => "close_funding_gap".into(),
        Some(FreebetAutoRolloverStatus::AwaitingTrigger) => "place_manual_legs".into(),
        Some(FreebetAutoRolloverStatus::Monitoring) => "complete_rollover".into(),
        Some(FreebetAutoRolloverStatus::Completed) => "audit_snapshot".into(),
        Some(FreebetAutoRolloverStatus::DraftOnly) | None => match lifecycle_stage {
            FreebetLifecycleStage::Discovered => "review_opportunity".into(),
            FreebetLifecycleStage::Available => "claim_bonus".into(),
            FreebetLifecycleStage::Qualified => "prepare_conversion_plan".into(),
            FreebetLifecycleStage::Planned => "place_manual_legs".into(),
            FreebetLifecycleStage::RolloverInProgress => "complete_rollover".into(),
            FreebetLifecycleStage::RolloverCompleted => "audit_snapshot".into(),
        },
    }
}

fn infer_blocked_by(auto_rollover: Option<&FreebetAutoRolloverDraft>) -> Vec<String> {
    let Some(auto_rollover) = auto_rollover else {
        return Vec::new();
    };

    match auto_rollover.status {
        FreebetAutoRolloverStatus::AwaitingFunding => auto_rollover
            .funding_readiness
            .blocking_bookmakers
            .iter()
            .map(|bookmaker| format!("funding:{bookmaker}"))
            .collect(),
        FreebetAutoRolloverStatus::AwaitingTrigger => vec!["manual_trigger".into()],
        _ => Vec::new(),
    }
}

fn infer_read_only_follow_up(
    lifecycle_stage: &FreebetLifecycleStage,
    auto_rollover: Option<&FreebetAutoRolloverDraft>,
) -> String {
    if let Some(auto_rollover) = auto_rollover {
        return auto_rollover.read_only_check.clone();
    }

    match lifecycle_stage {
        FreebetLifecycleStage::Discovered => {
            "Refresh lifecycle after the next odds sync and confirm the opportunity still qualifies."
                .into()
        }
        FreebetLifecycleStage::Available => {
            "Refresh lifecycle after the next bonus sync and confirm the offer remains available."
                .into()
        }
        FreebetLifecycleStage::Qualified => {
            "Refresh lifecycle after plan review and confirm the qualifying inputs stay unchanged."
                .into()
        }
        FreebetLifecycleStage::Planned => {
            "Refresh lifecycle after manual placement and confirm the draft is still aligned."
                .into()
        }
        FreebetLifecycleStage::RolloverInProgress => {
            "Refresh lifecycle after each turnover sync and confirm the remaining requirement shrinks."
                .into()
        }
        FreebetLifecycleStage::RolloverCompleted => {
            "Refresh lifecycle only for audit and confirm the snapshot stays completed.".into()
        }
    }
}

fn infer_read_only_focus(
    lifecycle_stage: &FreebetLifecycleStage,
    auto_rollover: Option<&FreebetAutoRolloverDraft>,
) -> String {
    if let Some(auto_rollover) = auto_rollover {
        return match auto_rollover.status {
            FreebetAutoRolloverStatus::AwaitingFunding => "balance_refresh".into(),
            FreebetAutoRolloverStatus::AwaitingTrigger => "manual_settlement".into(),
            FreebetAutoRolloverStatus::Monitoring => "turnover_progress".into(),
            FreebetAutoRolloverStatus::Completed => "completion_audit".into(),
            FreebetAutoRolloverStatus::DraftOnly => "draft_review".into(),
        };
    }

    match lifecycle_stage {
        FreebetLifecycleStage::Discovered => "odds_sync".into(),
        FreebetLifecycleStage::Available => "bonus_sync".into(),
        FreebetLifecycleStage::Qualified => "plan_review".into(),
        FreebetLifecycleStage::Planned => "manual_placement".into(),
        FreebetLifecycleStage::RolloverInProgress => "turnover_progress".into(),
        FreebetLifecycleStage::RolloverCompleted => "completion_audit".into(),
    }
}

pub fn build_staged_rollover_actions(
    lifecycle_stage: &FreebetLifecycleStage,
    auto_rollover: Option<&FreebetAutoRolloverDraft>,
) -> Vec<FreebetLifecycleAction> {
    let funding_status = match auto_rollover.map(|item| &item.status) {
        Some(FreebetAutoRolloverStatus::AwaitingFunding) => FreebetLifecycleActionStatus::Ready,
        Some(FreebetAutoRolloverStatus::AwaitingTrigger)
        | Some(FreebetAutoRolloverStatus::Monitoring)
        | Some(FreebetAutoRolloverStatus::Completed) => FreebetLifecycleActionStatus::Completed,
        Some(FreebetAutoRolloverStatus::DraftOnly) => FreebetLifecycleActionStatus::Pending,
        None => match lifecycle_stage {
            FreebetLifecycleStage::Discovered | FreebetLifecycleStage::Available => {
                FreebetLifecycleActionStatus::Pending
            }
            FreebetLifecycleStage::Qualified | FreebetLifecycleStage::Planned => {
                FreebetLifecycleActionStatus::Ready
            }
            FreebetLifecycleStage::RolloverInProgress => FreebetLifecycleActionStatus::Completed,
            FreebetLifecycleStage::RolloverCompleted => FreebetLifecycleActionStatus::Completed,
        },
    };
    let trigger_status = match auto_rollover.map(|item| &item.status) {
        Some(FreebetAutoRolloverStatus::AwaitingFunding)
        | Some(FreebetAutoRolloverStatus::DraftOnly) => FreebetLifecycleActionStatus::Pending,
        Some(FreebetAutoRolloverStatus::AwaitingTrigger) => FreebetLifecycleActionStatus::Ready,
        Some(FreebetAutoRolloverStatus::Monitoring)
        | Some(FreebetAutoRolloverStatus::Completed) => FreebetLifecycleActionStatus::Completed,
        None => match lifecycle_stage {
            FreebetLifecycleStage::Discovered | FreebetLifecycleStage::Available => {
                FreebetLifecycleActionStatus::Pending
            }
            FreebetLifecycleStage::Qualified | FreebetLifecycleStage::Planned => {
                FreebetLifecycleActionStatus::Ready
            }
            FreebetLifecycleStage::RolloverInProgress
            | FreebetLifecycleStage::RolloverCompleted => FreebetLifecycleActionStatus::Completed,
        },
    };
    let monitoring_status = match auto_rollover.map(|item| &item.status) {
        Some(FreebetAutoRolloverStatus::Monitoring) => FreebetLifecycleActionStatus::Monitoring,
        Some(FreebetAutoRolloverStatus::Completed) => FreebetLifecycleActionStatus::Completed,
        _ => match lifecycle_stage {
            FreebetLifecycleStage::RolloverInProgress => FreebetLifecycleActionStatus::Monitoring,
            FreebetLifecycleStage::RolloverCompleted => FreebetLifecycleActionStatus::Completed,
            _ => FreebetLifecycleActionStatus::Pending,
        },
    };
    let audit_status = if matches!(lifecycle_stage, FreebetLifecycleStage::RolloverCompleted)
        || matches!(
            auto_rollover.map(|item| &item.status),
            Some(FreebetAutoRolloverStatus::Completed)
        ) {
        FreebetLifecycleActionStatus::Ready
    } else {
        FreebetLifecycleActionStatus::Pending
    };

    vec![
        FreebetLifecycleAction {
            key: "funding_check".into(),
            label: "Refresh funding coverage".into(),
            status: funding_status,
            detail: auto_rollover
                .map(|item| item.funding_recommendation.clone())
                .filter(|item| !item.trim().is_empty())
                .unwrap_or_else(|| {
                    "Confirm balances still cover the planned qualifying and hedge cash legs."
                        .into()
                }),
        },
        FreebetLifecycleAction {
            key: "manual_trigger".into(),
            label: "Wait for manual qualifying trigger".into(),
            status: trigger_status,
            detail: auto_rollover
                .map(|item| item.trigger.clone())
                .filter(|item| !item.trim().is_empty())
                .unwrap_or_else(|| {
                    "Manual qualifying/freebet placement must appear before execution tracking can advance."
                        .into()
                }),
        },
        FreebetLifecycleAction {
            key: "turnover_monitoring".into(),
            label: "Monitor rollover turnover".into(),
            status: monitoring_status,
            detail: auto_rollover
                .map(|item| item.read_only_check.clone())
                .filter(|item| !item.trim().is_empty())
                .unwrap_or_else(|| {
                    "Track turnover progress as a read-only workflow; no coupon submit path is armed."
                        .into()
                }),
        },
        FreebetLifecycleAction {
            key: "completion_audit".into(),
            label: "Audit completed rollover snapshot".into(),
            status: audit_status,
            detail: infer_read_only_follow_up(lifecycle_stage, auto_rollover),
        },
    ]
}

fn build_auto_rollover_draft(
    bookmaker: &str,
    lifecycle_stage: &FreebetLifecycleStage,
    bonus: Option<&BonusInfo>,
    plan: Option<&FreebetConversionPlan>,
    rollover: Option<&FreebetRolloverProgress>,
    balances: &HashMap<String, BookmakerBalance>,
) -> Option<FreebetAutoRolloverDraft> {
    if plan.is_none() && rollover.is_none() && bonus.is_none() {
        return None;
    }

    let required_cash_by_bookmaker = plan
        .map(|item| item.required_cash_by_bookmaker.clone())
        .unwrap_or_default();
    let funding_gap_by_bookmaker: HashMap<String, f64> = required_cash_by_bookmaker
        .iter()
        .filter_map(|(required_bookmaker, required_cash)| {
            let available = balances
                .get(required_bookmaker)
                .map(|item| item.available)
                .unwrap_or(0.0);
            let gap = (required_cash - available).max(0.0);
            (gap > 0.0).then(|| (required_bookmaker.clone(), gap))
        })
        .collect();
    let total_funding_gap = funding_gap_by_bookmaker.values().sum::<f64>();
    let mut blocking_bookmakers: Vec<String> = funding_gap_by_bookmaker.keys().cloned().collect();
    blocking_bookmakers.sort();
    let largest_gap = funding_gap_by_bookmaker
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(bookmaker, gap)| (bookmaker.clone(), *gap));
    let funding_readiness = FreebetFundingReadiness {
        ready: funding_gap_by_bookmaker.is_empty(),
        total_gap: total_funding_gap,
        blocking_bookmakers,
        largest_gap_bookmaker: largest_gap.as_ref().map(|(bookmaker, _)| bookmaker.clone()),
        largest_gap_amount: largest_gap.as_ref().map(|(_, gap)| *gap),
    };

    let status = if matches!(lifecycle_stage, &FreebetLifecycleStage::RolloverCompleted) {
        FreebetAutoRolloverStatus::Completed
    } else if !funding_gap_by_bookmaker.is_empty() {
        FreebetAutoRolloverStatus::AwaitingFunding
    } else if rollover
        .map(|item| item.status == FreebetProgressStatus::InProgress)
        .unwrap_or(false)
    {
        FreebetAutoRolloverStatus::Monitoring
    } else if plan.is_some() || bonus.is_some() {
        FreebetAutoRolloverStatus::AwaitingTrigger
    } else {
        FreebetAutoRolloverStatus::DraftOnly
    };

    let trigger = match status {
        FreebetAutoRolloverStatus::Completed => "rollover already completed".to_string(),
        FreebetAutoRolloverStatus::AwaitingFunding => {
            "funding gaps must be closed before rollover draft can start".to_string()
        }
        FreebetAutoRolloverStatus::AwaitingTrigger => {
            "wait for manual qualifying/freebet placement; draft stays in no-op mode".to_string()
        }
        FreebetAutoRolloverStatus::Monitoring => {
            "track wagering progress updates without placing real bets".to_string()
        }
        FreebetAutoRolloverStatus::DraftOnly => {
            "draft is available for manual review only".to_string()
        }
    };
    let next_action = match status {
        FreebetAutoRolloverStatus::Completed => {
            "No action needed; keep the completed rollover archived for audit.".to_string()
        }
        FreebetAutoRolloverStatus::AwaitingFunding => {
            if let Some((bookmaker, gap)) = largest_gap {
                format!("Top up {bookmaker} by at least {gap:.2} before reviewing the draft again.")
            } else {
                "Close the remaining funding gaps before reviewing the draft again.".to_string()
            }
        }
        FreebetAutoRolloverStatus::AwaitingTrigger => {
            "Place the qualifying/freebet legs manually, then refresh lifecycle tracking."
                .to_string()
        }
        FreebetAutoRolloverStatus::Monitoring => {
            "Track turnover updates until the remaining rollover requirement reaches zero."
                .to_string()
        }
        FreebetAutoRolloverStatus::DraftOnly => {
            "Review the draft and wait for a qualifying trigger before taking action.".to_string()
        }
    };
    let read_only_check = match status {
        FreebetAutoRolloverStatus::Completed => {
            "Re-open the lifecycle snapshot only for audit; the draft should remain completed."
                .to_string()
        }
        FreebetAutoRolloverStatus::AwaitingFunding => {
            "After balances update, refresh lifecycle tracking and confirm the draft leaves awaiting_funding."
                .to_string()
        }
        FreebetAutoRolloverStatus::AwaitingTrigger => {
            "After the manual legs settle, refresh lifecycle tracking and confirm the draft enters monitoring."
                .to_string()
        }
        FreebetAutoRolloverStatus::Monitoring => {
            "Refresh lifecycle tracking after each turnover sync and confirm the remaining requirement keeps falling."
                .to_string()
        }
        FreebetAutoRolloverStatus::DraftOnly => {
            "Refresh lifecycle tracking after a manual trigger appears; the draft should stay read-only."
                .to_string()
        }
    };

    let mut notes = vec![format!(
        "safe auto-rollover remains draft-only for {bookmaker}; real execution is disabled"
    )];
    if let Some(item) = rollover {
        notes.push(format!(
            "turnover progress {:.2}/{:.2} ({:.1}%)",
            item.completed_turnover, item.required_turnover, item.progress_percent
        ));
    }
    if funding_gap_by_bookmaker.is_empty() {
        notes.push("funding payload is already covered by available balances".into());
    } else {
        notes.push(format!(
            "manual top-up still required across {} bookmaker(s): {:.2} total",
            funding_readiness.blocking_bookmakers.len(),
            funding_readiness.total_gap
        ));
        notes.push("funding payload still needs manual top-up before any rollover workflow".into());
    }

    Some(FreebetAutoRolloverDraft {
        status,
        safe_mode: true,
        execution_allowed: false,
        required_cash_by_bookmaker,
        funding_gap_by_bookmaker,
        funding_readiness,
        funding_recommendation: plan
            .map(|item| item.funding_recommendation.clone())
            .unwrap_or_else(|| "no funding payload available yet".into()),
        trigger,
        next_action,
        read_only_check,
        notes,
    })
}

pub fn collect_freebet_lifecycle(
    opportunities: Vec<FreebetOpportunity>,
    bonus_hunter: &BonusHunter,
    bankroll_manager: &BankrollManager,
) -> Vec<FreebetLifecycleState> {
    let freebet_bonuses = bonus_hunter.get_active_freebet_bonuses();
    let bonus_plans = bonus_hunter.get_all_bonus_plans();
    let bankroll_state = bankroll_manager.get_state();
    let deposit_guidance = bankroll_manager.get_deposit_allocation_guidance();

    let mut best_opportunities: HashMap<String, FreebetOpportunity> = HashMap::new();
    for opportunity in opportunities {
        best_opportunities
            .entry(opportunity.bookmaker.clone())
            .and_modify(|current| {
                if opportunity.guaranteed_profit > current.guaranteed_profit {
                    *current = opportunity.clone();
                }
            })
            .or_insert(opportunity);
    }

    let bonus_by_bookmaker: HashMap<String, BonusInfo> = freebet_bonuses
        .into_iter()
        .map(|bonus| (bonus.bookmaker.clone(), bonus))
        .collect();
    let bonus_plans_by_bookmaker: HashMap<String, shared::BonusPlan> = bonus_plans
        .into_iter()
        .map(|plan| (plan.bookmaker.clone(), plan))
        .collect();
    let balances_by_bookmaker: HashMap<String, BookmakerBalance> = bankroll_state
        .bookmakers
        .into_iter()
        .map(|balance| (balance.bookmaker.clone(), balance))
        .collect();
    let deposits_by_bookmaker: HashMap<String, shared::DepositAllocationTarget> = deposit_guidance
        .targets
        .into_iter()
        .map(|target| (target.bookmaker.clone(), target))
        .collect();

    let mut bookmakers: Vec<String> = best_opportunities.keys().cloned().collect();
    for bookmaker in bonus_by_bookmaker.keys() {
        if !bookmakers.iter().any(|item| item == bookmaker) {
            bookmakers.push(bookmaker.clone());
        }
    }
    for bookmaker in bonus_plans_by_bookmaker.keys() {
        if !bookmakers.iter().any(|item| item == bookmaker) {
            bookmakers.push(bookmaker.clone());
        }
    }

    let updated_at = Utc::now();
    let mut states = Vec::new();
    for bookmaker in bookmakers {
        let opportunity = best_opportunities.get(&bookmaker).cloned();
        let bonus = bonus_by_bookmaker.get(&bookmaker).cloned();
        let bonus_plan = bonus_plans_by_bookmaker.get(&bookmaker);
        let plan = opportunity
            .as_ref()
            .map(|item| build_recommended_freebet_plan(bonus_hunter, item));
        let rollover = bonus
            .as_ref()
            .and_then(|item| build_rollover_progress(item, bonus_plan));
        let allocation =
            build_freebet_allocation(&bookmaker, &balances_by_bookmaker, &deposits_by_bookmaker);
        let lifecycle_stage = infer_freebet_stage(
            opportunity.as_ref(),
            bonus.as_ref(),
            plan.as_ref(),
            rollover.as_ref(),
        );
        let auto_rollover = build_auto_rollover_draft(
            &bookmaker,
            &lifecycle_stage,
            bonus.as_ref(),
            plan.as_ref(),
            rollover.as_ref(),
            &balances_by_bookmaker,
        );
        let next_milestone = infer_next_milestone(&lifecycle_stage, auto_rollover.as_ref());
        let blocked_by = infer_blocked_by(auto_rollover.as_ref());
        let read_only_follow_up =
            infer_read_only_follow_up(&lifecycle_stage, auto_rollover.as_ref());
        let read_only_focus = infer_read_only_focus(&lifecycle_stage, auto_rollover.as_ref());
        let rollover_actions =
            build_staged_rollover_actions(&lifecycle_stage, auto_rollover.as_ref());

        states.push(FreebetLifecycleState {
            bookmaker,
            lifecycle_stage,
            next_milestone,
            blocked_by,
            read_only_follow_up,
            read_only_focus,
            opportunity,
            bonus,
            plan,
            rollover,
            allocation,
            auto_rollover,
            rollover_actions,
            execution_readiness: None,
            updated_at,
        });
    }

    states.sort_by(|a, b| a.bookmaker.cmp(&b.bookmaker));
    states
}

pub async fn persist_freebet_lifecycle_states(
    store: &FreebetLifecycleStore,
    states: &[FreebetLifecycleState],
) -> Result<()> {
    for state in states {
        store.save_state(state).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::{
        BonusConfig, BonusDifficulty, BonusStatus, BonusType, Event, FreebetOpportunity, Sport,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_opportunity(bookmaker: &str) -> FreebetOpportunity {
        FreebetOpportunity {
            id: Uuid::new_v4(),
            bookmaker: bookmaker.into(),
            hedge_bookmaker: "fonbet".into(),
            event: Event {
                id: "evt-1".into(),
                sport: Sport::Football,
                league: "Test League".into(),
                home_team: "A".into(),
                away_team: "B".into(),
                start_time: None,
                is_live: false,
                bookmaker_slug: bookmaker.into(),
                raw_url: None,
                extra: HashMap::new(),
            },
            market: "1X2".into(),
            selection: "1".into(),
            hedge_selection: "X2".into(),
            back_odds: 4.2,
            lay_odds: 1.9,
            freebet_amount: 1_000.0,
            guaranteed_profit: 620.0,
            roi: 62.0,
            detected_at: Utc::now(),
        }
    }

    #[test]
    fn collects_lifecycle_state_from_current_inputs() {
        let bonus_hunter = BonusHunter::new(BonusConfig::default());
        let bankroll_manager = BankrollManager::new(shared::BankrollConfig::default());
        bankroll_manager.update_balance("pari", 5_000.0, 500.0);

        bonus_hunter.add_bonus(BonusInfo {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            bonus_type: BonusType::Freebet,
            name: "Freebet".into(),
            amount: 1_000.0,
            currency: "RUB".into(),
            wager_requirement: 1.0,
            min_odds: 1.5,
            max_bet: 0.0,
            expiry_days: 7,
            real_value: 700.0,
            ev: 650.0,
            difficulty: BonusDifficulty::Easy,
            status: BonusStatus::Claimed,
            wager_progress: 0.0,
            detected_at: Utc::now(),
            url: None,
        });

        let states = collect_freebet_lifecycle(
            vec![make_opportunity("pari")],
            &bonus_hunter,
            &bankroll_manager,
        );

        assert_eq!(states.len(), 1);
        assert_eq!(states[0].bookmaker, "pari");
        assert_eq!(states[0].lifecycle_stage, FreebetLifecycleStage::Planned);
        assert_eq!(states[0].next_milestone, "close_funding_gap");
        assert_eq!(states[0].blocked_by, vec!["funding:fonbet"]);
        assert!(states[0]
            .read_only_follow_up
            .contains("confirm the draft leaves awaiting_funding"));
        assert_eq!(states[0].read_only_focus, "balance_refresh");
        assert!(states[0].plan.is_some());
        assert!(states[0].allocation.is_some());
        assert!(states[0].auto_rollover.is_some());

        let plan = states[0].plan.as_ref().expect("plan should exist");
        assert!((plan.required_cash_by_bookmaker["pari"] - 238.09523809523807).abs() < 1e-9);
        assert!((plan.required_cash_by_bookmaker["fonbet"] - 1_684.2105263157896).abs() < 1e-9);
        assert!(plan.funding_recommendation.contains("pari: 238.10"));
        assert!(plan.funding_recommendation.contains("fonbet: 1684.21"));

        let auto_rollover = states[0]
            .auto_rollover
            .as_ref()
            .expect("auto-rollover draft should exist");
        assert_eq!(
            auto_rollover.status,
            FreebetAutoRolloverStatus::AwaitingFunding
        );
        assert!(auto_rollover.safe_mode);
        assert!(!auto_rollover.execution_allowed);
        assert!(!auto_rollover.funding_readiness.ready);
        assert_eq!(
            auto_rollover.funding_readiness.blocking_bookmakers,
            vec!["fonbet"]
        );
        assert_eq!(
            auto_rollover
                .funding_readiness
                .largest_gap_bookmaker
                .as_deref(),
            Some("fonbet")
        );
        assert!(
            (auto_rollover.required_cash_by_bookmaker["pari"] - 238.09523809523807).abs() < 1e-9
        );
        assert!(
            (auto_rollover.funding_gap_by_bookmaker["fonbet"] - 1_684.2105263157896).abs() < 1e-9
        );
        assert!((auto_rollover.funding_readiness.total_gap - 1_684.2105263157896).abs() < 1e-9);
        assert!(auto_rollover
            .trigger
            .contains("funding gaps must be closed"));
        assert!(auto_rollover.next_action.contains("Top up fonbet"));
        assert!(auto_rollover
            .read_only_check
            .contains("confirm the draft leaves awaiting_funding"));
        assert_eq!(states[0].rollover_actions.len(), 4);
        assert_eq!(states[0].rollover_actions[0].key, "funding_check");
        assert_eq!(
            states[0].rollover_actions[0].status,
            FreebetLifecycleActionStatus::Ready
        );
        assert_eq!(states[0].rollover_actions[1].key, "manual_trigger");
        assert_eq!(
            states[0].rollover_actions[1].status,
            FreebetLifecycleActionStatus::Pending
        );
        assert!(states[0].execution_readiness.is_none());
    }

    #[test]
    fn collects_monitoring_draft_when_rollover_is_already_in_progress() {
        let bonus_hunter = BonusHunter::new(BonusConfig::default());
        let bankroll_manager = BankrollManager::new(shared::BankrollConfig::default());
        bankroll_manager.update_balance("pari", 5_000.0, 0.0);
        bankroll_manager.update_balance("fonbet", 5_000.0, 0.0);

        bonus_hunter.add_bonus(BonusInfo {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            bonus_type: BonusType::Freebet,
            name: "Freebet".into(),
            amount: 1_000.0,
            currency: "RUB".into(),
            wager_requirement: 3.0,
            min_odds: 1.5,
            max_bet: 0.0,
            expiry_days: 7,
            real_value: 700.0,
            ev: 650.0,
            difficulty: BonusDifficulty::Easy,
            status: BonusStatus::Wagering,
            wager_progress: 1_500.0,
            detected_at: Utc::now(),
            url: None,
        });

        let states = collect_freebet_lifecycle(
            vec![make_opportunity("pari")],
            &bonus_hunter,
            &bankroll_manager,
        );

        let auto_rollover = states[0]
            .auto_rollover
            .as_ref()
            .expect("auto-rollover draft should exist");
        assert_eq!(
            states[0].lifecycle_stage,
            FreebetLifecycleStage::RolloverInProgress
        );
        assert_eq!(states[0].next_milestone, "complete_rollover");
        assert!(states[0].blocked_by.is_empty());
        assert!(states[0]
            .read_only_follow_up
            .contains("remaining requirement keeps falling"));
        assert_eq!(states[0].read_only_focus, "turnover_progress");
        assert_eq!(auto_rollover.status, FreebetAutoRolloverStatus::Monitoring);
        assert!(auto_rollover.funding_gap_by_bookmaker.is_empty());
        assert!(auto_rollover.funding_readiness.ready);
        assert_eq!(auto_rollover.funding_readiness.total_gap, 0.0);
        assert!(auto_rollover
            .trigger
            .contains("track wagering progress updates"));
        assert!(auto_rollover
            .next_action
            .contains("remaining rollover requirement reaches zero"));
        assert!(auto_rollover
            .read_only_check
            .contains("remaining requirement keeps falling"));
        assert_eq!(states[0].rollover_actions.len(), 4);
        assert_eq!(states[0].rollover_actions[2].key, "turnover_monitoring");
        assert_eq!(
            states[0].rollover_actions[2].status,
            FreebetLifecycleActionStatus::Monitoring
        );
    }

    #[tokio::test]
    async fn persists_lifecycle_states_to_store() {
        let store = FreebetLifecycleStore::new("memory").await.unwrap();
        let state = FreebetLifecycleState {
            bookmaker: "pari".into(),
            lifecycle_stage: FreebetLifecycleStage::Planned,
            next_milestone: "place_manual_legs".into(),
            blocked_by: vec!["manual_trigger".into()],
            read_only_follow_up:
                "Refresh lifecycle after manual placement and confirm the draft is still aligned."
                    .into(),
            read_only_focus: "manual_placement".into(),
            opportunity: Some(make_opportunity("pari")),
            bonus: None,
            plan: None,
            rollover: None,
            allocation: None,
            auto_rollover: None,
            rollover_actions: Vec::new(),
            execution_readiness: None,
            updated_at: Utc::now(),
        };

        persist_freebet_lifecycle_states(&store, &[state])
            .await
            .unwrap();

        assert_eq!(store.count().await, 1);
        assert_eq!(
            store.get_state("pari").await.unwrap().lifecycle_stage,
            FreebetLifecycleStage::Planned
        );
    }
}
