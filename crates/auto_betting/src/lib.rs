pub mod adapters;
pub mod engine;
pub mod execution;
pub mod executor;
pub mod limiter;
pub mod persistence;
pub mod registry;
pub mod stealth;
pub mod validator;

pub use adapters::{
    builtin_adapter, register_builtin_adapters, supported_bookmakers, FonbetExecutionAdapter,
    PariExecutionAdapter,
};
pub use engine::AutoBetEngine;
pub use execution::{BookmakerExecutionAdapter, NoopExecutionAdapter};
pub use executor::BetExecutor;
pub use limiter::BetLimiter;
pub use persistence::{ExecutionRegistryPersistence, ExecutionRegistrySnapshot};
pub use registry::ExecutionRegistry;
pub use stealth::StealthBetting;
pub use validator::StakeValidator;
