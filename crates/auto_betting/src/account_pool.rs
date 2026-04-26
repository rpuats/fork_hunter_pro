use chrono::{DateTime, Utc};
/// Account pooling for load balancing bets across multiple accounts per bookmaker
/// Enables better risk distribution and coverage for high-volume auto-betting
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Represents a single betting account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettingAccount {
    pub id: String,
    pub bookmaker_slug: String,
    pub account_type: AccountType,
    pub balance: f64,
    pub max_bet: f64,
    pub daily_limit: f64,
    pub daily_spent: f64,
    pub daily_profit: f64,
    pub last_updated: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountType {
    Main,
    Secondary,
    Backup,
    Hedging,
}

impl BettingAccount {
    pub fn available_balance(&self) -> f64 {
        (self.balance - self.daily_spent).max(0.0)
    }

    pub fn can_bet(&self, amount: f64) -> bool {
        self.is_active
            && self.available_balance() >= amount
            && amount <= self.max_bet
            && (self.daily_spent + amount) <= self.daily_limit
    }
}

/// Pool of accounts for a bookmaker with load balancing
#[derive(Clone)]
pub struct AccountPool {
    bookmaker_slug: String,
    accounts: Arc<DashMap<String, Arc<RwLock<BettingAccount>>>>,
    round_robin_index: Arc<RwLock<usize>>,
    selection_strategy: Arc<RwLock<SelectionStrategy>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SelectionStrategy {
    RoundRobin,
    MaxAvailableBalance,
    LeastUsedToday,
    Random,
}

impl AccountPool {
    pub fn new(bookmaker_slug: String) -> Self {
        Self {
            bookmaker_slug,
            accounts: Arc::new(DashMap::new()),
            round_robin_index: Arc::new(RwLock::new(0)),
            selection_strategy: Arc::new(RwLock::new(SelectionStrategy::MaxAvailableBalance)),
        }
    }

    /// Add account to pool
    pub fn add_account(&self, account: BettingAccount) -> Result<(), String> {
        if account.bookmaker_slug != self.bookmaker_slug {
            return Err("Account bookmaker mismatch".to_string());
        }
        self.accounts
            .insert(account.id.clone(), Arc::new(RwLock::new(account)));
        Ok(())
    }

    /// Remove account from pool
    pub fn remove_account(&self, account_id: &str) -> Option<Arc<RwLock<BettingAccount>>> {
        self.accounts.remove(account_id).map(|(_, v)| v)
    }

    /// Get total balance across all accounts
    pub fn total_balance(&self) -> f64 {
        self.accounts
            .iter()
            .map(|entry| entry.value().read().balance)
            .sum()
    }

    /// Get total available balance (balance - daily spent)
    pub fn total_available_balance(&self) -> f64 {
        self.accounts
            .iter()
            .map(|entry| entry.value().read().available_balance())
            .sum()
    }

    /// Select best account for betting based on strategy
    pub fn select_account(&self, min_amount: f64) -> Option<Arc<RwLock<BettingAccount>>> {
        let strategy = *self.selection_strategy.read();

        match strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin(min_amount),
            SelectionStrategy::MaxAvailableBalance => self.select_max_balance(min_amount),
            SelectionStrategy::LeastUsedToday => self.select_least_used(min_amount),
            SelectionStrategy::Random => self.select_random(min_amount),
        }
    }

    fn select_round_robin(&self, min_amount: f64) -> Option<Arc<RwLock<BettingAccount>>> {
        let accounts_vec: Vec<_> = self
            .accounts
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        if accounts_vec.is_empty() {
            return None;
        }

        let mut idx = self.round_robin_index.write();
        let start_idx = *idx;

        for i in 0..accounts_vec.len() {
            let account_idx = (*idx + i) % accounts_vec.len();
            let account = accounts_vec[account_idx].read();

            if account.can_bet(min_amount) {
                *idx = (account_idx + 1) % accounts_vec.len();
                drop(account);
                return Some(accounts_vec[account_idx].clone());
            }
        }

        *idx = start_idx;
        None
    }

    fn select_max_balance(&self, min_amount: f64) -> Option<Arc<RwLock<BettingAccount>>> {
        self.accounts
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|acc| acc.read().can_bet(min_amount))
            .max_by(|a, b| {
                a.read()
                    .available_balance()
                    .partial_cmp(&b.read().available_balance())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn select_least_used(&self, min_amount: f64) -> Option<Arc<RwLock<BettingAccount>>> {
        self.accounts
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|acc| acc.read().can_bet(min_amount))
            .min_by(|a, b| {
                a.read()
                    .daily_spent
                    .partial_cmp(&b.read().daily_spent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn select_random(&self, min_amount: f64) -> Option<Arc<RwLock<BettingAccount>>> {
        use rand::seq::IteratorRandom;

        self.accounts
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|acc| acc.read().can_bet(min_amount))
            .choose(&mut rand::thread_rng())
    }

    /// Set selection strategy
    pub fn set_strategy(&self, strategy: SelectionStrategy) {
        *self.selection_strategy.write() = strategy;
    }

    /// Get all accounts
    pub fn get_all_accounts(&self) -> Vec<BettingAccount> {
        self.accounts
            .iter()
            .map(|entry| entry.value().read().clone())
            .collect()
    }

    /// Get active accounts count
    pub fn active_account_count(&self) -> usize {
        self.accounts
            .iter()
            .filter(|entry| entry.value().read().is_active)
            .count()
    }

    /// Get statistics
    pub fn get_stats(&self) -> PoolStatistics {
        let accounts = self.get_all_accounts();
        let total_balance: f64 = accounts.iter().map(|a| a.balance).sum();
        let total_available: f64 = accounts.iter().map(|a| a.available_balance()).sum();
        let total_daily_spent: f64 = accounts.iter().map(|a| a.daily_spent).sum();
        let total_daily_profit: f64 = accounts.iter().map(|a| a.daily_profit).sum();

        PoolStatistics {
            total_accounts: accounts.len(),
            active_accounts: self.active_account_count(),
            total_balance,
            total_available,
            total_daily_spent,
            total_daily_profit,
            avg_balance: if !accounts.is_empty() {
                total_balance / accounts.len() as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatistics {
    pub total_accounts: usize,
    pub active_accounts: usize,
    pub total_balance: f64,
    pub total_available: f64,
    pub total_daily_spent: f64,
    pub total_daily_profit: f64,
    pub avg_balance: f64,
}

/// Master account manager for all bookmakers
pub struct AccountManager {
    pools: Arc<DashMap<String, Arc<AccountPool>>>,
}

impl AccountManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(DashMap::new()),
        }
    }

    /// Get or create pool for bookmaker
    pub fn get_or_create_pool(&self, bookmaker_slug: &str) -> Arc<AccountPool> {
        if let Some(pool) = self.pools.get(bookmaker_slug) {
            pool.value().clone()
        } else {
            let pool = Arc::new(AccountPool::new(bookmaker_slug.to_string()));
            self.pools.insert(bookmaker_slug.to_string(), pool.clone());
            pool
        }
    }

    /// Add account to appropriate pool
    pub fn add_account(&self, account: BettingAccount) -> Result<(), String> {
        let pool = self.get_or_create_pool(&account.bookmaker_slug);
        pool.add_account(account)
    }

    /// Get total balance across all pools
    pub fn total_balance_all_bks(&self) -> f64 {
        self.pools
            .iter()
            .map(|entry| entry.value().total_balance())
            .sum()
    }

    /// Get global stats
    pub fn get_global_stats(&self) -> GlobalAccountStats {
        let total_balance: f64 = self.pools.iter().map(|p| p.value().total_balance()).sum();
        let total_available: f64 = self
            .pools
            .iter()
            .map(|p| p.value().total_available_balance())
            .sum();

        GlobalAccountStats {
            total_bookmakers: self.pools.len(),
            total_accounts: self.pools.iter().map(|p| p.value().accounts.len()).sum(),
            total_balance,
            total_available,
        }
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAccountStats {
    pub total_bookmakers: usize,
    pub total_accounts: usize,
    pub total_balance: f64,
    pub total_available: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_pool_round_robin() {
        let pool = AccountPool::new("pari".to_string());

        for i in 0..3 {
            let account = BettingAccount {
                id: format!("acc_{}", i),
                bookmaker_slug: "pari".to_string(),
                account_type: AccountType::Main,
                balance: 1000.0,
                max_bet: 500.0,
                daily_limit: 2000.0,
                daily_spent: 0.0,
                daily_profit: 0.0,
                last_updated: Utc::now(),
                is_active: true,
            };
            pool.add_account(account).unwrap();
        }

        pool.set_strategy(SelectionStrategy::RoundRobin);

        for i in 0..6 {
            let account = pool.select_account(100.0);
            assert!(account.is_some());
        }
    }

    #[test]
    fn test_account_manager() {
        let manager = AccountManager::new();

        let account = BettingAccount {
            id: "test_acc".to_string(),
            bookmaker_slug: "zenit".to_string(),
            account_type: AccountType::Main,
            balance: 5000.0,
            max_bet: 1000.0,
            daily_limit: 10000.0,
            daily_spent: 0.0,
            daily_profit: 0.0,
            last_updated: Utc::now(),
            is_active: true,
        };

        manager.add_account(account).unwrap();

        let stats = manager.get_global_stats();
        assert_eq!(stats.total_bookmakers, 1);
        assert_eq!(stats.total_accounts, 1);
        assert_eq!(stats.total_balance, 5000.0);
    }
}
