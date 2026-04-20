use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Запись в реестре ставок
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetLedgerEntry {
    /// ID записи
    pub id: Uuid,
    
    /// ID команды ставки
    pub bet_command_id: Uuid,
    
    /// ID вилки
    pub surebet_id: Uuid,
    
    /// Букмекер
    pub bookmaker: String,
    
    /// ID события
    pub event_id: String,
    
    /// Рынок ставки
    pub market: String,
    
    /// Выбор (selection)
    pub selection: String,
    
    /// Размещенная ставка
    pub stake: f64,
    
    /// Коэффициент
    pub odds: f64,
    
    /// Статус ставки
    pub status: String, // "pending", "placed", "won", "lost", "voided", "cancelled"
    
    /// Результат (для settled ставок)
    pub result: Option<String>, // "won", "lost", "void", "cancelled"
    
    /// Выплата (если выиграла)
    pub payout: Option<f64>,
    
    /// Прибыль/убыток
    pub profit_loss: Option<f64>,
    
    /// Время размещения
    pub placed_at: DateTime<Utc>,
    
    /// Время урегулирования (если урегулирована)
    pub settled_at: Option<DateTime<Utc>>,
    
    /// Дополнительная информация
    pub notes: Option<String>,
}

impl BetLedgerEntry {
    /// Создает новую запись для размещенной ставки
    pub fn new(
        bet_command_id: Uuid,
        surebet_id: Uuid,
        bookmaker: String,
        event_id: String,
        market: String,
        selection: String,
        stake: f64,
        odds: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            bet_command_id,
            surebet_id,
            bookmaker,
            event_id,
            market,
            selection,
            stake,
            odds,
            status: "pending".to_string(),
            result: None,
            payout: None,
            profit_loss: None,
            placed_at: Utc::now(),
            settled_at: None,
            notes: None,
        }
    }
    
    /// Отмечает ставку как выигрыш
    pub fn mark_won(&mut self, payout: f64) {
        self.status = "settled".to_string();
        self.result = Some("won".to_string());
        self.payout = Some(payout);
        self.profit_loss = Some(payout - self.stake);
        self.settled_at = Some(Utc::now());
    }
    
    /// Отмечает ставку как проигрыш
    pub fn mark_lost(&mut self) {
        self.status = "settled".to_string();
        self.result = Some("lost".to_string());
        self.payout = Some(0.0);
        self.profit_loss = Some(-self.stake);
        self.settled_at = Some(Utc::now());
    }
    
    /// Отмечает ставку как аннулированную
    pub fn mark_voided(&mut self) {
        self.status = "settled".to_string();
        self.result = Some("void".to_string());
        self.payout = Some(self.stake);
        self.profit_loss = Some(0.0);
        self.settled_at = Some(Utc::now());
    }
    
    /// Отмечает ставку как отмененную
    pub fn mark_cancelled(&mut self) {
        self.status = "cancelled".to_string();
        self.result = Some("cancelled".to_string());
        self.payout = Some(self.stake);
        self.profit_loss = Some(0.0);
        self.settled_at = Some(Utc::now());
    }
    
    /// Отмечает ставку как размещенную
    pub fn mark_placed(&mut self) {
        self.status = "placed".to_string();
    }
    
    /// Проверяет, урегулирована ли ставка
    pub fn is_settled(&self) -> bool {
        self.status == "settled"
    }
    
    /// Получает размер выигрыша/проигрыша
    pub fn get_result_amount(&self) -> f64 {
        self.profit_loss.unwrap_or(0.0)
    }
}

/// Статистика по ставкам
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetStatistics {
    /// Всего размещено ставок
    pub total_bets: u64,
    
    /// Всего выигрышей
    pub winning_bets: u64,
    
    /// Всего проигрышей
    pub losing_bets: u64,
    
    /// Всего аннулированных ставок
    pub voided_bets: u64,
    
    /// Всего отмененных ставок
    pub cancelled_bets: u64,
    
    /// Общий размещенный объем
    pub total_stake: f64,
    
    /// Общая выплата
    pub total_payout: f64,
    
    /// Общий профит/убыток
    pub total_profit_loss: f64,
    
    /// ROI
    pub roi: f64,
    
    /// Процент выигрыша
    pub win_rate: f64,
    
    /// Средняя ставка
    pub avg_stake: f64,
    
    /// Средний коэффициент
    pub avg_odds: f64,
    
    /// Период (дата начала сбора статистики)
    pub period_start: DateTime<Utc>,
    
    /// Время последнего обновления
    pub last_updated: DateTime<Utc>,
}

impl BetStatistics {
    /// Создает новую статистику
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            total_bets: 0,
            winning_bets: 0,
            losing_bets: 0,
            voided_bets: 0,
            cancelled_bets: 0,
            total_stake: 0.0,
            total_payout: 0.0,
            total_profit_loss: 0.0,
            roi: 0.0,
            win_rate: 0.0,
            avg_stake: 0.0,
            avg_odds: 0.0,
            period_start: now,
            last_updated: now,
        }
    }
    
    /// Обновляет статистику с новой записью
    pub fn update(&mut self, entry: &BetLedgerEntry) {
        if entry.is_settled() {
            self.total_bets += 1;
            self.total_stake += entry.stake;
            
            match entry.result.as_deref() {
                Some("won") => {
                    self.winning_bets += 1;
                    if let Some(payout) = entry.payout {
                        self.total_payout += payout;
                    }
                }
                Some("lost") => {
                    self.losing_bets += 1;
                }
                Some("void") => {
                    self.voided_bets += 1;
                    if let Some(payout) = entry.payout {
                        self.total_payout += payout;
                    }
                }
                Some("cancelled") => {
                    self.cancelled_bets += 1;
                    if let Some(payout) = entry.payout {
                        self.total_payout += payout;
                    }
                }
                _ => {}
            }
            
            self.total_profit_loss += entry.get_result_amount();
        }
        
        self.last_updated = Utc::now();
        self.recalculate();
    }
    
    /// Пересчитывает производные метрики
    pub fn recalculate(&mut self) {
        if self.total_stake > 0.0 {
            self.roi = (self.total_profit_loss / self.total_stake) * 100.0;
            self.avg_stake = self.total_stake / self.total_bets.max(1) as f64;
        }
        
        if self.total_bets > 0 {
            self.win_rate = (self.winning_bets as f64 / self.total_bets as f64) * 100.0;
        }
    }
}

impl Default for BetStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait для persistence ставок
#[async_trait::async_trait]
pub trait BetLedgerPersistence: Send + Sync {
    /// Добавляет запись в реестр
    async fn add_entry(&self, entry: BetLedgerEntry) -> anyhow::Result<()>;
    
    /// Обновляет запись
    async fn update_entry(&self, entry: BetLedgerEntry) -> anyhow::Result<()>;
    
    /// Получает запись по ID
    async fn get_entry(&self, id: Uuid) -> anyhow::Result<Option<BetLedgerEntry>>;
    
    /// Получает все записи по вилке
    async fn get_entries_by_surebet(&self, surebet_id: Uuid) -> anyhow::Result<Vec<BetLedgerEntry>>;
    
    /// Получает все записи по букмекеру
    async fn get_entries_by_bookmaker(&self, bookmaker: &str) -> anyhow::Result<Vec<BetLedgerEntry>>;
    
    /// Получает статистику за период
    async fn get_statistics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> anyhow::Result<BetStatistics>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_ledger_entry() {
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
        
        assert_eq!(entry.stake, 1000.0);
        assert_eq!(entry.odds, 2.0);
        assert_eq!(entry.status, "pending");
    }
    
    #[test]
    fn test_mark_won() {
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
        
        entry.mark_won(2000.0);
        
        assert_eq!(entry.status, "settled");
        assert_eq!(entry.result, Some("won".to_string()));
        assert_eq!(entry.payout, Some(2000.0));
        assert_eq!(entry.profit_loss, Some(1000.0));
        assert!(entry.is_settled());
    }
    
    #[test]
    fn test_mark_lost() {
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
        
        entry.mark_lost();
        
        assert_eq!(entry.status, "settled");
        assert_eq!(entry.result, Some("lost".to_string()));
        assert_eq!(entry.payout, Some(0.0));
        assert_eq!(entry.profit_loss, Some(-1000.0));
    }
    
    #[test]
    fn test_bet_statistics() {
        let mut stats = BetStatistics::new();
        
        let mut entry1 = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-1".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );
        entry1.mark_won(2000.0);
        
        stats.update(&entry1);
        
        assert_eq!(stats.total_bets, 1);
        assert_eq!(stats.winning_bets, 1);
        assert_eq!(stats.total_stake, 1000.0);
        assert_eq!(stats.total_profit_loss, 1000.0);
        assert_eq!(stats.win_rate, 100.0);
    }
    
    #[test]
    fn test_bet_statistics_mixed() {
        let mut stats = BetStatistics::new();
        
        // Выигрышная ставка
        let mut entry1 = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-1".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            1000.0,
            2.0,
        );
        entry1.mark_won(2000.0);
        
        // Проигрышная ставка
        let mut entry2 = BetLedgerEntry::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-2".to_string(),
            "1x2".to_string(),
            "X".to_string(),
            1000.0,
            3.0,
        );
        entry2.mark_lost();
        
        stats.update(&entry1);
        stats.update(&entry2);
        
        assert_eq!(stats.total_bets, 2);
        assert_eq!(stats.winning_bets, 1);
        assert_eq!(stats.losing_bets, 1);
        assert_eq!(stats.total_stake, 2000.0);
        assert_eq!(stats.total_profit_loss, 0.0); // 1000 - 1000
        assert_eq!(stats.win_rate, 50.0);
    }
}
