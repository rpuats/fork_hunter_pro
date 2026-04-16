use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use shared::BetStatus;
use uuid::Uuid;

use crate::persistence::{ExecutionLedgerAction, ExecutionLedgerEntry};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ExecutionStatePhase {
    PendingPlacement,
    ConfirmedPlacement,
    Settled,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExecutionStateSnapshot {
    pub placement_id: Uuid,
    pub bookmaker: String,
    pub phase: ExecutionStatePhase,
    pub placement_status: BetStatus,
    pub sequence: u64,
    pub updated_at: DateTime<Utc>,
    pub last_action: ExecutionLedgerAction,
    pub last_error: Option<String>,
}

impl Eq for ExecutionStateSnapshot {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExecutionStateTransition {
    pub placement_id: Uuid,
    pub bookmaker: String,
    pub from_phase: Option<ExecutionStatePhase>,
    pub to_phase: ExecutionStatePhase,
    pub placement_status: BetStatus,
    pub sequence: u64,
    pub action: ExecutionLedgerAction,
    pub occurred_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl Eq for ExecutionStateTransition {}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExecutionStateReplay {
    pub snapshots: Vec<ExecutionStateSnapshot>,
    pub transitions: Vec<ExecutionStateTransition>,
}

impl Eq for ExecutionStateReplay {}

pub struct ExecutionStateMachine;

impl ExecutionStateMachine {
    pub fn snapshot_from_entry(
        previous: Option<&ExecutionStateSnapshot>,
        entry: &ExecutionLedgerEntry,
    ) -> Result<(ExecutionStateSnapshot, ExecutionStateTransition), String> {
        let next_phase = phase_from_status(&entry.placement.status);
        let from_phase = previous.map(|snapshot| snapshot.phase);

        validate_transition(from_phase, next_phase)?;

        let sequence = previous.map_or(1, |snapshot| snapshot.sequence + 1);
        let snapshot = ExecutionStateSnapshot {
            placement_id: entry.placement.id,
            bookmaker: entry.placement.bookmaker.clone(),
            phase: next_phase,
            placement_status: entry.placement.status.clone(),
            sequence,
            updated_at: entry.recorded_at,
            last_action: entry.action.clone(),
            last_error: entry.placement.error.clone(),
        };
        let transition = ExecutionStateTransition {
            placement_id: entry.placement.id,
            bookmaker: entry.placement.bookmaker.clone(),
            from_phase,
            to_phase: next_phase,
            placement_status: entry.placement.status.clone(),
            sequence,
            action: entry.action.clone(),
            occurred_at: entry.recorded_at,
            error: entry.placement.error.clone(),
        };

        Ok((snapshot, transition))
    }

    pub fn replay<'a>(
        entries: impl IntoIterator<Item = &'a ExecutionLedgerEntry>,
    ) -> Result<ExecutionStateReplay, String> {
        let mut latest = BTreeMap::new();
        let mut transitions = Vec::new();

        for entry in entries {
            let previous = latest.get(&entry.placement.id);
            let (snapshot, transition) = Self::snapshot_from_entry(previous, entry)?;
            latest.insert(entry.placement.id, snapshot);
            transitions.push(transition);
        }

        Ok(ExecutionStateReplay {
            snapshots: latest.into_values().collect(),
            transitions,
        })
    }
}

fn phase_from_status(status: &BetStatus) -> ExecutionStatePhase {
    match status {
        BetStatus::Pending => ExecutionStatePhase::PendingPlacement,
        BetStatus::Placed => ExecutionStatePhase::ConfirmedPlacement,
        BetStatus::Settled => ExecutionStatePhase::Settled,
        BetStatus::Cancelled => ExecutionStatePhase::Cancelled,
        BetStatus::Error => ExecutionStatePhase::Failed,
    }
}

fn validate_transition(
    from_phase: Option<ExecutionStatePhase>,
    to_phase: ExecutionStatePhase,
) -> Result<(), String> {
    let valid = match from_phase {
        None => true,
        Some(current) if current == to_phase => true,
        Some(ExecutionStatePhase::PendingPlacement) => matches!(
            to_phase,
            ExecutionStatePhase::ConfirmedPlacement
                | ExecutionStatePhase::Settled
                | ExecutionStatePhase::Cancelled
                | ExecutionStatePhase::Failed
        ),
        Some(ExecutionStatePhase::ConfirmedPlacement) => matches!(
            to_phase,
            ExecutionStatePhase::Settled
                | ExecutionStatePhase::Cancelled
                | ExecutionStatePhase::Failed
        ),
        Some(ExecutionStatePhase::Settled)
        | Some(ExecutionStatePhase::Cancelled)
        | Some(ExecutionStatePhase::Failed) => false,
    };

    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid execution state transition: {:?} -> {:?}",
            from_phase, to_phase
        ))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use shared::{
        BetExecutionReceipt, BetExecutionStatus, BetPlacement, BetResult, BookmakerExecutionMode,
        Event, Sport,
    };

    use super::*;

    fn assert_eq_trait<T: Eq>() {}

    fn make_entry(
        status: BetStatus,
        action: ExecutionLedgerAction,
        offset_seconds: i64,
    ) -> ExecutionLedgerEntry {
        ExecutionLedgerEntry {
            placement: BetPlacement {
                id: Uuid::nil(),
                bookmaker: "pari".into(),
                event: Event {
                    id: "event-1".into(),
                    sport: Sport::Football,
                    league: "Test League".into(),
                    home_team: "A".into(),
                    away_team: "B".into(),
                    start_time: None,
                    is_live: false,
                    bookmaker_slug: "pari".into(),
                    raw_url: None,
                    extra: Default::default(),
                },
                market: "1X2".into(),
                selection: "1".into(),
                odds: 2.1,
                stake: 500.0,
                status: status.clone(),
                placed_at: Utc::now(),
                execution: Some(BetExecutionReceipt {
                    ticket_id: Some("t-1".into()),
                    account_id: None,
                    bookmaker: "pari".into(),
                    status: BetExecutionStatus::Accepted,
                    mode: BookmakerExecutionMode::DryRun,
                    accepted_stake: 500.0,
                    accepted_odds: 2.1,
                    message: None,
                    placed_at: Utc::now(),
                }),
                result: if status == BetStatus::Settled {
                    Some(BetResult::Won(1050.0))
                } else {
                    None
                },
                error: (status == BetStatus::Error).then(|| "failed".into()),
            },
            action,
            recorded_at: Utc::now() + Duration::seconds(offset_seconds),
        }
    }

    #[test]
    fn replays_pending_to_settled_sequence() {
        let placed = make_entry(BetStatus::Pending, ExecutionLedgerAction::Placed, 0);
        let settled = make_entry(BetStatus::Settled, ExecutionLedgerAction::Updated, 5);

        let replay = ExecutionStateMachine::replay([&placed, &settled]).unwrap();

        assert_eq!(replay.transitions.len(), 2);
        assert_eq!(replay.snapshots.len(), 1);
        assert_eq!(replay.snapshots[0].phase, ExecutionStatePhase::Settled);
        assert_eq!(replay.snapshots[0].sequence, 2);
    }

    #[test]
    fn rejects_invalid_terminal_transition() {
        let failed = make_entry(BetStatus::Error, ExecutionLedgerAction::Placed, 0);
        let settled = make_entry(BetStatus::Settled, ExecutionLedgerAction::Updated, 5);

        let error = ExecutionStateMachine::replay([&failed, &settled]).unwrap_err();

        assert!(error.contains("invalid execution state transition"));
    }

    #[test]
    fn state_machine_types_support_eq() {
        assert_eq_trait::<ExecutionStateSnapshot>();
        assert_eq_trait::<ExecutionStateTransition>();
        assert_eq_trait::<ExecutionStateReplay>();
    }
}
