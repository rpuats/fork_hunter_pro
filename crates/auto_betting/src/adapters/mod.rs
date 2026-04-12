use std::sync::Arc;

use crate::execution::BookmakerExecutionAdapter;
use crate::registry::ExecutionRegistry;

pub mod fonbet;
pub mod pari;

pub use fonbet::FonbetExecutionAdapter;
pub use pari::PariExecutionAdapter;

pub fn builtin_adapter(bookmaker: &str) -> Option<Arc<dyn BookmakerExecutionAdapter>> {
    match bookmaker {
        PariExecutionAdapter::BOOKMAKER => Some(Arc::new(PariExecutionAdapter::default())),
        FonbetExecutionAdapter::BOOKMAKER => Some(Arc::new(FonbetExecutionAdapter::default())),
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
}

pub fn supported_bookmakers() -> &'static [&'static str] {
    &[
        PariExecutionAdapter::BOOKMAKER,
        FonbetExecutionAdapter::BOOKMAKER,
    ]
}
