use shared::{BetPlacement, Surebet};
use tracing::{error, info};

use super::engine::AutoBetEngine;
use super::stealth::StealthBetting;

pub struct BetExecutor {
    engine: AutoBetEngine,
    stealth: StealthBetting,
}

impl BetExecutor {
    pub fn new(engine: AutoBetEngine) -> Self {
        Self {
            engine,
            stealth: StealthBetting::new(),
        }
    }

    pub async fn execute_surebet(&self, surebet: &Surebet) -> Result<Vec<BetPlacement>, String> {
        info!(surebet_id = surebet.id.to_string(), "Executing surebet");

        self.stealth.wait_stealth().await;

        match self.engine.place_surebet(surebet).await {
            Ok(placements) => {
                info!(count = placements.len(), "Surebet executed successfully");
                Ok(placements)
            }
            Err(e) => {
                error!(error = e.to_string(), "Surebet execution failed");
                Err(e)
            }
        }
    }

    pub async fn execute_batch(
        &self,
        surebets: &[Surebet],
    ) -> Vec<Result<Vec<BetPlacement>, String>> {
        let mut results = Vec::new();

        for surebet in surebets {
            let result = self.execute_surebet(surebet).await;
            results.push(result);

            self.stealth.wait_stealth().await;
        }

        results
    }

    pub fn get_engine(&self) -> &AutoBetEngine {
        &self.engine
    }
}
