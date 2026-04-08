use crate::engine::{GhostScanner, ScannerState};
use shared::models::ScannerMetrics;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct ScannerRunner {
    scanner: Arc<GhostScanner>,
}

impl ScannerRunner {
    pub fn new(scanner: Arc<GhostScanner>) -> Self {
        Self { scanner }
    }

    pub async fn start(&self) {
        info!("ScannerRunner started");
        self.scanner.start().await;
    }

    pub fn stop(&self) {
        self.scanner.stop();
        info!("ScannerRunner stopped");
    }

    pub fn get_state(&self) -> ScannerState {
        let state = self.scanner.state_rx.borrow().clone();
        state
    }

    pub fn get_metrics(&self) -> Option<ScannerMetrics> {
        self.scanner.state_rx.borrow().last_metrics.clone()
    }
}
