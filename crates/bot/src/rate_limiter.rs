use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Token bucket rate limiter for alert throttling
/// Ensures max N alerts per time period
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
    capacity: f64,
    refill_per_second: f64,
}

struct RateLimiterState {
    tokens: f64,
    last_refill_at: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    /// # Arguments
    /// * `capacity` - max tokens (e.g., 10 for max 10 alerts)
    /// * `refill_per_second` - tokens added per second (e.g., 10/60 = 1 token per 6 seconds)
    pub fn new(capacity: f64, refill_per_second: f64) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: capacity,
                last_refill_at: Instant::now(),
            })),
            capacity,
            refill_per_second,
        }
    }

    /// Create limiter for max N alerts per minute
    pub fn alerts_per_minute(max_alerts: f64) -> Self {
        Self::new(max_alerts, max_alerts / 60.0)
    }

    /// Try to consume one token; returns true if successful
    pub fn try_consume(&self, tokens_needed: f64) -> bool {
        let mut state = self.state.lock();
        self.refill(&mut state);

        if state.tokens >= tokens_needed {
            state.tokens -= tokens_needed;
            true
        } else {
            false
        }
    }

    /// Get current token count
    pub fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock();
        self.refill(&mut state);
        state.tokens
    }

    /// Reset the limiter (full tokens)
    pub fn reset(&self) {
        let mut state = self.state.lock();
        state.tokens = self.capacity;
        state.last_refill_at = Instant::now();
    }

    /// Get stats for monitoring
    pub fn stats(&self) -> RateLimiterStats {
        let state = self.state.lock();
        RateLimiterStats {
            available_tokens: state.tokens,
            capacity: self.capacity,
            refill_per_second: self.refill_per_second,
        }
    }

    fn refill(&self, state: &mut RateLimiterState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill_at).as_secs_f64();
        let tokens_to_add = elapsed * self.refill_per_second;

        state.tokens = (state.tokens + tokens_to_add).min(self.capacity);
        state.last_refill_at = now;
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub available_tokens: f64,
    pub capacity: f64,
    pub refill_per_second: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn limiter_starts_with_full_capacity() {
        let limiter = RateLimiter::new(10.0, 1.0);
        assert_eq!(limiter.available_tokens(), 10.0);
    }

    #[test]
    fn consume_single_token() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        assert!(limiter.try_consume(1.0));
        assert!((limiter.available_tokens() - 9.0).abs() < 0.01);
    }

    #[test]
    fn reject_when_no_tokens() {
        let limiter = RateLimiter::new(1.0, 0.0);
        assert!(limiter.try_consume(1.0));
        assert!(!limiter.try_consume(1.0));
    }

    #[test]
    fn tokens_refill_over_time() {
        let limiter = RateLimiter::new(10.0, 2.0); // 2 tokens per second
        assert!(limiter.try_consume(10.0));
        assert_eq!(limiter.available_tokens(), 0.0);

        thread::sleep(Duration::from_millis(500));
        let after_half_second = limiter.available_tokens();
        assert!(after_half_second >= 0.9 && after_half_second <= 1.1); // ~1 token

        thread::sleep(Duration::from_millis(500));
        let after_one_second = limiter.available_tokens();
        assert!(after_one_second >= 1.9 && after_one_second <= 2.1); // ~2 tokens
    }

    #[test]
    fn alerts_per_minute_config() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        assert_eq!(limiter.capacity, 10.0);
        assert!((limiter.refill_per_second - 10.0 / 60.0).abs() < 0.001);
    }

    #[test]
    fn reset_restores_capacity() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        limiter.try_consume(10.0);
        assert_eq!(limiter.available_tokens(), 0.0);

        limiter.reset();
        assert_eq!(limiter.available_tokens(), 10.0);
    }

    #[test]
    fn consume_partial_tokens() {
        let limiter = RateLimiter::new(10.0, 1.0);
        assert!(limiter.try_consume(2.5));
        assert!((limiter.available_tokens() - 7.5).abs() < 0.01);
    }

    #[test]
    fn stats_reflect_current_state() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        let stats = limiter.stats();
        assert_eq!(stats.capacity, 10.0);
        assert_eq!(stats.available_tokens, 10.0);
        assert!((stats.refill_per_second - 10.0 / 60.0).abs() < 0.001);
    }
}
