# EXPRESS-FORKS CODE CHANGES - DETAILED BREAKDOWN

## File 1: crates/express_forks/src/calculator.rs

### Changes Summary
- **Lines Added**: ~334 lines total
- **New Structures**: MultiLegOptimizer, OptimizedLeg
- **New Methods**: 7 new public methods
- **New Tests**: 19 comprehensive test cases

### New Imports
```rust
use std::collections::{HashMap, HashSet};
```

### New Structures (82 lines)

#### 1. OptimizedLeg (9 lines)
```rust
#[derive(Clone, Debug)]
pub struct OptimizedLeg {
    pub event_id: String,
    pub best_odds: f64,
    pub best_bookmaker: String,
    pub market: String,
    pub selection: String,
    pub available_in_bks: Vec<String>,
}
```
**Purpose**: Represents a single leg with best available odds across all BKs

#### 2. MultiLegOptimizer (73 lines)
```rust
pub struct MultiLegOptimizer {
    min_legs: usize,
    max_legs: usize,
    min_roi_3plus_legs: f64,
}

impl MultiLegOptimizer {
    pub fn new(min_legs: usize, max_legs: usize, min_roi_3plus_legs: f64) -> Self
    pub fn optimize_legs(&self, events_odds: &HashMap<String, Vec<&Odd>>) -> HashMap<String, OptimizedLeg>
    pub fn calculate_roi(&self, legs_count: usize, express_odds: f64, lay_total: f64) -> f64
    pub fn roi_meets_threshold(&self, legs_count: usize, roi: f64) -> bool
    pub fn validate_leg_availability(&self, legs: &[OptimizedLeg]) -> bool
}
```
**Purpose**: Core optimization logic for multi-leg forks

### Enhanced ExpressForkCalculator

#### Constructor Changes
```rust
// Old
pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self

// New (keeps old, adds new method)
pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self
pub fn new_with_optimizer(max_legs: usize, min_profit: f64, default_stake: f64, min_roi_3plus: f64) -> Self
```

#### find_express_forks Enhanced
```rust
// OLD: Simple odds collection
// NEW: Uses MultiLegOptimizer.optimize_legs() first

pub fn find_express_forks(&self, events: &[Event], all_odds: &[Odd]) -> Vec<ExpressFork> {
    // 1. Build odds_by_event map
    // 2. Run optimizer.optimize_legs() to get best odds per event
    // 3. For each leg count 2..max_legs:
    //    - For each combination of that size:
    //      - Call try_express_combo with optimized legs
    // 4. Filter by min_profit
    // 5. Sort by ROI descending
}
```

#### try_express_combo Complete Rewrite
```rust
// OLD: 2-leg only, simple calculation
// NEW: N-leg support with:
// - Multi-leg validation
// - Cascade odds calculation
// - Per-leg BK optimization
// - ROI thresholding
// - Risk level assignment

fn try_express_combo(
    &self,
    event_ids: Vec<&&String>,
    odds_by_event: &HashMap<String, Vec<&Odd>>,
    optimized_legs: &HashMap<String, OptimizedLeg>,  // NEW
    events: &[Event],
    leg_count: usize,  // NEW
) -> Option<ExpressFork>
```

**Key Algorithm Changes**:
1. **Leg Validation**: Check all legs are available in 2+ BKs
2. **Cascade Calculation**: Express = prod(all_best_odds), Lay = prod(all_worst_odds)
3. **ROI Check**: Apply leg-count-specific thresholds
4. **Individual Legs**: Create separate leg entries for each event (not just one lay leg)
5. **Risk Scaling**: Different risk levels for 2, 3, 4+ legs

### New Test Cases (19 tests)

#### Optimizer Tests (6)
- `test_optimizer_optimize_legs` - Verify leg selection
- `test_optimizer_calculate_roi_2leg` - ROI math verification
- `test_optimizer_calculate_roi_3leg` - Multi-leg ROI math
- `test_optimizer_roi_meets_threshold_2leg` - 2-leg filtering
- `test_optimizer_roi_meets_threshold_3leg` - 3-leg filtering
- `test_optimizer_validate_leg_availability` - BK coverage check

#### Multi-Leg Detection Tests (4)
- `test_2leg_express_fork_detection` - 2-leg forks
- `test_3leg_express_fork_detection` - 3-leg forks
- `test_4leg_express_fork_detection` - 4-leg forks
- `test_5leg_express_fork_detection` - 5-leg forks

#### Feature Tests (9)
- `test_roi_filtering_3plus_legs` - ROI threshold enforcement
- `test_per_leg_bk_optimization` - Multi-BK selection
- `test_no_forks_with_zero_odds` - Edge case handling
- `test_no_forks_insufficient_legs` - Minimum leg requirement
- `test_fork_risk_levels` - Risk classification
- `test_cascade_odds_calculation` - Odds multiplication
- `test_multiple_combinations_at_different_legs` - Mixed leg counts
- `test_respects_max_legs_limit` - Max leg enforcement
- `test_stake_distribution` - Stake calculation

---

## File 2: crates/express_forks/src/scanner.rs

### Changes Summary
- **Lines Added**: ~280 lines total
- **New Structures**: ComboCache
- **New Methods**: 4 new public methods
- **New Tests**: 12 comprehensive test cases

### New Structure: ComboCache (45 lines)

```rust
#[derive(Clone)]
pub struct ComboCache {
    results: Arc<DashMap<String, Option<ExpressFork>>>,
    seen_combos: Arc<RwLock<HashSet<String>>>,
}

impl ComboCache {
    pub fn new() -> Self
    pub fn get(&self, key: &str) -> Option<Option<ExpressFork>>
    pub fn insert(&self, key: String, fork: Option<ExpressFork>>
    pub fn mark_seen(&self, key: String) -> bool
    pub fn is_seen(&self, key: &str) -> bool
    pub fn clear(&self)
    pub fn size(&self) -> usize
}
```
**Purpose**: Efficient caching of computed combinations with deduplication

### Enhanced ExpressForkScanner

#### New Field
```rust
combo_cache: ComboCache,  // Added for caching
```

#### Constructor Changes
```rust
// Old constructor (kept for compatibility)
pub fn new(max_legs: usize, min_profit: f64, default_stake: f64) -> Self

// New constructor (supports custom ROI thresholds)
pub fn new_with_min_roi(max_legs: usize, min_profit: f64, default_stake: f64, min_roi_3plus: f64) -> Self
```

#### New Methods
```rust
// Cache statistics API
pub fn cache_stats(&self) -> (usize, usize)  // Returns (cache_size, seen_count)

// Cache management
pub fn clear_caches(&self)  // Clears all cached data
```

### Enhanced scan() Method
```rust
// No signature change, but now:
// 1. Uses new calculator with optimizer
// 2. Integrates with combo_cache
// 3. Still maintains seen_keys deduplication
```

### New Test Cases (12 tests)

#### Basic Tests (3)
- `test_scan_express_forks` - Basic scanning functionality
- `test_cache_deduplication` - Duplicate prevention
- `test_get_recent_forks` - Recent fork retrieval

#### Configuration Tests (2)
- `test_scanner_with_custom_min_roi` - Custom thresholds
- `test_fork_key_consistency` - Dedup key stability

#### Cache Tests (4)
- `test_cache_stats` - Statistics API
- `test_clear_caches` - Cache clearing
- `test_combo_cache_new` - Initialization
- `test_combo_cache_operations` - Cache operations

#### Feature Tests (3)
- `test_multi_leg_combinations` - Multiple leg counts
- `test_scanner_performance_many_events` - Performance test
- `test_scan_empty_data` - Edge case handling

---

## Key Implementation Details

### 1. Cascade Odds Calculation

**OLD** (2-leg only):
```rust
let express_total: f64 = express_odds.iter().product();
let lay_total = min_lay.powi(2);  // Hardcoded for 2 legs
```

**NEW** (N-leg):
```rust
// Express: multiply all best odds
let express_total: f64 = combo_legs.iter().map(|l| l.best_odds).product();

// Lay: multiply all worst odds per leg
let lay_odds_per_leg: Vec<f64> = /* for each leg, find worst odds */;
let lay_total: f64 = lay_odds_per_leg.iter().product();
```

### 2. Per-Leg BK Selection

**NEW**:
```rust
// For each event, find the bookmaker with best odds
let optimized_legs = self.optimizer.optimize_legs(&odds_by_event);

// Each leg can be from different BK
for (idx, (eid, leg_info)) in event_ids.iter().zip(combo_legs.iter()).enumerate() {
    legs.push(ExpressForkLeg {
        bookmaker: leg_info.best_bookmaker.clone(),  // Can differ per leg
        // ...
    });
}
```

### 3. ROI Filtering Logic

```rust
// Leg-count-specific thresholds
pub fn roi_meets_threshold(&self, legs_count: usize, roi: f64) -> bool {
    if legs_count >= 3 {
        roi >= self.min_roi_3plus_legs  // 3.0% default
    } else {
        roi > 0.1  // Minimal for 2-leg
    }
}
```

### 4. Leg Availability Validation

```rust
// Ensure each leg is available in 2+ BKs
pub fn validate_leg_availability(&self, legs: &[OptimizedLeg]) -> bool {
    legs.iter().all(|leg| leg.available_in_bks.len() >= 2)
}
```

### 5. Risk Level Assignment

```rust
let risk_level = match leg_count {
    2 => ExpressForkRisk::Low,
    3 => ExpressForkRisk::Medium,
    4 => ExpressForkRisk::High,
    _ => ExpressForkRisk::High,
};
```

---

## Performance Characteristics

### Time Complexity
- **For N events, finding all K-leg combinations**:
  - Combinations count: C(N, K) = N! / (K! × (N-K)!)
  - For each combination: O(K) operations
  - Total: O(C(N, K) × K)
  - Example: 50 events, max 5 legs ≈ 2.5M combinations

### Space Complexity
- **Cache**: O(min(seen_count, 10,000))
- **Recent forks**: O(seen_count) bounded
- **Optimizer results**: O(N) where N = number of events

### Optimization Techniques
1. **ComboCache**: Avoid recalculating same combos
2. **Early Exit**: Skip invalid combos early
3. **Bounded Cache**: Automatic cleanup at 5K/10K threshold
4. **Concurrent**: DashMap for parallel access

---

## Configuration Examples

### Conservative (High ROI only)
```rust
let scanner = ExpressForkScanner::new_with_min_roi(3, 0.5, 1000.0, 5.0);
// Max 3 legs, min 5% ROI for 3+ legs
```

### Balanced (Default)
```rust
let scanner = ExpressForkScanner::new(5, 0.1, 1000.0);
// Max 5 legs, min 3% ROI for 3+ legs (hardcoded)
```

### Aggressive (Lower ROI)
```rust
let scanner = ExpressForkScanner::new_with_min_roi(5, 0.05, 1000.0, 2.0);
// Max 5 legs, min 2% ROI for 3+ legs
```

---

## Migration Guide for Existing Code

### Before
```rust
let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);
let forks = calc.find_express_forks(&events, &odds);
```

### After (Backward Compatible)
```rust
let calc = ExpressForkCalculator::new(3, 0.5, 1000.0);  // Still works!
let forks = calc.find_express_forks(&events, &odds);    // Same results, better
```

### After (New Features)
```rust
let calc = ExpressForkCalculator::new_with_optimizer(5, 0.5, 1000.0, 2.5);
let forks = calc.find_express_forks(&events, &odds);  // Custom ROI threshold
```

---

## Testing Strategy

### Unit Tests
- Isolated function testing
- Math verification
- Edge cases

### Integration Tests
- Multi-component workflows
- Scanner with calculator
- Cache integration

### Performance Tests
- 10-event stress test
- Completion time verification
- Memory stability

### Edge Case Tests
- Empty inputs
- Single event
- Insufficient BK coverage
- Low ROI scenarios

---

## Deployment Checklist

- ✅ Code compiles without warnings
- ✅ All 31 tests pass
- ✅ Backward compatible with old code
- ✅ Documentation complete
- ✅ Examples provided
- ✅ Edge cases handled
- ✅ Performance acceptable
- ✅ Memory bounded
- ✅ Concurrent access safe (Arc + DashMap)
- ✅ Risk levels assigned correctly
