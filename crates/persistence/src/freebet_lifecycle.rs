use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Error, Result};
use shared::FreebetLifecycleState;
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::str::FromStr;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct FreebetLifecycleStore {
    pool: Option<Arc<SqlitePool>>,
    data: Arc<Mutex<HashMap<String, FreebetLifecycleState>>>,
}

impl FreebetLifecycleStore {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url.is_empty() || database_url == "memory" {
            info!("Using in-memory freebet lifecycle storage");
            return Ok(Self {
                pool: None,
                data: Arc::new(Mutex::new(HashMap::new())),
            });
        }

        let connect_options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        match SqlitePool::connect_with(connect_options).await {
            Ok(pool) => {
                info!(
                    url = database_url,
                    "Connected to SQLite for freebet lifecycle"
                );
                let store = Self {
                    pool: Some(Arc::new(pool)),
                    data: Arc::new(Mutex::new(HashMap::new())),
                };
                store.migrate().await?;
                store.preload().await?;
                Ok(store)
            }
            Err(error) => {
                warn!(error = %error, "Failed to connect to SQLite for freebet lifecycle, falling back to in-memory");
                Ok(Self {
                    pool: None,
                    data: Arc::new(Mutex::new(HashMap::new())),
                })
            }
        }
    }

    async fn migrate(&self) -> Result<(), Error> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS freebet_lifecycle_state (
                    bookmaker TEXT PRIMARY KEY,
                    state_json TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            info!("Freebet lifecycle state migration complete");
        }

        Ok(())
    }

    async fn preload(&self) -> Result<(), Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };

        let rows = sqlx::query(
            r#"
            SELECT bookmaker, state_json
            FROM freebet_lifecycle_state
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut data = self.data.lock().await;
        data.clear();

        for row in rows {
            let bookmaker: String = row.try_get("bookmaker")?;
            let raw: String = row.try_get("state_json")?;

            match serde_json::from_str::<FreebetLifecycleState>(&raw) {
                Ok(state) => {
                    data.insert(bookmaker, state);
                }
                Err(error) => {
                    warn!(bookmaker, error = %error, "Failed to deserialize persisted freebet lifecycle state");
                }
            }
        }

        Ok(())
    }

    pub async fn save_state(&self, state: &FreebetLifecycleState) -> Result<(), Error> {
        {
            let mut data = self.data.lock().await;
            data.insert(state.bookmaker.clone(), state.clone());
        }

        if let Some(pool) = &self.pool {
            let state_json = serde_json::to_string(state)?;

            sqlx::query(
                r#"
                INSERT INTO freebet_lifecycle_state (bookmaker, state_json, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(bookmaker) DO UPDATE SET
                    state_json = excluded.state_json,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&state.bookmaker)
            .bind(state_json)
            .bind(state.updated_at.timestamp())
            .execute(pool.as_ref())
            .await?;
        }

        Ok(())
    }

    pub async fn get_state(&self, bookmaker: &str) -> Option<FreebetLifecycleState> {
        let data = self.data.lock().await;
        data.get(bookmaker).cloned()
    }

    pub async fn list_states(&self) -> Vec<FreebetLifecycleState> {
        let data = self.data.lock().await;
        let mut states: Vec<_> = data.values().cloned().collect();
        states.sort_by(|a, b| a.bookmaker.cmp(&b.bookmaker));
        states
    }

    pub async fn count(&self) -> usize {
        let data = self.data.lock().await;
        data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::{
        FreebetAutoRolloverDraft, FreebetAutoRolloverStatus, FreebetConversionPlan,
        FreebetFundingReadiness, FreebetHedgeLeg, FreebetLifecycleStage, FreebetPlanStep,
        FreebetStepType,
    };
    use uuid::Uuid;

    fn make_state(bookmaker: &str, stage: FreebetLifecycleStage) -> FreebetLifecycleState {
        FreebetLifecycleState {
            bookmaker: bookmaker.into(),
            lifecycle_stage: stage,
            next_milestone: "close_funding_gap".into(),
            blocked_by: vec!["funding:fonbet".into()],
            read_only_follow_up:
                "After balances update, refresh lifecycle tracking and confirm the draft leaves awaiting_funding."
                    .into(),
            read_only_focus: "balance_refresh".into(),
            opportunity: None,
            bonus: None,
            plan: Some(FreebetConversionPlan {
                id: Uuid::new_v4(),
                bookmaker: bookmaker.into(),
                freebet_amount: 1_000.0,
                qualifying_cost: 50.0,
                conversion_rate: 0.7,
                estimated_profit: 650.0,
                required_cash_by_bookmaker: HashMap::from([
                    (bookmaker.into(), 500.0),
                    ("fonbet".into(), 540.0),
                ]),
                funding_recommendation:
                    "Keep cash ready before starting the sequence: fonbet: 540.00, pari: 500.00. The 1000.00 freebet itself is placed at pari without extra cash stake.".into(),
                hedge: FreebetHedgeLeg {
                    bookmaker: "fonbet".into(),
                    market: "1X2".into(),
                    selection: "X2".into(),
                    odds: 1.85,
                    stake: 540.0,
                },
                steps: vec![
                    FreebetPlanStep {
                        step_number: 1,
                        step_type: FreebetStepType::QualifyingBet,
                        bookmaker: bookmaker.into(),
                        market: "1X2".into(),
                        selection: "1".into(),
                        odds: 2.0,
                        stake: 500.0,
                        note: "Qualify".into(),
                    },
                    FreebetPlanStep {
                        step_number: 2,
                        step_type: FreebetStepType::FreebetBet,
                        bookmaker: bookmaker.into(),
                        market: "1X2".into(),
                        selection: "X".into(),
                        odds: 4.2,
                        stake: 1_000.0,
                        note: "Convert".into(),
                    },
                ],
                created_at: Utc::now(),
            }),
            rollover: None,
            allocation: None,
            auto_rollover: Some(FreebetAutoRolloverDraft {
                status: FreebetAutoRolloverStatus::AwaitingFunding,
                safe_mode: true,
                execution_allowed: false,
                required_cash_by_bookmaker: HashMap::from([
                    (bookmaker.into(), 500.0),
                    ("fonbet".into(), 540.0),
                ]),
                funding_gap_by_bookmaker: HashMap::from([("fonbet".into(), 540.0)]),
                funding_readiness: FreebetFundingReadiness {
                    ready: false,
                    total_gap: 540.0,
                    blocking_bookmakers: vec!["fonbet".into()],
                    largest_gap_bookmaker: Some("fonbet".into()),
                    largest_gap_amount: Some(540.0),
                },
                funding_recommendation:
                    "Keep cash ready before starting the sequence: fonbet: 540.00, pari: 500.00. The 1000.00 freebet itself is placed at pari without extra cash stake.".into(),
                trigger: "funding gaps must be closed before rollover draft can start".into(),
                next_action: "Top up fonbet by at least 540.00 before reviewing the draft again."
                    .into(),
                read_only_check:
                    "After balances update, refresh lifecycle tracking and confirm the draft leaves awaiting_funding."
                        .into(),
                notes: vec![
                    "safe auto-rollover remains draft-only for pari; real execution is disabled"
                        .into(),
                ],
            }),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn stores_and_reads_freebet_lifecycle_in_memory() {
        let store = FreebetLifecycleStore::new("memory").await.unwrap();
        let state = make_state("pari", FreebetLifecycleStage::Planned);

        store.save_state(&state).await.unwrap();

        let loaded = store.get_state("pari").await.unwrap();
        assert_eq!(loaded.lifecycle_stage, FreebetLifecycleStage::Planned);
        assert_eq!(loaded.next_milestone, "close_funding_gap");
        assert_eq!(loaded.blocked_by, vec!["funding:fonbet"]);
        assert!(loaded
            .read_only_follow_up
            .contains("confirm the draft leaves awaiting_funding"));
        assert_eq!(loaded.read_only_focus, "balance_refresh");
        assert_eq!(loaded.plan.unwrap().bookmaker, "pari");
        let auto_rollover = loaded.auto_rollover.expect("auto-rollover draft");
        assert_eq!(
            auto_rollover.status,
            FreebetAutoRolloverStatus::AwaitingFunding
        );
        assert!(!auto_rollover.funding_readiness.ready);
        assert_eq!(auto_rollover.funding_readiness.total_gap, 540.0);
        assert_eq!(
            auto_rollover
                .funding_readiness
                .largest_gap_bookmaker
                .as_deref(),
            Some("fonbet")
        );
        assert!(auto_rollover
            .read_only_check
            .contains("confirm the draft leaves awaiting_funding"));
        assert_eq!(store.count().await, 1);
    }

    #[tokio::test]
    async fn overwrites_existing_bookmaker_state() {
        let store = FreebetLifecycleStore::new("memory").await.unwrap();

        store
            .save_state(&make_state("pari", FreebetLifecycleStage::Planned))
            .await
            .unwrap();
        store
            .save_state(&make_state(
                "pari",
                FreebetLifecycleStage::RolloverCompleted,
            ))
            .await
            .unwrap();

        let states = store.list_states().await;
        assert_eq!(states.len(), 1);
        assert_eq!(
            states[0].lifecycle_stage,
            FreebetLifecycleStage::RolloverCompleted
        );
    }
}
