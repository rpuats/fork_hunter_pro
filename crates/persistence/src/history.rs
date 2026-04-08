use shared::{Surebet, SurebetLeg};
use sqlx::SqlitePool;
use std::sync::Arc;
use anyhow::{Error, Result};
use tracing::{info, warn};

pub struct SurebetHistory {
    pool: Option<Arc<SqlitePool>>,
    /// In-memory fallback
    data: Arc<tokio::sync::Mutex<Vec<Surebet>>>,
}

impl SurebetHistory {
    pub async fn new(database_url: &str) -> Result<Self, Error> {
        if database_url.is_empty() || database_url == "memory" {
            info!("Using in-memory history storage");
            return Ok(Self {
                pool: None,
                data: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            });
        }

        // Try to connect to SQLite
        match SqlitePool::connect(database_url).await {
            Ok(pool) => {
                info!(url = database_url, "Connected to SQLite");
                let hist = Self {
                    pool: Some(Arc::new(pool)),
                    data: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                };
                hist.migrate().await?;
                Ok(hist)
            }
            Err(e) => {
                warn!(error = %e, "Failed to connect to SQLite, falling back to in-memory");
                Ok(Self {
                    pool: None,
                    data: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                })
            }
        }
    }

    async fn migrate(&self) -> Result<(), Error> {
        if let Some(pool) = &self.pool {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS surebets (
                    id TEXT PRIMARY KEY,
                    sport TEXT NOT NULL,
                    league TEXT NOT NULL,
                    home_team TEXT NOT NULL,
                    away_team TEXT NOT NULL,
                    start_time INTEGER,
                    is_live INTEGER NOT NULL DEFAULT 0,
                    profit_percent REAL NOT NULL,
                    total_stake REAL NOT NULL,
                    detected_at INTEGER NOT NULL,
                    verified INTEGER NOT NULL DEFAULT 0,
                    mirror INTEGER NOT NULL DEFAULT 0
                )
                "#,
            ).execute(pool.as_ref()).await?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS surebet_legs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    surebet_id TEXT NOT NULL,
                    bookmaker TEXT NOT NULL,
                    market TEXT NOT NULL,
                    selection TEXT NOT NULL,
                    odds REAL NOT NULL,
                    line REAL,
                    stake REAL NOT NULL,
                    payout REAL NOT NULL,
                    url TEXT,
                    FOREIGN KEY (surebet_id) REFERENCES surebets(id)
                )
                "#,
            ).execute(pool.as_ref()).await?;

            info!("Database migration complete");
        }
        Ok(())
    }

    pub async fn save(&self, surebet: &Surebet) -> Result<(), Error> {
        // Always save to in-memory
        self.data.lock().await.push(surebet.clone());

        // Also save to SQLite if available
        if let Some(pool) = &self.pool {
            let start_time_ts = surebet.start_time.map(|t| t.timestamp());
            let detected_ts = surebet.detected_at.timestamp();

            sqlx::query(
                r#"
                INSERT OR REPLACE INTO surebets (id, sport, league, home_team, away_team, start_time, is_live, profit_percent, total_stake, detected_at, verified, mirror)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&surebet.id.to_string())
            .bind(format!("{:?}", surebet.sport))
            .bind(&surebet.league)
            .bind(&surebet.home_team)
            .bind(&surebet.away_team)
            .bind(start_time_ts)
            .bind(if surebet.is_live { 1 } else { 0 })
            .bind(surebet.profit_percent)
            .bind(surebet.total_stake)
            .bind(detected_ts)
            .bind(if surebet.verified { 1 } else { 0 })
            .bind(if surebet.mirror { 1 } else { 0 })
            .execute(pool.as_ref()).await?;

            // Save legs
            for leg in &surebet.legs {
                sqlx::query(
                    r#"
                    INSERT INTO surebet_legs (surebet_id, bookmaker, market, selection, odds, line, stake, payout, url)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&surebet.id.to_string())
                .bind(&leg.bookmaker)
                .bind(&leg.market)
                .bind(&leg.selection)
                .bind(leg.odds)
                .bind(leg.line)
                .bind(leg.stake)
                .bind(leg.payout)
                .bind(&leg.url)
                .execute(pool.as_ref()).await?;
            }
        }

        Ok(())
    }

    pub async fn get_recent(&self, limit: i32) -> Result<Vec<Surebet>, Error> {
        // Return from in-memory (always available)
        let data = self.data.lock().await;
        let result: Vec<Surebet> = data.iter()
            .rev()
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(result)
    }

    pub async fn get_stats(&self) -> Result<SurebetStats, Error> {
        let data = self.data.lock().await;
        let total = data.len();
        
        if total == 0 {
            return Ok(SurebetStats {
                total: 0,
                avg_profit: 0.0,
                max_profit: 0.0,
                total_stake: 0.0,
                total_profit: 0.0,
            });
        }

        let mut total_profit = 0.0;
        let mut max_profit = f64::MIN;
        let mut total_stake = 0.0;

        for sb in data.iter() {
            let profit = sb.profit_percent * sb.total_stake / 100.0;
            total_profit += profit;
            max_profit = max_profit.max(profit);
            total_stake += sb.total_stake;
        }

        Ok(SurebetStats {
            total,
            avg_profit: total_profit / total as f64,
            max_profit,
            total_stake,
            total_profit,
        })
    }

    pub async fn get_legs(&self, surebet_id: &str) -> Result<Vec<SurebetLeg>, Error> {
        let data = self.data.lock().await;
        let legs = data.iter()
            .find(|sb| sb.id.to_string() == surebet_id)
            .map(|sb| sb.legs.clone())
            .unwrap_or_default();
        Ok(legs)
    }

    pub async fn count(&self) -> Result<usize, Error> {
        let data = self.data.lock().await;
        Ok(data.len())
    }
}

#[derive(Debug, Clone)]
pub struct SurebetStats {
    pub total: usize,
    pub avg_profit: f64,
    pub max_profit: f64,
    pub total_stake: f64,
    pub total_profit: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Sport;
    use uuid::Uuid;
    use chrono::Utc;

    fn make_test_surebet() -> Surebet {
        Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Test League".into(),
            home_team: "Team A".into(),
            away_team: "Team B".into(),
            start_time: None,
            is_live: false,
            profit_percent: 3.0,
            total_stake: 1000.0,
            legs: vec![
                SurebetLeg {
                    bookmaker: "bk1".into(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: 2.0,
                    line: None,
                    stake: 500.0,
                    payout: 1000.0,
                    url: None,
                },
            ],
            detected_at: Utc::now(),
            verified: true,
            mirror: false,
        }
    }

    #[tokio::test]
    async fn test_save_and_get_recent() {
        let hist = SurebetHistory::new("memory").await.unwrap();
        
        let surebet = make_test_surebet();
        hist.save(&surebet).await.unwrap();
        
        let recent = hist.get_recent(10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].profit_percent, 3.0);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let hist = SurebetHistory::new("memory").await.unwrap();
        
        for i in 0..5 {
            let mut sb = make_test_surebet();
            sb.profit_percent = (i + 1) as f64;
            sb.total_stake = 1000.0;
            hist.save(&sb).await.unwrap();
        }
        
        let stats = hist.get_stats().await.unwrap();
        assert_eq!(stats.total, 5);
        assert!(stats.avg_profit > 0.0);
        assert!(stats.total_stake > 0.0);
    }

    #[tokio::test]
    async fn test_get_legs() {
        let hist = SurebetHistory::new("memory").await.unwrap();
        
        let surebet = make_test_surebet();
        let sb_id = surebet.id.to_string();
        hist.save(&surebet).await.unwrap();
        
        let legs = hist.get_legs(&sb_id).await.unwrap();
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].bookmaker, "bk1");
    }

    #[tokio::test]
    async fn test_count() {
        let hist = SurebetHistory::new("memory").await.unwrap();
        assert_eq!(hist.count().await.unwrap(), 0);
        
        hist.save(&make_test_surebet()).await.unwrap();
        assert_eq!(hist.count().await.unwrap(), 1);
    }
}
