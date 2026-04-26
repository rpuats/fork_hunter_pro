use std::sync::Arc;

use crate::execution::BookmakerExecutionAdapter;
use crate::registry::ExecutionRegistry;

pub mod fonbet;
// Marathon adapter behind feature flag for MVP stability
#[cfg(feature = "marathon_mvp")]
pub mod marathon;

pub use fonbet::FonbetExecutionAdapter;
// MarathonExecutionAdapter removed in MVP
#[cfg(feature = "marathon_mvp")]
pub use marathon::MarathonExecutionAdapter;
pub use pari::PariExecutionAdapter;

pub fn builtin_adapter(bookmaker: &str) -> Option<Arc<dyn BookmakerExecutionAdapter>> {
    match bookmaker {
        PariExecutionAdapter::BOOKMAKER => Some(Arc::new(PariExecutionAdapter::default())),
        FonbetExecutionAdapter::BOOKMAKER => Some(Arc::new(FonbetExecutionAdapter::default())),
        #[cfg(feature = "marathon_mvp")]
        MarathonExecutionAdapter::BOOKMAKER => Some(Arc::new(MarathonExecutionAdapter::default())),
        // Marathon adapter removed in MVP
        _ => None,
    }
}

pub fn register_builtin_adapters(registry: &ExecutionRegistry) {
    registry.register_adapter(
        PariExecutionAdapter::BOOKMAKER,
        Arc::new(PariExecutionAdapter::default()),
    );
    registry.register_adapter(
        FonbetExecutionAdapter::BOOKMAKER,
        Arc::new(FonbetExecutionAdapter::default()),
    );
    #[cfg(feature = "marathon_mvp")]
    registry.register_adapter(
        MarathonExecutionAdapter::BOOKMAKER,
        Arc::new(MarathonExecutionAdapter::default()),
    );
}

pub fn supported_bookmakers() -> &'static [&'static str] {
    &[
        PariExecutionAdapter::BOOKMAKER,
        FonbetExecutionAdapter::BOOKMAKER,
        #[cfg(feature = "marathon_mvp")]
        MarathonExecutionAdapter::BOOKMAKER,
    ]
}
