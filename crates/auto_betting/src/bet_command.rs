use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// PlaceBeautifulBetCommand - Команда для размещения ставки с соблюдением всех требований
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceBeautifulBetCommand {
    /// Уникальный ID команды
    pub command_id: Uuid,

    /// Связанный с вилкой ID
    pub surebet_id: Uuid,

    /// Букмекер для ставки
    pub bookmaker: String,

    /// ID события
    pub event_id: String,

    /// Рынок (маркет) ставки
    pub market: String,

    /// Выбор (selection) - что ставим
    pub selection: String,

    /// Коэффициент
    pub odds: f64,

    /// Вычисленный размер ставки (на основе Kelly)
    pub calculated_stake: f64,

    /// Минимальная ставка букмекера
    pub min_stake: Option<f64>,

    /// Максимальная ставка букмекера
    pub max_stake: Option<f64>,

    /// Доля Kelly (обычно 0.25 для безопасности)
    pub kelly_fraction: f64,

    /// Предполагаемая вероятность события
    pub estimated_probability: f64,

    /// Истинная вероятность (если известна)
    pub true_probability: Option<f64>,

    /// Маржа букмекера
    pub bookmaker_margin: f64,

    /// Время создания команды
    pub created_at: DateTime<Utc>,

    /// Крайний срок размещения ставки
    pub expires_at: Option<DateTime<Utc>>,

    /// Статус команды
    pub status: BetCommandStatus,

    /// Причина статуса (если есть)
    pub status_reason: Option<String>,

    /// URL для ставки (если применимо)
    pub bet_url: Option<String>,

    /// Метаданные для отслеживания
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BetCommandStatus {
    /// Команда создана, ожидает проверки
    Pending,

    /// Проверка статуса букмекера
    Validating,

    /// Ставка готова к размещению
    Ready,

    /// Ставка размещена
    Placed,

    /// Ставка принята букмекером
    Accepted,

    /// Ставка отклонена
    Rejected,

    /// Команда отменена
    Cancelled,

    /// Ошибка при размещении
    Error,
}

impl PlaceBeautifulBetCommand {
    /// Создает новую команду для размещения ставки
    pub fn new(
        surebet_id: Uuid,
        bookmaker: String,
        event_id: String,
        market: String,
        selection: String,
        odds: f64,
        calculated_stake: f64,
        kelly_fraction: f64,
        estimated_probability: f64,
        bookmaker_margin: f64,
    ) -> Self {
        Self {
            command_id: Uuid::new_v4(),
            surebet_id,
            bookmaker,
            event_id,
            market,
            selection,
            odds,
            calculated_stake,
            min_stake: None,
            max_stake: None,
            kelly_fraction,
            estimated_probability,
            true_probability: None,
            bookmaker_margin,
            created_at: Utc::now(),
            expires_at: None,
            status: BetCommandStatus::Pending,
            status_reason: None,
            bet_url: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Проверяет, истекла ли команда
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Проверяет, что ставка в пределах лимитов
    pub fn is_within_limits(&self) -> bool {
        if let Some(min) = self.min_stake {
            if self.calculated_stake < min {
                return false;
            }
        }

        if let Some(max) = self.max_stake {
            if self.calculated_stake > max {
                return false;
            }
        }

        true
    }

    /// Получает ставку, ограниченную лимитами букмекера
    pub fn get_limited_stake(&self) -> f64 {
        let mut stake = self.calculated_stake;

        if let Some(min) = self.min_stake {
            stake = stake.max(min);
        }

        if let Some(max) = self.max_stake {
            stake = stake.min(max);
        }

        stake
    }

    /// Вычисляет ожидаемый выигрыш
    pub fn expected_payout(&self) -> f64 {
        self.get_limited_stake() * self.odds
    }

    /// Вычисляет ожидаемый профит
    pub fn expected_profit(&self) -> f64 {
        let payout = self.expected_payout();
        payout - self.get_limited_stake()
    }

    /// Получает ROI (Return on Investment)
    pub fn get_roi(&self) -> f64 {
        if self.calculated_stake == 0.0 {
            return 0.0;
        }
        self.expected_profit() / self.calculated_stake
    }

    /// Проверяет, что у нас есть edge (валуйная ставка)
    pub fn has_edge(&self) -> bool {
        // Если истинная вероятность выше, чем подразумевается коэффициентом
        if let Some(true_prob) = self.true_probability {
            let implied_prob = 1.0 / self.odds;
            return true_prob > implied_prob;
        }

        // Альтернативно, проверяем оценочную вероятность
        let implied_prob = 1.0 / self.odds;
        self.estimated_probability > implied_prob
    }

    /// Устанавливает статус и причину
    pub fn set_status(&mut self, status: BetCommandStatus, reason: Option<String>) {
        self.status = status;
        self.status_reason = reason;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_bet_command() {
        let cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0,
            1000.0,
            0.25,
            0.55,
            0.05,
        );

        assert_eq!(cmd.status, BetCommandStatus::Pending);
        assert_eq!(cmd.calculated_stake, 1000.0);
        assert_eq!(cmd.odds, 2.0);
    }

    #[test]
    fn test_is_within_limits() {
        let mut cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0,
            1000.0,
            0.25,
            0.55,
            0.05,
        );

        // Без лимитов - должно быть в порядке
        assert!(cmd.is_within_limits());

        // Установим минимум выше ставки
        cmd.min_stake = Some(2000.0);
        assert!(!cmd.is_within_limits());

        // Установим максимум ниже ставки
        cmd.max_stake = Some(500.0);
        assert!(!cmd.is_within_limits());

        // Корректные лимиты
        cmd.min_stake = Some(500.0);
        cmd.max_stake = Some(2000.0);
        assert!(cmd.is_within_limits());
    }

    #[test]
    fn test_expected_payout() {
        let cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0,
            1000.0,
            0.25,
            0.55,
            0.05,
        );

        let payout = cmd.expected_payout();
        assert!((payout - 2000.0).abs() < 0.01); // 1000 * 2.0
    }

    #[test]
    fn test_expected_profit() {
        let cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0,
            1000.0,
            0.25,
            0.55,
            0.05,
        );

        let profit = cmd.expected_profit();
        assert!((profit - 1000.0).abs() < 0.01); // 2000 - 1000
    }

    #[test]
    fn test_has_edge() {
        let mut cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0, // implied prob = 0.5
            1000.0,
            0.25,
            0.55, // estimated > implied
            0.05,
        );

        assert!(cmd.has_edge()); // 0.55 > 0.5

        cmd.estimated_probability = 0.45;
        assert!(!cmd.has_edge()); // 0.45 < 0.5
    }

    #[test]
    fn test_get_limited_stake() {
        let mut cmd = PlaceBeautifulBetCommand::new(
            Uuid::new_v4(),
            "Pari".to_string(),
            "event-123".to_string(),
            "1x2".to_string(),
            "1".to_string(),
            2.0,
            1000.0,
            0.25,
            0.55,
            0.05,
        );

        assert_eq!(cmd.get_limited_stake(), 1000.0);

        // Минимум выше
        cmd.min_stake = Some(1500.0);
        assert_eq!(cmd.get_limited_stake(), 1500.0);

        // Максимум ниже
        cmd.max_stake = Some(800.0);
        assert_eq!(cmd.get_limited_stake(), 800.0);

        // Обе границы
        cmd.min_stake = Some(900.0);
        cmd.max_stake = Some(1100.0);
        assert_eq!(cmd.get_limited_stake(), 1000.0);
    }
}
