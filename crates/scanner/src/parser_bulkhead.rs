use std::sync::Arc;

use shared::config::{RuntimeProfile, ScannerConfig};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug)]
pub struct ParserExecutionBulkhead {
    semaphore: Arc<Semaphore>,
    max_parallelism: usize,
    strict_mode: bool,
}

impl ParserExecutionBulkhead {
    pub fn from_runtime_defaults(profile: RuntimeProfile, parser_count: usize) -> Self {
        let dev_parallelism = parser_count.max(1);
        let production_parallelism = Some(dev_parallelism.min(2).max(1));

        Self::new(profile, dev_parallelism, production_parallelism)
    }

    pub fn from_config(profile: RuntimeProfile, scanner_config: &ScannerConfig) -> Self {
        Self::new(
            profile,
            scanner_config.parallel_parsers,
            scanner_config.production_parallel_parsers,
        )
    }

    pub fn new(
        profile: RuntimeProfile,
        parallel_parsers: usize,
        production_parallel_parsers: Option<usize>,
    ) -> Self {
        let scanner_config = ScannerConfig {
            parallel_parsers,
            production_parallel_parsers,
            ..ScannerConfig::default()
        };
        let max_parallelism = scanner_config.parser_execution_parallelism(profile);
        let strict_mode = scanner_config.parser_execution_strict_mode(profile);

        Self {
            semaphore: Arc::new(Semaphore::new(max_parallelism)),
            max_parallelism,
            strict_mode,
        }
    }

    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("parser execution semaphore must remain open")
    }

    pub fn max_parallelism(&self) -> usize {
        self.max_parallelism
    }

    pub fn strict_mode(&self) -> bool {
        self.strict_mode
    }
}

impl Clone for ParserExecutionBulkhead {
    fn clone(&self) -> Self {
        Self {
            semaphore: Arc::clone(&self.semaphore),
            max_parallelism: self.max_parallelism,
            strict_mode: self.strict_mode,
        }
    }
}
