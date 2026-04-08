use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max: u32,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout_secs: u64, half_open_max: u32) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            failure_threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout_secs),
            half_open_max,
        }
    }

    pub fn allow_request(&self) -> bool {
        let state = *self.state.read();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = *self.last_failure_time.read() {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        *self.state.write() = CircuitState::HalfOpen;
                        *self.success_count.write() = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        let state = *self.state.read();
        match state {
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.write();
                *success_count += 1;
                if *success_count >= self.half_open_max {
                    *self.state.write() = CircuitState::Closed;
                    *self.failure_count.write() = 0;
                }
            }
            CircuitState::Closed => {
                *self.failure_count.write() = 0;
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        *self.last_failure_time.write() = Some(Instant::now());
        let state = *self.state.read();

        match state {
            CircuitState::HalfOpen => {
                *self.state.write() = CircuitState::Open;
            }
            CircuitState::Closed => {
                let mut failures = self.failure_count.write();
                *failures += 1;
                if *failures >= self.failure_threshold {
                    *self.state.write() = CircuitState::Open;
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    pub fn failure_count(&self) -> u32 {
        *self.failure_count.read()
    }

    pub fn reset(&self) {
        *self.state.write() = CircuitState::Closed;
        *self.failure_count.write() = 0;
        *self.success_count.write() = 0;
        *self.last_failure_time.write() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 1, 1);
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_recovers_after_timeout() {
        let cb = CircuitBreaker::new(2, 0, 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        thread::sleep(Duration::from_millis(100));
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_reset() {
        let cb = CircuitBreaker::new(2, 60, 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }
}
