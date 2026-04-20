use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use uuid::Uuid;

use super::ledger::{BetLedgerEntry, BetLedgerPersistence, BetStatistics};

/// SQLite-based Bet Ledger Persistence
pub struct SqliteBetLedger {
    pool: SqlitePool,
}

impl SqliteBetLedger {
    /// Создает новое хранилище на основе SQLite
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        // Создаем пул соединений
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        // Запускаем миграции
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bet_ledger (
                id TEXT PRIMARY KEY,
                bet_command_id TEXT NOT NULL,
                surebet_id TEXT NOT NULL,
                bookmaker TEXT NOT NULL,
                event_id TEXT NOT NULL,
                market TEXT NOT NULL,
                selection TEXT NOT NULL,
                stake REAL NOT NULL,
                odds REAL NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                payout REAL,
                profit_loss REAL,
                placed_at TIMESTAMP NOT NULL,
                settled_at TIMESTAMP,
                notes TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Создаем индексы для быстрого поиска
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_surebet_id ON bet_ledger(surebet_id)",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_bookmaker ON bet_ledger(bookmaker)",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_status ON bet_ledger(status)",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_placed_at ON bet_ledger(placed_at)",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Создает базу данных на диске
    pub async fn new_with_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let db_url = format!("sqlite://{}", path.display());
        
        // Убеждаемся, что директория существует
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Self::new(&db_url).await
    }

    /// Создает тестовую базу данных в памяти
    pub async fn new_in_memory() -> anyhow::Result<Self> {
        Self::new("sqlite::memory:").await
    }
}

#[async_trait::async_trait]
impl BetLedgerPersistence for SqliteBetLedger {
    async fn add_entry(&self, entry: BetLedgerEntry) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO bet_ledger (
                id, bet_command_id, surebet_id, bookmaker, event_id, market, selection,
                stake, odds, status, result, payout, profit_loss, placed_at, settled_at, notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(entry.id.to_string())
        .bind(entry.bet_command_id.to_string())
        .bind(entry.surebet_id.to_string())
        .bind(&entry.bookmaker)
        .bind(&entry.event_id)
        .bind(&entry.market)
        .bind(&entry.selection)
        .bind(entry.stake)
        .bind(entry.odds)
        .bind(&entry.status)
        .bind(&entry.result)
        .bind(entry.payout)
        .bind(entry.profit_loss)
        .bind(entry.placed_at)
        .bind(entry.settled_at)
        .bind(&entry.notes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_entry(&self, entry: BetLedgerEntry) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE bet_ledger
            SET status = ?, result = ?, payout = ?, profit_loss = ?, settled_at = ?, notes = ?
            WHERE id = ?
            "#,
        )
        .bind(&entry.status)
        .bind(&entry.result)
        .bind(entry.payout)
        .bind(entry.profit_loss)
        .bind(entry.settled_at)
        .bind(&entry.notes)
        .bind(entry.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_entry(&self, id: Uuid) -> anyhow::Result<Option<BetLedgerEntry>> {
        let row = sqlx::query_as::<_, BetLedgerEntryRow>(
            r#"
            SELECT * FROM bet_ledger WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into_entry()))
    }

    async fn get_entries_by_surebet(
        &self,
        surebet_id: Uuid,
    ) -> anyhow::Result<Vec<BetLedgerEntry>> {
        let rows = sqlx::query_as::<_, BetLedgerEntryRow>(
            r#"
            SELECT * FROM bet_ledger WHERE surebet_id = ? ORDER BY placed_at DESC
            "#,
        )
        .bind(surebet_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_entry()).collect())
    }

    async fn get_entries_by_bookmaker(
        &self,
        bookmaker: &str,
    ) -> anyhow::Result<Vec<BetLedgerEntry>> {
        let rows = sqlx::query_as::<_, BetLedgerEntryRow>(
            r#"
            SELECT * FROM bet_ledger WHERE bookmaker = ? ORDER BY placed_at DESC
            "#,
        )
        .bind(bookmaker)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_entry()).collect())
    }

    async fn get_statistics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> anyhow::Result<BetStatistics> {
        let rows = sqlx::query_as::<_, BetLedgerEntryRow>(
            r#"
            SELECT * FROM bet_ledger
            WHERE placed_at >= ? AND placed_at <= ? AND status = 'settled'
            ORDER BY placed_at DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let mut stats = BetStatistics::new();
        stats.period_start = start;

        for row in rows {
            let entry = row.into_entry();
            stats.update(&entry);
        }

        Ok(stats)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct BetLedgerEntryRow {
    id: String,
    bet_command_id: String,
    surebet_id: String,
    bookmaker: String,
    event_id: String,
    market: String,
    selection: String,
    stake: f64,
    odds: f64,
    status: String,
    result: Option<String>,
    payout: Option<f64>,
    profit_loss: Option<f64>,
    placed_at: DateTime<Utc>,
    settled_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl BetLedgerEntryRow {
    fn into_entry(self) -> BetLedgerEntry {
        BetLedgerEntry {
            id: Uuid::parse_str(&self.id).unwrap_or_else(|_| Uuid::new_v4()),
            bet_command_id: Uuid::parse_str(&self.bet_command_id)
                .unwrap_or_else(|_| Uuid::new_v4()),
            surebet_id: Uuid::parse_str(&self.surebet_id)
                .unwrap_or_else(|_| Uuid::new_v4()),
            bookmaker: self.bookmaker,
            event_id: self.event_id,
            market: self.market,
            selection: self.selection,
            stake: self.stake,
            odds: self.odds,
            status: self.status,
            result: self.result,
            payout: self.payout,
            profit_loss: self.profit_loss,
            placed_at: self.placed_at,
            settled_at: self.settled_at,
            notes: self.notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_in_memory_database() {
        let ledger = SqliteBetLedger::new_in_memory().await;
        assert!(ledger.is_ok());
    }

    #[tokio::test]
    async fn test_add_entry() {
        let ledger = SqliteBetLedger::new_in_memory()
            .await
            .expect("Failed to create ledger");

        let entry = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );

        let result = ledger.add_entry(entry.clone()).await;
        assert!(result.is_ok());

        let retrieved = ledger.get_entry(entry.id).await.expect("Failed to get entry");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.bookmaker, "Pari");
        assert_eq!(retrieved.stake, 1000.0);
    }

    #[tokio::test]
    async fn test_update_entry() {
        let ledger = SqliteBetLedger::new_in_memory()
            .await
            .expect("Failed to create ledger");

        let mut entry = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );

        ledger.add_entry(entry.clone()).await.expect("Failed to add entry");

        entry.mark_won(2000.0);
        ledger.update_entry(entry.clone()).await.expect("Failed to update entry");

        let retrieved = ledger.get_entry(entry.id).await.expect("Failed to get entry");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.status, "settled");
        assert_eq!(retrieved.result, Some("won".to_string()));
        assert_eq!(retrieved.payout, Some(2000.0));
    }

    #[tokio::test]
    async fn test_get_entries_by_surebet() {
        let ledger = SqliteBetLedger::new_in_memory()
            .await
            .expect("Failed to create ledger");

        let surebet_id = Uuid::new_v4();

        let entry1 = BetLedgerEntry::new(
            Uuid::new_v4(),
            surebet_id,
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );

        let entry2 = BetLedgerEntry::new(
            Uuid::new_v4(),
            surebet_id,
            "Fonbet".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "2".to_string(),
            1000.0,
            3.0,
        );

        ledger.add_entry(entry1).await.expect("Failed to add entry");
        ledger.add_entry(entry2).await.expect("Failed to add entry");

        let entries = ledger.get_entries_by_surebet(surebet_id).await.expect("Failed to get entries");
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_get_statistics() {
        let ledger = SqliteBetLedger::new_in_memory()
            .await
            .expect("Failed to create ledger");

        let now = Utc::now();
        let start = now - chrono::Duration::hours(1);
        let end = now + chrono::Duration::hours(1);

        let mut entry1 = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );
        entry1.mark_won(2000.0);

        let mut entry2 = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Fonbet".to_string(),
            "event-456".to_string(),
            "1x2".to_string(),
            "X".to_string(),
            1000.0,
            3.0,
        );
        entry2.mark_lost();

        ledger.add_entry(entry1).await.expect("Failed to add entry");
        ledger.add_entry(entry2).await.expect("Failed to add entry");

        let stats = ledger.get_statistics(start, end).await.expect("Failed to get statistics");
        assert_eq!(stats.total_bets, 2);
        assert_eq!(stats.winning_bets, 1);
        assert_eq!(stats.losing_bets, 1);
        assert_eq!(stats.total_stake, 2000.0);
    }
}
