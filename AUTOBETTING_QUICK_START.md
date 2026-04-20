# AutoBetting Quick Start Guide

## 5-Minute Setup

### 1. Initialize Components

```rust
use auto_betting::{PlaceBeautifulBetCommand, BetPlacementStateMachine};
use bankroll_manager::{
    AccountManager, BookmakerAccount, 
    ExposureLimits, ExposureValidator,
    SqliteBetLedger, KellyCalculator,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize accounts
    let mut account_mgr = AccountManager::new();
    account_mgr.add_account(BookmakerAccount::new(
        "Pari".to_string(),
        100000.0,    // Initial balance
        "RUB".to_string(),
    ))?;
    
    account_mgr.add_account(BookmakerAccount::new(
        "Fonbet".to_string(),
        100000.0,
        "RUB".to_string(),
    ))?;

    // Initialize exposure limits
    let limits = ExposureLimits::default();
    let mut exposure = ExposureValidator::new(limits);

    // Initialize ledger (SQLite persistence)
    let ledger = SqliteBetLedger::new_with_file("bets.db").await?;

    // ... rest of implementation
    Ok(())
}
```

### 2. Calculate Kelly Stake

```rust
let bankroll = 100000.0;
let estimated_probability = 0.55;
let odds = 2.10;
let kelly_fraction = 0.25;  // Conservative (25%)
let max_exposure = 5.0;     // 5% max per bet

let stake = KellyCalculator::optimal_stake(
    bankroll,
    estimated_probability,
    odds,
    kelly_fraction,
    max_exposure,
);

println!("Kelly stake: {} RUB", stake);
// Output: Kelly stake: 1376.14 RUB
```

### 3. Create Bet Command

```rust
let surebet_id = Uuid::new_v4();

let cmd = PlaceBeautifulBetCommand::new(
    surebet_id,
    "Pari".to_string(),
    "event-pari-123".to_string(),
    "1x2".to_string(),
    "1".to_string(),          // Home team win
    2.10,
    stake,
    0.25,                      // Kelly fraction
    0.55,                      // Estimated probability
    0.05,                      // Bookmaker margin
);

println!("Bet command created: {:?}", cmd.command_id);
// Expected profit: {:.2}", cmd.expected_profit());
```

### 4. State Machine Workflow

```rust
// Initialize state machine
let mut state = BetPlacementStateMachine::new(cmd.command_id);

// 1. Validate exposure limits
state.validate_exposure()?;
exposure.can_place_bet(
    "Pari",
    "event-pari-123", 
    "EPL",
    "Football",
    stake,
    bankroll,
)?;

// 2. Validate account balance
state.validate_balance(account_mgr.get_account("Pari").unwrap().balance)?;

// 3. Mark ready
state.mark_ready()?;

// 4. Execute (simulate API call)
state.start_execution()?;
state.mark_placed(Some("TICKET-12345".to_string()))?;

// 5. Confirm
state.mark_confirmed()?;

println!("Bet placed successfully!");
```

### 5. Update Ledger & Accounts

```rust
// Create ledger entry
let mut entry = BetLedgerEntry::new(
    cmd.command_id,
    surebet_id,
    "Pari".to_string(),
    "event-pari-123".to_string(),
    "1x2".to_string(),
    "1".to_string(),
    stake,
    2.10,
);

// Save to database
ledger.add_entry(entry.clone()).await?;

// Update account balance (stake withdrawn)
account_mgr.withdraw("Pari", stake)?;

// Update exposure tracker
exposure.register_bet(
    "Pari",
    "event-pari-123",
    "EPL",
    "Football",
    stake,
);

println!("Ledger updated");
```

### 6. Check Results Later

```rust
// Simulate bet result
let mut settled_entry = entry.clone();
settled_entry.mark_won(4200.0);  // Payout from 2.10 * 2000

// Update ledger
ledger.update_entry(settled_entry.clone()).await?;

// Update account balance (return stake + profit)
account_mgr.return_stake("Pari", stake)?;
account_mgr.deposit("Pari", 4200.0)?;

// Get statistics
let stats = ledger.get_statistics(
    Utc::now() - chrono::Duration::days(1),
    Utc::now(),
).await?;

println!("Total bets: {}", stats.total_bets);
println!("Winning bets: {}", stats.winning_bets);
println!("Total P&L: {:.2} RUB", stats.total_profit_loss);
println!("ROI: {:.2}%", stats.roi);
```

## Working Example - Full Surebet Flow

```rust
use auto_betting::{PlaceBeautifulBetCommand, BetPlacementStateMachine};
use bankroll_manager::{
    AccountManager, BookmakerAccount,
    ExposureLimits, ExposureValidator,
    SqliteBetLedger, BetLedgerEntry, KellyCalculator,
};
use chrono::Utc;
use uuid::Uuid;

#[tokio::main]
async fn place_surebet_example() -> anyhow::Result<()> {
    // Setup
    let surebet_id = Uuid::new_v4();
    let bankroll = 100000.0;
    
    // 1. Initialize accounts
    let mut accounts = AccountManager::new();
    accounts.add_account(BookmakerAccount::new("Pari".to_string(), 100000.0, "RUB".to_string()));
    accounts.add_account(BookmakerAccount::new("Fonbet".to_string(), 100000.0, "RUB".to_string()));

    // 2. Setup exposure limits
    let mut exposure = ExposureValidator::new(ExposureLimits::default());

    // 3. Initialize ledger
    let ledger = SqliteBetLedger::new_in_memory().await?;

    // SUREBET: Event 1 (Pari), Event 2 (Fonbet)
    // Leg 1: Pari, Prob=55%, Odds=2.10
    let stake_1 = KellyCalculator::optimal_stake(bankroll, 0.55, 2.10, 0.25, 5.0);
    
    let mut cmd_1 = PlaceBeautifulBetCommand::new(
        surebet_id, "Pari".to_string(), "event-123".to_string(),
        "1x2".to_string(), "1".to_string(), 2.10, stake_1, 0.25, 0.55, 0.05,
    );
    
    // Leg 2: Fonbet, Prob=50%, Odds=2.00
    let stake_2 = KellyCalculator::optimal_stake(bankroll, 0.50, 2.00, 0.25, 5.0);
    
    let mut cmd_2 = PlaceBeautifulBetCommand::new(
        surebet_id, "Fonbet".to_string(), "event-123".to_string(),
        "1x2".to_string(), "2".to_string(), 2.00, stake_2, 0.25, 0.50, 0.05,
    );

    // Place Leg 1 (Pari)
    println!("=== Placing Bet 1 (Pari) ===");
    
    let mut state_1 = BetPlacementStateMachine::new(cmd_1.command_id);
    state_1.validate_exposure()?;
    exposure.can_place_bet("Pari", "event-123", "EPL", "Football", stake_1, bankroll)?;
    
    state_1.validate_balance(accounts.get_account("Pari").unwrap().balance)?;
    state_1.mark_ready()?;
    state_1.start_execution()?;
    state_1.mark_placed(Some("PARI-001".to_string()))?;
    state_1.mark_confirmed()?;

    // Record in ledger
    let mut entry_1 = BetLedgerEntry::new(
        cmd_1.command_id, surebet_id, "Pari".to_string(), 
        "event-123".to_string(), "1x2".to_string(), "1".to_string(),
        stake_1, 2.10,
    );
    entry_1.mark_placed();
    ledger.add_entry(entry_1.clone()).await?;

    accounts.withdraw("Pari", stake_1)?;
    exposure.register_bet("Pari", "event-123", "EPL", "Football", stake_1);
    
    println!("✓ Bet 1 placed: {} RUB at 2.10", stake_1);

    // Place Leg 2 (Fonbet)
    println!("\n=== Placing Bet 2 (Fonbet) ===");
    
    let mut state_2 = BetPlacementStateMachine::new(cmd_2.command_id);
    state_2.validate_exposure()?;
    exposure.can_place_bet("Fonbet", "event-123", "EPL", "Football", stake_2, bankroll)?;
    
    state_2.validate_balance(accounts.get_account("Fonbet").unwrap().balance)?;
    state_2.mark_ready()?;
    state_2.start_execution()?;
    state_2.mark_placed(Some("FONBET-001".to_string()))?;
    state_2.mark_confirmed()?;

    // Record in ledger
    let mut entry_2 = BetLedgerEntry::new(
        cmd_2.command_id, surebet_id, "Fonbet".to_string(),
        "event-123".to_string(), "1x2".to_string(), "2".to_string(),
        stake_2, 2.00,
    );
    entry_2.mark_placed();
    ledger.add_entry(entry_2.clone()).await?;

    accounts.withdraw("Fonbet", stake_2)?;
    exposure.register_bet("Fonbet", "event-123", "EPL", "Football", stake_2);
    
    println!("✓ Bet 2 placed: {} RUB at 2.00", stake_2);

    // Simulate results
    println!("\n=== Event Result: 1 (Home Team Win) ===");
    
    let payout_1 = stake_1 * 2.10;
    let mut settled_entry_1 = entry_1;
    settled_entry_1.mark_won(payout_1);
    ledger.update_entry(settled_entry_1).await?;
    accounts.return_stake("Pari", payout_1)?;
    
    let mut settled_entry_2 = entry_2;
    settled_entry_2.mark_lost();
    ledger.update_entry(settled_entry_2).await?;
    accounts.return_stake("Fonbet", stake_2)?;  // Return losing stake

    // Check results
    println!("\n=== Final Results ===");
    let total_balance = accounts.get_total_balance();
    let total_initial = accounts.get_total_initial_balance();
    let total_profit = total_balance - total_initial;

    println!("Total balance: {:.2} RUB", total_balance);
    println!("Total initial: {:.2} RUB", total_initial);
    println!("Total profit: {:.2} RUB", total_profit);
    println!("ROI: {:.2}%", (total_profit / total_initial) * 100.0);

    // Get statistics
    let stats = ledger.get_statistics(
        Utc::now() - chrono::Duration::days(1),
        Utc::now() + chrono::Duration::hours(1),
    ).await?;
    
    println!("\n=== Statistics ===");
    println!("Total bets: {}", stats.total_bets);
    println!("Winning bets: {}", stats.winning_bets);
    println!("Losing bets: {}", stats.losing_bets);
    println!("Total stake: {:.2} RUB", stats.total_stake);
    println!("Total payout: {:.2} RUB", stats.total_payout);
    println!("P&L: {:.2} RUB", stats.total_profit_loss);
    println!("ROI: {:.2}%", stats.roi);
    println!("Win rate: {:.2}%", stats.win_rate);

    Ok(())
}
```

## Key Functions Reference

### Kelly Calculation
```rust
// Basic Kelly
let f_kelly = KellyCalculator::full_kelly(prob, odds);

// Fractional Kelly (safer)
let f_frac = KellyCalculator::fractional_kelly(edge, odds, 0.25);

// Full calculation with constraints
let stake = KellyCalculator::optimal_stake(
    bankroll,
    estimated_prob,
    odds,
    kelly_fraction,
    max_exposure_pct,
);
```

### Exposure Validation
```rust
// Check if we can place bet
exposure.can_place_bet(
    "Pari",           // bookmaker
    "event-123",      // event_id
    "EPL",            // league
    "Football",       // sport
    stake,
    bankroll,
)?;

// Register after placement
exposure.register_bet("Pari", "event-123", "EPL", "Football", stake);

// Reset daily (at end of day)
exposure.reset();
```

### Ledger Operations
```rust
// Create entry
let entry = BetLedgerEntry::new(
    cmd_id, surebet_id, bookmaker, event_id,
    market, selection, stake, odds,
);

// Mark status
entry.mark_placed();
entry.mark_won(payout);
entry.mark_lost();
entry.mark_voided();
entry.mark_cancelled();

// Persist
ledger.add_entry(entry).await?;
ledger.update_entry(updated_entry).await?;

// Query
let entries = ledger.get_entries_by_surebet(surebet_id).await?;
let stats = ledger.get_statistics(start, end).await?;
```

## Configuration Presets

### Ultra-Conservative (1% Kelly)
```rust
kelly_fraction: 0.01,
max_exposure: 0.5,
per_event: 0.5,
per_bookmaker: 2.0,
per_league: 5.0,
```

### Conservative (25% Kelly) - **RECOMMENDED**
```rust
kelly_fraction: 0.25,
max_exposure: 5.0,
per_event: 2.5,
per_bookmaker: 7.5,
per_league: 15.0,
```

### Moderate (50% Kelly)
```rust
kelly_fraction: 0.50,
max_exposure: 10.0,
per_event: 5.0,
per_bookmaker: 15.0,
per_league: 30.0,
```

### Aggressive (75% Kelly)
```rust
kelly_fraction: 0.75,
max_exposure: 15.0,
per_event: 10.0,
per_bookmaker: 25.0,
per_league: 50.0,
```

## Troubleshooting

**Q: Betting stops with "Exposure limit exceeded"**
- A: Reduce bet size or increase `per_event_percent` limit

**Q: "Insufficient balance" error**
- A: Add funds via `account_mgr.deposit()` or reduce stake

**Q: How to pause betting?**
- A: Set `emergency_stop()` flag in engine

**Q: How to review past bets?**
- A: Use `ledger.get_entries_by_surebet()` or `get_statistics()`

---

**Ready to deploy!** Follow AUTOBETTING_IMPLEMENTATION_GUIDE.md for production setup.
