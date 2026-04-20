use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Лимиты экспозиции для управления рисками
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureLimits {
    /// Максимальная экспозиция на один букмекер (%)
    pub per_bookmaker_percent: f64,
    
    /// Максимальная экспозиция на одно событие (%)
    pub per_event_percent: f64,
    
    /// Максимальная экспозиция на одну лигу (%)
    pub per_league_percent: f64,
    
    /// Максимальная экспозиция на один вид спорта (%)
    pub per_sport_percent: f64,
    
    /// Минимальная экспозиция между ставками для диверсификации (%)
    pub min_diversification_percent: f64,
}

impl Default for ExposureLimits {
    fn default() -> Self {
        Self {
            per_bookmaker_percent: 10.0,  // 10% bankroll на букмекер
            per_event_percent: 5.0,        // 5% bankroll на событие
            per_league_percent: 15.0,      // 15% bankroll на лигу
            per_sport_percent: 30.0,       // 30% bankroll на спорт
            min_diversification_percent: 1.0,
        }
    }
}

/// Текущая экспозиция по ставкам
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureTracker {
    /// Экспозиция по букмекерам: bookmaker -> exposure_amount
    pub by_bookmaker: HashMap<String, f64>,
    
    /// Экспозиция по событиям: event_id -> exposure_amount
    pub by_event: HashMap<String, f64>,
    
    /// Экспозиция по лигам: league -> exposure_amount
    pub by_league: HashMap<String, f64>,
    
    /// Экспозиция по спортам: sport -> exposure_amount
    pub by_sport: HashMap<String, f64>,
}

impl Default for ExposureTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ExposureTracker {
    /// Создает новый трекер
    pub fn new() -> Self {
        Self {
            by_bookmaker: HashMap::new(),
            by_event: HashMap::new(),
            by_league: HashMap::new(),
            by_sport: HashMap::new(),
        }
    }
    
    /// Добавляет экспозицию по букмекеру
    pub fn add_bookmaker_exposure(&mut self, bookmaker: &str, amount: f64) {
        *self.by_bookmaker.entry(bookmaker.to_string()).or_insert(0.0) += amount;
    }
    
    /// Добавляет экспозицию по событию
    pub fn add_event_exposure(&mut self, event_id: &str, amount: f64) {
        *self.by_event.entry(event_id.to_string()).or_insert(0.0) += amount;
    }
    
    /// Добавляет экспозицию по лиге
    pub fn add_league_exposure(&mut self, league: &str, amount: f64) {
        *self.by_league.entry(league.to_string()).or_insert(0.0) += amount;
    }
    
    /// Добавляет экспозицию по спорту
    pub fn add_sport_exposure(&mut self, sport: &str, amount: f64) {
        *self.by_sport.entry(sport.to_string()).or_insert(0.0) += amount;
    }
    
    /// Получает общую экспозицию по букмекеру
    pub fn get_bookmaker_exposure(&self, bookmaker: &str) -> f64 {
        self.by_bookmaker.get(bookmaker).copied().unwrap_or(0.0)
    }
    
    /// Получает общую экспозицию по событию
    pub fn get_event_exposure(&self, event_id: &str) -> f64 {
        self.by_event.get(event_id).copied().unwrap_or(0.0)
    }
    
    /// Получает общую экспозицию по лиге
    pub fn get_league_exposure(&self, league: &str) -> f64 {
        self.by_league.get(league).copied().unwrap_or(0.0)
    }
    
    /// Получает общую экспозицию по спорту
    pub fn get_sport_exposure(&self, sport: &str) -> f64 {
        self.by_sport.get(sport).copied().unwrap_or(0.0)
    }
    
    /// Получает общую экспозицию
    pub fn get_total_exposure(&self) -> f64 {
        // Считаем максимум - может быть пересчитано для вилок
        self.by_bookmaker.values().sum::<f64>().max(
            self.by_event.values().sum::<f64>()
        )
    }
}

/// Валидатор экспозиции
pub struct ExposureValidator {
    limits: ExposureLimits,
    tracker: ExposureTracker,
}

impl ExposureValidator {
    /// Создает новый валидатор
    pub fn new(limits: ExposureLimits) -> Self {
        Self {
            limits,
            tracker: ExposureTracker::new(),
        }
    }
    
    /// Проверяет, может ли быть размещена ставка
    pub fn can_place_bet(
        &self,
        bookmaker: &str,
        event_id: &str,
        league: &str,
        sport: &str,
        stake: f64,
        bankroll: f64,
    ) -> Result<(), String> {
        // Проверяем лимит по букмекеру
        let bk_exposure = self.tracker.get_bookmaker_exposure(bookmaker) + stake;
        let bk_limit = bankroll * (self.limits.per_bookmaker_percent / 100.0);
        if bk_exposure > bk_limit {
            return Err(format!(
                "Bookmaker exposure limit exceeded: {} > {}",
                bk_exposure, bk_limit
            ));
        }
        
        // Проверяем лимит по событию
        let event_exposure = self.tracker.get_event_exposure(event_id) + stake;
        let event_limit = bankroll * (self.limits.per_event_percent / 100.0);
        if event_exposure > event_limit {
            return Err(format!(
                "Event exposure limit exceeded: {} > {}",
                event_exposure, event_limit
            ));
        }
        
        // Проверяем лимит по лиге
        let league_exposure = self.tracker.get_league_exposure(league) + stake;
        let league_limit = bankroll * (self.limits.per_league_percent / 100.0);
        if league_exposure > league_limit {
            return Err(format!(
                "League exposure limit exceeded: {} > {}",
                league_exposure, league_limit
            ));
        }
        
        // Проверяем лимит по спорту
        let sport_exposure = self.tracker.get_sport_exposure(sport) + stake;
        let sport_limit = bankroll * (self.limits.per_sport_percent / 100.0);
        if sport_exposure > sport_limit {
            return Err(format!(
                "Sport exposure limit exceeded: {} > {}",
                sport_exposure, sport_limit
            ));
        }
        
        Ok(())
    }
    
    /// Регистрирует размещенную ставку
    pub fn register_bet(
        &mut self,
        bookmaker: &str,
        event_id: &str,
        league: &str,
        sport: &str,
        stake: f64,
    ) {
        self.tracker.add_bookmaker_exposure(bookmaker, stake);
        self.tracker.add_event_exposure(event_id, stake);
        self.tracker.add_league_exposure(league, stake);
        self.tracker.add_sport_exposure(sport, stake);
    }
    
    /// Получает текущий трекер
    pub fn get_tracker(&self) -> &ExposureTracker {
        &self.tracker
    }
    
    /// Получает мутабельный трекер
    pub fn get_tracker_mut(&mut self) -> &mut ExposureTracker {
        &mut self.tracker
    }
    
    /// Сбрасывает трекер (например, в конце дня)
    pub fn reset(&mut self) {
        self.tracker = ExposureTracker::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exposure_tracker_add_bookmaker() {
        let mut tracker = ExposureTracker::new();
        tracker.add_bookmaker_exposure("Pari", 1000.0);
        tracker.add_bookmaker_exposure("Pari", 500.0);
        
        assert_eq!(tracker.get_bookmaker_exposure("Pari"), 1500.0);
    }
    
    #[test]
    fn test_exposure_tracker_add_event() {
        let mut tracker = ExposureTracker::new();
        tracker.add_event_exposure("event-123", 2000.0);
        
        assert_eq!(tracker.get_event_exposure("event-123"), 2000.0);
    }
    
    #[test]
    fn test_exposure_validator_bookmaker_limit() {
        let limits = ExposureLimits {
            per_bookmaker_percent: 10.0,
            ..Default::default()
        };
        let mut validator = ExposureValidator::new(limits);
        let bankroll = 100000.0;
        
        // Первая ставка в 8000 - должна пройти (8% < 10%)
        let result = validator.can_place_bet("Pari", "event-1", "League1", "Football", 8000.0, bankroll);
        assert!(result.is_ok());
        
        // Регистрируем ставку
        validator.register_bet("Pari", "event-1", "League1", "Football", 8000.0);
        
        // Вторая ставка в 3000 - должна пройти (11% всего, но учитываем старую)
        // Старая: 8000, новая: 3000 = 11000
        // Лимит: 100000 * 0.1 = 10000
        // Должна не пройти
        let result = validator.can_place_bet("Pari", "event-2", "League1", "Football", 3000.0, bankroll);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_exposure_validator_event_limit() {
        let limits = ExposureLimits {
            per_event_percent: 5.0,
            ..Default::default()
        };
        let mut validator = ExposureValidator::new(limits);
        let bankroll = 100000.0;
        
        // Ставка в 4000 - должна пройти (4% < 5%)
        let result = validator.can_place_bet("Pari", "event-1", "League1", "Football", 4000.0, bankroll);
        assert!(result.is_ok());
        
        validator.register_bet("Pari", "event-1", "League1", "Football", 4000.0);
        
        // Вторая ставка на тот же event в 2000 - должна не пройти (6% > 5%)
        let result = validator.can_place_bet("Fonbet", "event-1", "League1", "Football", 2000.0, bankroll);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_exposure_validator_league_limit() {
        let limits = ExposureLimits {
            per_league_percent: 15.0,
            ..Default::default()
        };
        let validator = ExposureValidator::new(limits);
        let bankroll = 100000.0;
        
        // Ставка в 12000 - должна пройти (12% < 15%)
        let result = validator.can_place_bet("Pari", "event-1", "EPL", "Football", 12000.0, bankroll);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_exposure_validator_reset() {
        let limits = ExposureLimits::default();
        let mut validator = ExposureValidator::new(limits);
        
        validator.register_bet("Pari", "event-1", "League1", "Football", 5000.0);
        assert_eq!(validator.get_tracker().get_bookmaker_exposure("Pari"), 5000.0);
        
        validator.reset();
        assert_eq!(validator.get_tracker().get_bookmaker_exposure("Pari"), 0.0);
    }
}
