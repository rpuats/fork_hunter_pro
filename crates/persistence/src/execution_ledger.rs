use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Error, Result};
use auto_betting::{
    ExecutionLedgerEntry, ExecutionLedgerPersistence, ExecutionStateMachine, ExecutionStateReplay,
};
use shared::{BetPlacement, BetStatus, ExecutionPlacementSummary};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

pub struct ExecutionLedgerStore {
    pool: Option<Arc<SqlitePool>>,
    data: Arc<Mutex<Vec<ExecutionLedgerEntry>>>,
    tx: Option<mpsc::UnboundedSender<LedgerCommand>>,
}

enum LedgerCommand {
    Record(ExecutionLedgerEntry),
    Flush(oneshot::Sender<()>),
}

impl ExecutionLedgerStore {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url.is_empty() || database_url == "memory" {
            info!("Using in-memory execution ledger storage");
            return Ok(Self {
                pool: None,
                data: Arc::new(Mutex::new(Vec::new())),
                tx: None,
            });
        }

        let connect_options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        match SqlitePool::connect_with(connect_options).await {
            Ok(pool) => {
                info!(
                    url = database_url,
                    "Connected to SQLite for execution ledger"
                );
                let pool = Arc::new(pool);
                let store = Self {
                    pool: Some(pool.clone()),
                    data: Arc::new(Mutex::new(Vec::new())),
                    tx: None,
                };
                store.migrate().await?;
                store.preload().await?;

                let (tx, rx) = mpsc::unbounded_channel();
                store.spawn_writer(pool, rx);

                Ok(Self {
                    tx: Some(tx),
                    ..store
                })
            }
            Err(error) => {
                warn!(error = %error, "Failed to connect to SQLite for execution ledger, falling back to in-memory");
                Ok(Self {
                    pool: None,
                    data: Arc::new(Mutex::new(Vec::new())),
                    tx: None,
                })
            }
        }
    }

    async fn migrate(&self) -> Result<(), Error> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS execution_ledger (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    placement_id TEXT NOT NULL,
                    bookmaker TEXT NOT NULL,
                    action TEXT NOT NULL,
                    status TEXT NOT NULL,
                    placed_at INTEGER NOT NULL,
                    recorded_at INTEGER NOT NULL,
                    payload_json TEXT NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            info!("Execution ledger migration complete");
        }

        Ok(())
    }

    async fn preload(&self) -> Result<(), Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };

        let rows = sqlx::query(
            r#"
            SELECT payload_json
            FROM execution_ledger
            ORDER BY id ASC
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut data = self.data.lock().unwrap();
        data.clear();

        for row in rows {
            let payload: String = row.try_get("payload_json")?;
            match serde_json::from_str::<ExecutionLedgerEntry>(&payload) {
                Ok(entry) => data.push(entry),
                Err(error) => {
                    warn!(error = %error, "Failed to deserialize persisted execution ledger entry")
                }
            }
        }

        Ok(())
    }

    fn spawn_writer(&self, pool: Arc<SqlitePool>, mut rx: mpsc::UnboundedReceiver<LedgerCommand>) {
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    LedgerCommand::Record(entry) => {
                        if let Err(error) = persist_entry(&pool, &entry).await {
                            warn!(error = %error, placement_id = %entry.placement.id, "Failed to persist execution ledger entry");
                        }
                    }
                    LedgerCommand::Flush(done) => {
                        let _ = done.send(());
                    }
                }
            }
        });
    }

    pub async fn get_recent(&self, limit: usize) -> Result<Vec<ExecutionLedgerEntry>, Error> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().rev().take(limit).cloned().collect())
    }

    pub async fn count(&self) -> Result<usize, Error> {
        Ok(self.data.lock().unwrap().len())
    }

    pub async fn get_recent_placements(&self, limit: usize) -> Result<Vec<BetPlacement>, Error> {
        let data = self.data.lock().unwrap();
        let mut seen = HashSet::new();
        let mut placements = Vec::new();

        for entry in data.iter().rev() {
            if seen.insert(entry.placement.id) {
                placements.push(entry.placement.clone());
            }

            if placements.len() >= limit {
                break;
            }
        }

        Ok(placements)
    }

    pub async fn summarize_latest_placements(&self) -> Result<ExecutionPlacementSummary, Error> {
        let data = self.data.lock().unwrap();
        let mut latest = HashMap::new();

        for entry in data.iter() {
            latest.insert(entry.placement.id, entry.placement.status.clone());
        }

        let mut summary = ExecutionPlacementSummary {
            total: latest.len(),
            pending: 0,
            placed: 0,
            settled: 0,
            cancelled: 0,
            errors: 0,
        };

        for status in latest.into_values() {
            match status {
                BetStatus::Pending => summary.pending += 1,
                BetStatus::Placed => summary.placed += 1,
                BetStatus::Settled => summary.settled += 1,
                BetStatus::Cancelled => summary.cancelled += 1,
                BetStatus::Error => summary.errors += 1,
            }
        }

        Ok(summary)
    }

    pub async fn replay_state_machine(&self) -> Result<ExecutionStateReplay, Error> {
        let data = self.data.lock().unwrap();
        ExecutionStateMachine::replay(data.iter()).map_err(Error::msg)
    }

    pub async fn flush(&self) -> Result<(), Error> {
        let Some(tx) = &self.tx else {
            return Ok(());
        };

        let (done_tx, done_rx) = oneshot::channel();
        tx.send(LedgerCommand::Flush(done_tx))
            .map_err(|_| Error::msg("execution ledger writer is unavailable"))?;
        done_rx
            .await
            .map_err(|_| Error::msg("execution ledger flush was interrupted"))?;
        Ok(())
    }
}

impl ExecutionLedgerPersistence for ExecutionLedgerStore {
    fn record(&self, entry: ExecutionLedgerEntry) {
        self.data.lock().unwrap().push(entry.clone());
        let persistable = entry.clone();

        if let Some(tx) = &self.tx {
            let _ = tx.send(LedgerCommand::Record(persistable));
        }
    }
}

async fn persist_entry(pool: &SqlitePool, entry: &ExecutionLedgerEntry) -> Result<(), sqlx::Error> {
    let payload_json =
        serde_json::to_string(entry).map_err(|error| sqlx::Error::Protocol(error.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO execution_ledger (
            placement_id,
            bookmaker,
            action,
            status,
            placed_at,
            recorded_at,
            payload_json
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(entry.placement.id.to_string())
    .bind(&entry.placement.bookmaker)
    .bind(format!("{:?}", entry.action))
    .bind(format!("{:?}", entry.placement.status))
    .bind(entry.placement.placed_at.timestamp())
    .bind(entry.recorded_at.timestamp())
    .bind(payload_json)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_betting::ExecutionLedgerAction;
    use chrono::Utc;
    use shared::{
        BetExecutionReceipt, BetExecutionStatus, BetPlacement, BetResult, BetStatus,
        BookmakerExecutionMode, Event, Sport,
    };
    use uuid::Uuid;

    fn make_entry() -> ExecutionLedgerEntry {
        ExecutionLedgerEntry {
            placement: BetPlacement {
                id: Uuid::new_v4(),
                bookmaker: "pari".into(),
                event: Event {
                    id: "event-1".into(),
                    sport: Sport::Football,
                    league: "Test League".into(),
                    home_team: "Team A".into(),
                    away_team: "Team B".into(),
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
                status: BetStatus::Placed,
                placed_at: Utc::now(),
                execution: Some(BetExecutionReceipt {
                    ticket_id: Some("ticket-1".into()),
                    account_id: None,
                    bookmaker: "pari".into(),
                    status: BetExecutionStatus::Accepted,
                    mode: BookmakerExecutionMode::DryRun,
                    accepted_stake: 500.0,
                    accepted_odds: 2.1,
                    message: None,
                    placed_at: Utc::now(),
                }),
                result: Some(BetResult::Won(1050.0)),
                error: None,
            },
            action: ExecutionLedgerAction::Placed,
            recorded_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn stores_execution_ledger_entries_in_memory() {
        let store = ExecutionLedgerStore::new("memory").await.unwrap();
        let entry = make_entry();

        store.record(entry.clone());
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        assert_eq!(store.count().await.unwrap(), 1);
        let recent = store.get_recent(1).await.unwrap();
        assert_eq!(recent[0].placement.id, entry.placement.id);
        assert_eq!(recent[0].action, ExecutionLedgerAction::Placed);
    }

    #[tokio::test]
    async fn reloads_persisted_execution_ledger_entries_from_sqlite() {
        let db_path = std::env::temp_dir().join(format!("execution-ledger-{}.db", Uuid::new_v4()));
        let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

        let store = ExecutionLedgerStore::new(&db_url).await.unwrap();
        let entry = make_entry();
        store.record(entry.clone());
        store.flush().await.unwrap();
        drop(store);

        let reloaded = ExecutionLedgerStore::new(&db_url).await.unwrap();
        let recent = reloaded.get_recent(1).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].placement.id, entry.placement.id);

        let _ = tokio::fs::remove_file(db_path).await;
    }

    #[tokio::test]
    async fn get_recent_placements_returns_latest_state_per_bet() {
        let store = ExecutionLedgerStore::new("memory").await.unwrap();
        let mut entry = make_entry();
        let placement_id = entry.placement.id;

        store.record(entry.clone());

        entry.action = ExecutionLedgerAction::Updated;
        entry.placement.status = BetStatus::Settled;
        store.record(entry.clone());

        let placements = store.get_recent_placements(10).await.unwrap();
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].id, placement_id);
        assert_eq!(placements[0].status, BetStatus::Settled);
    }

    #[tokio::test]
    async fn summarize_latest_placements_uses_latest_status_per_bet() {
        let store = ExecutionLedgerStore::new("memory").await.unwrap();
        let mut settled = make_entry();
        let mut cancelled = make_entry();

        settled.placement.status = BetStatus::Placed;
        cancelled.placement.status = BetStatus::Pending;

        store.record(settled.clone());
        settled.action = ExecutionLedgerAction::Updated;
        settled.placement.status = BetStatus::Settled;
        store.record(settled);

        store.record(cancelled.clone());
        cancelled.action = ExecutionLedgerAction::Updated;
        cancelled.placement.status = BetStatus::Cancelled;
        store.record(cancelled);

        let summary = store.summarize_latest_placements().await.unwrap();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.pending, 0);
        assert_eq!(summary.placed, 0);
        assert_eq!(summary.settled, 1);
        assert_eq!(summary.cancelled, 1);
        assert_eq!(summary.errors, 0);
    }

    #[tokio::test]
    async fn replays_execution_state_machine_from_ledger_entries() {
        let store = ExecutionLedgerStore::new("memory").await.unwrap();
        let mut entry = make_entry();

        entry.placement.status = BetStatus::Pending;
        store.record(entry.clone());

        entry.action = ExecutionLedgerAction::Updated;
        entry.placement.status = BetStatus::Settled;
        store.record(entry);

        let replay = store.replay_state_machine().await.unwrap();
        assert_eq!(replay.transitions.len(), 2);
        assert_eq!(replay.snapshots.len(), 1);
        assert_eq!(replay.snapshots[0].sequence, 2);
    }
}
