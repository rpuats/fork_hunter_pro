use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Состояния для размещения ставки
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BetPlacementState {
    /// Ставка создана, ожидает валидации
    Created,
    
    /// Валидация лимитов экспозиции
    ValidatingExposure,
    
    /// Валидация баланса счета
    ValidatingBalance,
    
    /// Готовность к размещению
    Ready,
    
    /// Выполнение ставки
    Executing,
    
    /// Ставка размещена
    Placed,
    
    /// Ставка принята букмекером
    Confirmed,
    
    /// Ставка отмена
    Cancelled,
    
    /// Ошибка при размещении
    Error,
}

/// Событие в жизненном цикле ставки
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BetPlacementEvent {
    /// Ставка создана
    Created {
        timestamp: DateTime<Utc>,
    },
    
    /// Экспозиция валидирована
    ExposureValidated {
        timestamp: DateTime<Utc>,
    },
    
    /// Баланс валидирован
    BalanceValidated {
        timestamp: DateTime<Utc>,
        available_balance: f64,
    },
    
    /// Ставка готова
    ReadyForExecution {
        timestamp: DateTime<Utc>,
    },
    
    /// Ставка выполняется
    ExecutionStarted {
        timestamp: DateTime<Utc>,
    },
    
    /// Ставка размещена
    Placed {
        timestamp: DateTime<Utc>,
        ticket_id: Option<String>,
    },
    
    /// Ставка подтверждена
    Confirmed {
        timestamp: DateTime<Utc>,
    },
    
    /// Ошибка валидации экспозиции
    ExposureValidationFailed {
        timestamp: DateTime<Utc>,
        reason: String,
    },
    
    /// Ошибка валидации баланса
    BalanceValidationFailed {
        timestamp: DateTime<Utc>,
        reason: String,
    },
    
    /// Ошибка выполнения
    ExecutionFailed {
        timestamp: DateTime<Utc>,
        reason: String,
    },
    
    /// Ставка отменена
    Cancelled {
        timestamp: DateTime<Utc>,
        reason: String,
    },
}

/// Машина состояний для размещения ставки
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetPlacementStateMachine {
    /// ID команды
    pub command_id: Uuid,
    
    /// Текущее состояние
    pub state: BetPlacementState,
    
    /// История событий
    pub events: Vec<BetPlacementEvent>,
    
    /// Время создания
    pub created_at: DateTime<Utc>,
    
    /// Время последнего изменения
    pub last_updated: DateTime<Utc>,
    
    /// Сообщение об ошибке (если есть)
    pub error_message: Option<String>,
}

impl BetPlacementStateMachine {
    /// Создает новую машину состояний
    pub fn new(command_id: Uuid) -> Self {
        let now = Utc::now();
        
        let mut machine = Self {
            command_id,
            state: BetPlacementState::Created,
            events: Vec::new(),
            created_at: now,
            last_updated: now,
            error_message: None,
        };
        
        machine.add_event(BetPlacementEvent::Created {
            timestamp: now,
        });
        
        machine
    }
    
    /// Добавляет событие
    fn add_event(&mut self, event: BetPlacementEvent) {
        self.events.push(event);
        self.last_updated = Utc::now();
    }
    
    /// Валидирует экспозицию
    pub fn validate_exposure(&mut self) -> Result<(), String> {
        if self.state != BetPlacementState::Created {
            return Err("Invalid state for exposure validation".into());
        }
        
        self.state = BetPlacementState::ValidatingExposure;
        self.add_event(BetPlacementEvent::ExposureValidated {
            timestamp: Utc::now(),
        });
        
        Ok(())
    }
    
    /// Ошибка при валидации экспозиции
    pub fn fail_exposure_validation(&mut self, reason: String) {
        self.state = BetPlacementState::Error;
        self.error_message = Some(reason.clone());
        self.add_event(BetPlacementEvent::ExposureValidationFailed {
            timestamp: Utc::now(),
            reason,
        });
    }
    
    /// Валидирует баланс
    pub fn validate_balance(&mut self, available_balance: f64) -> Result<(), String> {
        if self.state != BetPlacementState::ValidatingExposure {
            return Err("Invalid state for balance validation".into());
        }
        
        self.state = BetPlacementState::ValidatingBalance;
        self.add_event(BetPlacementEvent::BalanceValidated {
            timestamp: Utc::now(),
            available_balance,
        });
        
        Ok(())
    }
    
    /// Ошибка при валидации баланса
    pub fn fail_balance_validation(&mut self, reason: String) {
        self.state = BetPlacementState::Error;
        self.error_message = Some(reason.clone());
        self.add_event(BetPlacementEvent::BalanceValidationFailed {
            timestamp: Utc::now(),
            reason,
        });
    }
    
    /// Переводит в состояние Ready
    pub fn mark_ready(&mut self) -> Result<(), String> {
        if self.state != BetPlacementState::ValidatingBalance {
            return Err("Invalid state for ready".into());
        }
        
        self.state = BetPlacementState::Ready;
        self.add_event(BetPlacementEvent::ReadyForExecution {
            timestamp: Utc::now(),
        });
        
        Ok(())
    }
    
    /// Начинает выполнение
    pub fn start_execution(&mut self) -> Result<(), String> {
        if self.state != BetPlacementState::Ready {
            return Err("Invalid state for execution start".into());
        }
        
        self.state = BetPlacementState::Executing;
        self.add_event(BetPlacementEvent::ExecutionStarted {
            timestamp: Utc::now(),
        });
        
        Ok(())
    }
    
    /// Отмечает ставку как размещенную
    pub fn mark_placed(&mut self, ticket_id: Option<String>) -> Result<(), String> {
        if self.state != BetPlacementState::Executing {
            return Err("Invalid state for placed".into());
        }
        
        self.state = BetPlacementState::Placed;
        self.add_event(BetPlacementEvent::Placed {
            timestamp: Utc::now(),
            ticket_id,
        });
        
        Ok(())
    }
    
    /// Отмечает ставку как подтвержденную
    pub fn mark_confirmed(&mut self) -> Result<(), String> {
        if self.state != BetPlacementState::Placed {
            return Err("Invalid state for confirmed".into());
        }
        
        self.state = BetPlacementState::Confirmed;
        self.add_event(BetPlacementEvent::Confirmed {
            timestamp: Utc::now(),
        });
        
        Ok(())
    }
    
    /// Ошибка при выполнении
    pub fn fail_execution(&mut self, reason: String) {
        self.state = BetPlacementState::Error;
        self.error_message = Some(reason.clone());
        self.add_event(BetPlacementEvent::ExecutionFailed {
            timestamp: Utc::now(),
            reason,
        });
    }
    
    /// Отменяет ставку
    pub fn cancel(&mut self, reason: String) -> Result<(), String> {
        if matches!(self.state, BetPlacementState::Confirmed | BetPlacementState::Placed | BetPlacementState::Executing) {
            return Err("Cannot cancel bet in current state".into());
        }
        
        self.state = BetPlacementState::Cancelled;
        self.add_event(BetPlacementEvent::Cancelled {
            timestamp: Utc::now(),
            reason,
        });
        
        Ok(())
    }
    
    /// Проверяет, завершена ли ставка
    pub fn is_completed(&self) -> bool {
        matches!(
            self.state,
            BetPlacementState::Confirmed | BetPlacementState::Cancelled | BetPlacementState::Error
        )
    }
    
    /// Проверяет, содержит ли ошибку
    pub fn has_error(&self) -> bool {
        self.state == BetPlacementState::Error || self.error_message.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_state_machine() {
        let command_id = Uuid::new_v4();
        let machine = BetPlacementStateMachine::new(command_id);
        
        assert_eq!(machine.state, BetPlacementState::Created);
        assert_eq!(machine.events.len(), 1);
    }
    
    #[test]
    fn test_validate_exposure() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        let result = machine.validate_exposure();
        assert!(result.is_ok());
        assert_eq!(machine.state, BetPlacementState::ValidatingExposure);
    }
    
    #[test]
    fn test_validate_exposure_invalid_state() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        machine.state = BetPlacementState::Ready;
        
        let result = machine.validate_exposure();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_valid_state_transitions() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        // Created -> ValidatingExposure
        assert!(machine.validate_exposure().is_ok());
        
        // ValidatingExposure -> ValidatingBalance
        assert!(machine.validate_balance(10000.0).is_ok());
        
        // ValidatingBalance -> Ready
        assert!(machine.mark_ready().is_ok());
        
        // Ready -> Executing
        assert!(machine.start_execution().is_ok());
        
        // Executing -> Placed
        assert!(machine.mark_placed(Some("TICKET-123".to_string())).is_ok());
        
        // Placed -> Confirmed
        assert!(machine.mark_confirmed().is_ok());
        
        assert!(machine.is_completed());
    }
    
    #[test]
    fn test_exposure_validation_failure() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        assert!(machine.validate_exposure().is_ok());
        machine.fail_exposure_validation("Exposure limit exceeded".to_string());
        
        assert_eq!(machine.state, BetPlacementState::Error);
        assert!(machine.has_error());
        assert!(machine.is_completed());
    }
    
    #[test]
    fn test_balance_validation_failure() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        assert!(machine.validate_exposure().is_ok());
        assert!(machine.validate_balance(10000.0).is_ok());
        
        machine.fail_balance_validation("Insufficient balance".to_string());
        
        assert_eq!(machine.state, BetPlacementState::Error);
        assert!(machine.has_error());
    }
    
    #[test]
    fn test_execution_failure() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        assert!(machine.validate_exposure().is_ok());
        assert!(machine.validate_balance(10000.0).is_ok());
        assert!(machine.mark_ready().is_ok());
        assert!(machine.start_execution().is_ok());
        
        machine.fail_execution("Network error".to_string());
        
        assert_eq!(machine.state, BetPlacementState::Error);
        assert!(machine.has_error());
    }
    
    #[test]
    fn test_cancel_bet() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        assert!(machine.validate_exposure().is_ok());
        assert!(machine.validate_balance(10000.0).is_ok());
        
        let result = machine.cancel("User cancelled".to_string());
        assert!(result.is_ok());
        assert_eq!(machine.state, BetPlacementState::Cancelled);
        assert!(machine.is_completed());
    }
    
    #[test]
    fn test_cannot_cancel_confirmed_bet() {
        let command_id = Uuid::new_v4();
        let mut machine = BetPlacementStateMachine::new(command_id);
        
        assert!(machine.validate_exposure().is_ok());
        assert!(machine.validate_balance(10000.0).is_ok());
        assert!(machine.mark_ready().is_ok());
        assert!(machine.start_execution().is_ok());
        assert!(machine.mark_placed(None).is_ok());
        assert!(machine.mark_confirmed().is_ok());
        
        let result = machine.cancel("User cancelled".to_string());
        assert!(result.is_err());
    }
}
