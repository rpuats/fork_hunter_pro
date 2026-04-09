use crate::engine::{GhostScanner, ScannerState};
use shared::models::ScannerMetrics;
use shared::{CorridorOpportunity, ExpressFork};
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{info, warn};

#[derive(Clone)]
pub struct ScannerRunner {
    scanner: Arc<GhostScanner>,
    cached_state: Arc<Mutex<Option<ScannerState>>>,
}

impl ScannerRunner {
    pub fn new(scanner: Arc<GhostScanner>) -> Self {
        Self { 
            scanner,
            cached_state: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) {
        self.refresh_cached_state();

        let mut state_rx = self.scanner.state_tx.subscribe();
        let cached_state = Arc::clone(&self.cached_state);
        tokio::spawn(async move {
            loop {
                if state_rx.changed().await.is_err() {
                    break;
                }

                let state = state_rx.borrow_and_update().clone();
                match cached_state.lock() {
                    Ok(mut locked) => *locked = Some(state),
                    Err(_) => warn!("ScannerRunner cache mutex poisoned while updating state"),
                }
            }
        });

        info!("ScannerRunner started");
        self.scanner.start().await;
    }

    pub fn stop(&self) {
        self.scanner.stop();
        info!("ScannerRunner stopped");
    }

    pub fn get_state(&self) -> ScannerState {
        if let Ok(locked) = self.cached_state.try_lock() {
            if let Some(state) = locked.as_ref() {
                return state.clone();
            }
        }

        ScannerState {
            running: self.scanner.is_running(),
            last_metrics: None,
            cycle_count: 0,
        }
    }

    pub fn get_metrics(&self) -> Option<ScannerMetrics> {
        if let Ok(locked) = self.cached_state.try_lock() {
            if let Some(state) = locked.as_ref() {
                return state.last_metrics.clone();
            }
        }

        None
    }

    fn refresh_cached_state(&self) {
        let state = self.scanner.state_rx.borrow().clone();
        match self.cached_state.lock() {
            Ok(mut locked) => *locked = Some(state),
            Err(_) => warn!("ScannerRunner cache mutex poisoned while refreshing state"),
        }
    }

    pub fn get_corridors(&self, limit: usize) -> Vec<CorridorOpportunity> {
        self.scanner.get_corridors(limit)
    }

    pub fn get_express_forks(&self, limit: usize) -> Vec<ExpressFork> {
        self.scanner.get_express_forks(limit)
    }
}
