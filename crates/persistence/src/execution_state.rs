use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Error, Result};
use async_trait::async_trait;
use auto_betting::{ExecutionRegistryPersistence, ExecutionRegistrySnapshot};
use chrono::Utc;
use shared::{BookmakerAccount, BookmakerBalanceSnapshot, BookmakerSession};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
struct PersistedBookmakerState {
    account: Option<BookmakerAccount>,
    session: Option<BookmakerSession>,
    balance: Option<BookmakerBalanceSnapshot>,
}

pub struct ExecutionStateStore {
    pool: Option<Arc<SqlitePool>>,
    data: Arc<Mutex<HashMap<String, PersistedBookmakerState>>>,
}

impl ExecutionStateStore {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url.is_empty() || database_url == "memory" {
            info!("Using in-memory execution state storage");
            return Ok(Self {
                pool: None,
                data: Arc::new(Mutex::new(HashMap::new())),
            });
        }

        match SqlitePool::connect(database_url).await {
            Ok(pool) => {
                info!(
                    url = database_url,
                    "Connected to SQLite for execution state"
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
                warn!(error = %error, "Failed to connect to SQLite for execution state, falling back to in-memory");
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
                CREATE TABLE IF NOT EXISTS execution_registry_state (
                    bookmaker TEXT PRIMARY KEY,
                    account_json TEXT,
                    session_json TEXT,
                    balance_json TEXT,
                    updated_at INTEGER NOT NULL
                )
                "#,
            )
            .execute(pool.as_ref())
            .await?;

            info!("Execution registry state migration complete");
        }

        Ok(())
    }

    async fn preload(&self) -> Result<(), Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };

        let rows = sqlx::query(
            r#"
            SELECT bookmaker, account_json, session_json, balance_json
            FROM execution_registry_state
            "#,
        )
        .fetch_all(pool.as_ref())
        .await?;

        let mut data = self.data.lock().await;
        data.clear();

        for row in rows {
            let bookmaker: String = row.try_get("bookmaker")?;
            let account = parse_json_column::<BookmakerAccount>(&row, "account_json", &bookmaker);
            let session = parse_json_column::<BookmakerSession>(&row, "session_json", &bookmaker);
            let balance =
                parse_json_column::<BookmakerBalanceSnapshot>(&row, "balance_json", &bookmaker);

            data.insert(
                bookmaker,
                PersistedBookmakerState {
                    account,
                    session,
                    balance,
                },
            );
        }

        Ok(())
    }

    async fn save_account_inner(&self, account: &BookmakerAccount) -> Result<(), String> {
        self.update_state(&account.bookmaker, |state| {
            state.account = Some(account.clone());
        })
        .await
    }

    async fn save_session_inner(&self, session: &BookmakerSession) -> Result<(), String> {
        self.update_state(&session.bookmaker, |state| {
            state.session = Some(session.clone());
        })
        .await
    }

    async fn save_balance_inner(&self, snapshot: &BookmakerBalanceSnapshot) -> Result<(), String> {
        self.update_state(&snapshot.bookmaker, |state| {
            state.balance = Some(snapshot.clone());
        })
        .await
    }

    async fn update_state<F>(&self, bookmaker: &str, update: F) -> Result<(), String>
    where
        F: FnOnce(&mut PersistedBookmakerState),
    {
        let persisted = {
            let mut data = self.data.lock().await;
            let state = data.entry(bookmaker.to_string()).or_default();
            update(state);
            state.clone()
        };

        if let Some(pool) = &self.pool {
            let account_json = serialize_optional(&persisted.account)?;
            let session_json = serialize_optional(&persisted.session)?;
            let balance_json = serialize_optional(&persisted.balance)?;

            sqlx::query(
                r#"
                INSERT INTO execution_registry_state (
                    bookmaker,
                    account_json,
                    session_json,
                    balance_json,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(bookmaker) DO UPDATE SET
                    account_json = excluded.account_json,
                    session_json = excluded.session_json,
                    balance_json = excluded.balance_json,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(bookmaker)
            .bind(account_json)
            .bind(session_json)
            .bind(balance_json)
            .bind(Utc::now().timestamp())
            .execute(pool.as_ref())
            .await
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }
}

#[async_trait]
impl ExecutionRegistryPersistence for ExecutionStateStore {
    async fn load_snapshot(&self) -> Result<ExecutionRegistrySnapshot, String> {
        let data = self.data.lock().await;

        Ok(ExecutionRegistrySnapshot {
            accounts: data
                .values()
                .filter_map(|item| item.account.clone())
                .collect(),
            sessions: data
                .values()
                .filter_map(|item| item.session.clone())
                .collect(),
            balances: data
                .values()
                .filter_map(|item| item.balance.clone())
                .collect(),
        })
    }

    async fn save_account(&self, account: &BookmakerAccount) -> Result<(), String> {
        self.save_account_inner(account).await
    }

    async fn save_session(&self, session: &BookmakerSession) -> Result<(), String> {
        self.save_session_inner(session).await
    }

    async fn save_balance_snapshot(
        &self,
        snapshot: &BookmakerBalanceSnapshot,
    ) -> Result<(), String> {
        self.save_balance_inner(snapshot).await
    }
}

fn serialize_optional<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>, String> {
    value
        .as_ref()
        .map(|item| serde_json::to_string(item).map_err(|error| error.to_string()))
        .transpose()
}

fn parse_json_column<T>(row: &sqlx::sqlite::SqliteRow, column: &str, bookmaker: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let raw = row.try_get::<Option<String>, _>(column).ok().flatten()?;
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(bookmaker, column, error = %error, "Failed to deserialize persisted execution state column");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{BookmakerExecutionMode, BookmakerSessionState};
    use uuid::Uuid;

    fn make_account(bookmaker: &str) -> BookmakerAccount {
        BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: bookmaker.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[tokio::test]
    async fn stores_and_loads_execution_state_in_memory() {
        let store = ExecutionStateStore::new("memory").await.unwrap();
        let account = make_account("pari");
        let session = BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        };
        let balance = BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: "RUB".into(),
            total_balance: 15_000.0,
            available_balance: 12_000.0,
            exposure: 3_000.0,
            captured_at: Utc::now(),
        };

        store.save_account(&account).await.unwrap();
        store.save_session(&session).await.unwrap();
        store.save_balance_snapshot(&balance).await.unwrap();

        let snapshot = store.load_snapshot().await.unwrap();
        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.balances.len(), 1);
        assert_eq!(snapshot.accounts[0].bookmaker, "pari");
        assert_eq!(snapshot.balances[0].available_balance, 12_000.0);
    }
}
