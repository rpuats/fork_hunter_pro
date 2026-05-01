//! Operator Queue - Central queue for all operator actions

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Queue item types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum QueueItem {
    /// Bet awaiting confirmation
    BetConfirmation(BetConfirmationItem),
    
    /// Captcha required
    CaptchaRequired(CaptchaItem),
    
    /// 2FA required
    TwoFARequired(TwoFAItem),
    
    /// Auth required
    AuthRequired(AuthItem),
    
    /// Odds changed notification
    OddsChanged(OddsChangedItem),
    
    /// Low balance warning
    BalanceLow(BalanceLowItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetConfirmationItem {
    pub id: String,
    pub bet_id: String,
    pub fork_id: Uuid,
    pub bookmaker: String,
    pub event: String,
    pub market: String,
    pub selection: String,
    pub odds: Decimal,
    pub stake: Decimal,
    pub expected_odds: Decimal,
    pub screenshot_base64: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaItem {
    pub id: String,
    pub bookmaker: String,
    pub image_base64: String,
    pub attempt: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFAItem {
    pub id: String,
    pub bookmaker: String,
    pub method: String, // "sms", "totp", "email"
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthItem {
    pub id: String,
    pub bookmaker: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OddsChangedItem {
    pub id: String,
    pub fork_id: Uuid,
    pub bookmaker: String,
    pub old_odds: Decimal,
    pub new_odds: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceLowItem {
    pub id: String,
    pub bookmaker: String,
    pub balance: Decimal,
    pub threshold: Decimal,
    pub created_at: DateTime<Utc>,
}

impl QueueItem {
    pub fn id(&self) -> &str {
        match self {
            QueueItem::BetConfirmation(item) => &item.id,
            QueueItem::CaptchaRequired(item) => &item.id,
            QueueItem::TwoFARequired(item) => &item.id,
            QueueItem::AuthRequired(item) => &item.id,
            QueueItem::OddsChanged(item) => &item.id,
            QueueItem::BalanceLow(item) => &item.id,
        }
    }

    pub fn bookmaker(&self) -> &str {
        match self {
            QueueItem::BetConfirmation(item) => &item.bookmaker,
            QueueItem::CaptchaRequired(item) => &item.bookmaker,
            QueueItem::TwoFARequired(item) => &item.bookmaker,
            QueueItem::AuthRequired(item) => &item.bookmaker,
            QueueItem::OddsChanged(item) => &item.bookmaker,
            QueueItem::BalanceLow(item) => &item.bookmaker,
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            QueueItem::BetConfirmation(item) => item.created_at,
            QueueItem::CaptchaRequired(item) => item.created_at,
            QueueItem::TwoFARequired(item) => item.created_at,
            QueueItem::AuthRequired(item) => item.created_at,
            QueueItem::OddsChanged(item) => item.created_at,
            QueueItem::BalanceLow(item) => item.created_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        match self {
            QueueItem::BetConfirmation(item) => now > item.expires_at,
            _ => false,
        }
    }
}

/// Operator queue
pub struct OperatorQueue {
    items: VecDeque<QueueItem>,
    current: Option<QueueItem>,
    max_size: usize,
}

impl OperatorQueue {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            current: None,
            max_size: 100,
        }
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            items: VecDeque::new(),
            current: None,
            max_size,
        }
    }

    /// Add item to queue
    pub fn push(&mut self, item: QueueItem) {
        if self.items.len() >= self.max_size {
            self.items.pop_back();
        }
        self.items.push_front(item);
    }

    /// Get next item
    pub fn next(&mut self) -> Option<QueueItem> {
        // Remove expired items
        while let Some(item) = self.items.back() {
            if item.is_expired() {
                self.items.pop_back();
            } else {
                break;
            }
        }

        // Set current
        self.current = self.items.pop_back();
        self.current.clone()
    }

    /// Get current item
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.as_ref()
    }

    /// Resolve current item
    pub fn resolve_current(&mut self, resolved: bool) {
        if resolved {
            self.current = None;
        }
    }

    /// Remove item by ID
    pub fn remove(&mut self, id: &str) -> Option<QueueItem> {
        if let Some(pos) = self.items.iter().position(|item| item.id() == id) {
            self.items.remove(pos)
        } else {
            None
        }
    }

    /// Get all items
    pub fn items(&self) -> &VecDeque<QueueItem> {
        &self.items
    }

    /// Get non-expired items count
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|item| !item.is_expired()).count()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
    }
}

impl Default for OperatorQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Create queue item helpers
pub mod item_factory {
    use super::*;
    use uuid::Uuid;

    pub fn bet_confirmation(
        bet_id: String,
        fork_id: Uuid,
        bookmaker: String,
        event: String,
        market: String,
        selection: String,
        odds: Decimal,
        stake: Decimal,
        expected_odds: Decimal,
        screenshot_base64: Option<String>,
        expires_in_secs: i64,
    ) -> QueueItem {
        QueueItem::BetConfirmation(BetConfirmationItem {
            id: Uuid::new_v4().to_string(),
            bet_id,
            fork_id,
            bookmaker,
            event,
            market,
            selection,
            odds,
            stake,
            expected_odds,
            screenshot_base64,
            expires_at: Utc::now() + chrono::Duration::seconds(expires_in_secs),
            created_at: Utc::now(),
        })
    }

    pub fn captcha_required(
        bookmaker: String,
        image_base64: String,
        attempt: u32,
    ) -> QueueItem {
        QueueItem::CaptchaRequired(CaptchaItem {
            id: Uuid::new_v4().to_string(),
            bookmaker,
            image_base64,
            attempt,
            created_at: Utc::now(),
        })
    }

    pub fn two_fa_required(
        bookmaker: String,
        method: String,
    ) -> QueueItem {
        QueueItem::TwoFARequired(TwoFAItem {
            id: Uuid::new_v4().to_string(),
            bookmaker,
            method,
            created_at: Utc::now(),
        })
    }

    pub fn odds_changed(
        fork_id: Uuid,
        bookmaker: String,
        old_odds: Decimal,
        new_odds: Decimal,
    ) -> QueueItem {
        QueueItem::OddsChanged(OddsChangedItem {
            id: Uuid::new_v4().to_string(),
            fork_id,
            bookmaker,
            old_odds,
            new_odds,
            created_at: Utc::now(),
        })
    }

    pub fn balance_low(
        bookmaker: String,
        balance: Decimal,
        threshold: Decimal,
    ) -> QueueItem {
        QueueItem::BalanceLow(BalanceLowItem {
            id: Uuid::new_v4().to_string(),
            bookmaker,
            balance,
            threshold,
            created_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_push_and_next() {
        let mut queue = OperatorQueue::new();
        
        let item = item_factory::captcha_required(
            "pari".to_string(),
            "base64data".to_string(),
            1,
        );
        
        queue.push(item.clone());
        assert_eq!(queue.pending_count(), 1);
        
        let next = queue.next();
        assert!(next.is_some());
        assert_eq!(queue.current().unwrap().bookmaker(), "pari");
    }

    #[test]
    fn test_queue_remove() {
        let mut queue = OperatorQueue::new();
        let item = item_factory::captcha_required(
            "pari".to_string(),
            "base64data".to_string(),
            1,
        );
        let id = item.id().to_string();
        
        queue.push(item);
        assert_eq!(queue.pending_count(), 1);
        
        queue.remove(&id);
        assert_eq!(queue.pending_count(), 0);
    }
}
