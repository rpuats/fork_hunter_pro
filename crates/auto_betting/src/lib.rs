pub mod account_pool;
pub mod adapters;
pub mod approval;
pub mod auth;
pub mod bet_command;
pub mod bet_state_machine;
pub mod betting;
pub mod browser_pool;
pub mod engine;
pub mod execution;
pub mod executor;
pub mod limiter;
pub mod persistence;
pub mod registry;
pub mod state_machine;
pub mod stealth;
pub mod validator;

pub use execution::{
    ExecutionState, ForkExecution, ForkStatus, Fork, ForkLeg,
    AccountReadiness, BankrollPlan, StakeAllocation, StakingStrategy,
    DailyStats, GlobalLimits, ExecutionOrchestrator,
    ScannerBridge, spawn_scanner_bridge,
};

pub use account_pool::{
    AccountManager, AccountPool, AccountType, BettingAccount, PoolStatistics, SelectionStrategy,
};
pub use adapters::{
    builtin_adapter, register_builtin_adapters, supported_bookmakers, FonbetExecutionAdapter,
    PariExecutionAdapter,
};
pub use auth::{
    AuthError, AuthEvent, AuthManager, AuthStatus, BookmakerCredentials, Cookie, DisplaySettings,
    OddsFormat, SessionCookies, SessionStorage, TwoFAMethod,
    browser_auth::{
        authenticate_bookmaker, continue_after_2fa, continue_after_captcha,
    },
    display_config::{apply_display_config, get_display_config, BookmakerDisplayConfig, PostLoginAction},
    format_login, get_bookmaker_display_name, SUPPORTED_BOOKMAKERS,
    streaming_auth::streaming_authenticate,
};
pub use browser_pool::BrowserPool;
pub use betting::{
    BetInstruction, BetMode, BetResult, BetStatus, BettingEngine,
    auto_bet::place_auto_bet,
    manual_bet::prepare_manual_bet,
    operator_queue::{OperatorQueue, QueueItem, item_factory},
    runner::{BettingRunner, BettingRunnerConfig, BettingRunnerHandle, RunnerState, spawn_betting_runner},
    semi_auto_bet::{place_semi_auto_bet, OperatorEvent, OperatorResponse},
};
pub use approval::{
    build_surebet_execution_plan, ApprovalGateDecision, RankedLegPlan, SurebetExecutionPlan,
    PARI_ROLLOUT_BOOKMAKER,
};
pub use bet_command::{BetCommandStatus, PlaceBeautifulBetCommand};
pub use bet_state_machine::{BetPlacementEvent, BetPlacementState, BetPlacementStateMachine};
pub use engine::AutoBetEngine;
pub use execution::{BookmakerExecutionAdapter, NoopExecutionAdapter};
pub use executor::BetExecutor;
pub use limiter::BetLimiter;
pub use persistence::{
    ExecutionLedgerAction, ExecutionLedgerEntry, ExecutionLedgerPersistence,
    ExecutionRegistryPersistence, ExecutionRegistrySnapshot, ExecutionStatePersistence,
};
pub use registry::ExecutionRegistry;
pub use state_machine::{
    ExecutionStateMachine, ExecutionStatePhase, ExecutionStateReplay, ExecutionStateSnapshot,
    ExecutionStateTransition,
};
pub use stealth::StealthBetting;
pub use validator::StakeValidator;
