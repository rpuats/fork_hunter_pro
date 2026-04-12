pub mod allocation;
pub mod kelly;
pub mod manager;
pub mod rebalance;

pub use allocation::DepositAllocator;
pub use kelly::KellyCalculator;
pub use manager::BankrollManager;
pub use rebalance::RebalanceEngine;
