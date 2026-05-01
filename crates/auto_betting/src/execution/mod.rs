//! Execution module - State management and orchestration

pub mod execution_state;
pub mod scanner_bridge;

pub use execution_state::{
    ExecutionState, ForkExecution, ForkStatus, Fork, ForkLeg,
    AccountReadiness, BankrollPlan, StakeAllocation, StakingStrategy,
    DailyStats, GlobalLimits, ExecutionOrchestrator,
};
pub use scanner_bridge::{ScannerBridge, spawn_scanner_bridge};
