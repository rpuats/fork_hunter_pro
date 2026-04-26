use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Счет в букмекере
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAccount {
    /// Уникальный ID счета
    pub id: Uuid,

    /// Название букмекера
    pub bookmaker: String,

    /// Текущий баланс
    pub balance: f64,

    /// Начальный баланс (для отслеживания)
    pub initial_balance: f64,

    /// Валюта счета
    pub currency: String,

    /// Активен ли счет
    pub active: bool,

    /// Время создания
    pub created_at: DateTime<Utc>,

    /// Время последнего обновления
    pub updated_at: DateTime<Utc>,
}

impl BookmakerAccount {
    /// Создает новый счет
    pub fn new(bookmaker: String, initial_balance: f64, currency: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            bookmaker,
            balance: initial_balance,
            initial_balance,
            currency,
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Пополняет счет
    pub fn deposit(&mut self, amount: f64) {
        if amount > 0.0 {
            self.balance += amount;
            self.updated_at = Utc::now();
        }
    }

    /// Снимает со счета
    pub fn withdraw(&mut self, amount: f64) -> Result<(), String> {
        if amount > self.balance {
            return Err(format!(
                "Insufficient balance: {} > {}",
                amount, self.balance
            ));
        }
        self.balance -= amount;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Возвращает ставку (отмена ставки)
    pub fn return_stake(&mut self, amount: f64) {
        if amount > 0.0 {
            self.balance += amount;
            self.updated_at = Utc::now();
        }
    }

    /// Проверяет, достаточно ли средств
    pub fn has_sufficient_balance(&self, amount: f64) -> bool {
        self.balance >= amount && self.active
    }

    /// Получает прибыль/убыток
    pub fn get_profit_loss(&self) -> f64 {
        self.balance - self.initial_balance
    }

    /// Получает ROI
    pub fn get_roi(&self) -> f64 {
        if self.initial_balance == 0.0 {
            return 0.0;
        }
        self.get_profit_loss() / self.initial_balance
    }
}

/// Менеджер счетов
pub struct AccountManager {
    accounts: HashMap<String, BookmakerAccount>,
}

impl AccountManager {
    /// Создает новый менеджер счетов
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    /// Добавляет счет
    pub fn add_account(&mut self, account: BookmakerAccount) {
        self.accounts.insert(account.bookmaker.clone(), account);
    }

    /// Получает счет по названию букмекера
    pub fn get_account(&self, bookmaker: &str) -> Option<&BookmakerAccount> {
        self.accounts.get(bookmaker)
    }

    /// Получает мутабельный счет
    pub fn get_account_mut(&mut self, bookmaker: &str) -> Option<&mut BookmakerAccount> {
        self.accounts.get_mut(bookmaker)
    }

    /// Проверяет, есть ли счет
    pub fn has_account(&self, bookmaker: &str) -> bool {
        self.accounts.contains_key(bookmaker)
    }

    /// Проверяет, достаточно ли баланса
    pub fn has_sufficient_balance(&self, bookmaker: &str, amount: f64) -> bool {
        self.accounts
            .get(bookmaker)
            .map(|acc| acc.has_sufficient_balance(amount))
            .unwrap_or(false)
    }

    /// Пополняет счет
    pub fn deposit(&mut self, bookmaker: &str, amount: f64) -> Result<(), String> {
        self.accounts
            .get_mut(bookmaker)
            .ok_or_else(|| format!("Account for {} not found", bookmaker))
            .map(|acc| acc.deposit(amount))
    }

    /// Снимает со счета
    pub fn withdraw(&mut self, bookmaker: &str, amount: f64) -> Result<(), String> {
        self.accounts
            .get_mut(bookmaker)
            .ok_or_else(|| format!("Account for {} not found", bookmaker))
            .and_then(|acc| acc.withdraw(amount))
    }

    /// Возвращает ставку
    pub fn return_stake(&mut self, bookmaker: &str, amount: f64) -> Result<(), String> {
        self.accounts
            .get_mut(bookmaker)
            .ok_or_else(|| format!("Account for {} not found", bookmaker))
            .map(|acc| acc.return_stake(amount))
    }

    /// Получает общий баланс
    pub fn get_total_balance(&self) -> f64 {
        self.accounts.values().map(|acc| acc.balance).sum()
    }

    /// Получает общий начальный баланс
    pub fn get_total_initial_balance(&self) -> f64 {
        self.accounts.values().map(|acc| acc.initial_balance).sum()
    }

    /// Получает общий профит
    pub fn get_total_profit(&self) -> f64 {
        self.get_total_balance() - self.get_total_initial_balance()
    }

    /// Получает список всех счетов
    pub fn get_all_accounts(&self) -> Vec<BookmakerAccount> {
        self.accounts.values().cloned().collect()
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_account() {
        let account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        assert_eq!(account.balance, 10000.0);
        assert_eq!(account.initial_balance, 10000.0);
        assert!(account.active);
    }

    #[test]
    fn test_deposit() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        account.deposit(5000.0);
        assert_eq!(account.balance, 15000.0);
    }

    #[test]
    fn test_withdraw_success() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        let result = account.withdraw(3000.0);
        assert!(result.is_ok());
        assert_eq!(account.balance, 7000.0);
    }

    #[test]
    fn test_withdraw_insufficient_balance() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        let result = account.withdraw(15000.0);
        assert!(result.is_err());
        assert_eq!(account.balance, 10000.0);
    }

    #[test]
    fn test_return_stake() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        let _ = account.withdraw(3000.0);
        account.return_stake(3000.0);
        assert_eq!(account.balance, 10000.0);
    }

    #[test]
    fn test_profit_loss() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        account.balance = 12000.0;
        assert_eq!(account.get_profit_loss(), 2000.0);

        account.balance = 8000.0;
        assert_eq!(account.get_profit_loss(), -2000.0);
    }

    #[test]
    fn test_roi() {
        let mut account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        account.balance = 12000.0;
        assert!((account.get_roi() - 0.2).abs() < 0.001); // 20%
    }

    #[test]
    fn test_account_manager_add() {
        let mut manager = AccountManager::new();
        let account = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());

        manager.add_account(account);
        assert!(manager.has_account("Pari"));
    }

    #[test]
    fn test_account_manager_balance() {
        let mut manager = AccountManager::new();

        let account1 = BookmakerAccount::new("Pari".to_string(), 10000.0, "RUB".to_string());
        let account2 = BookmakerAccount::new("Fonbet".to_string(), 5000.0, "RUB".to_string());

        manager.add_account(account1);
        manager.add_account(account2);

        assert_eq!(manager.get_total_balance(), 15000.0);
    }
}
