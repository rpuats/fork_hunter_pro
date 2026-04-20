# Zenit Parser Fix — Testing & Validation Guide

## ✅ Pre-Merge Validation Checklist

### 1. Code Compilation
```bash
cd crates/parsers
cargo build --release
```
**Expected**: No warnings, compilation succeeds

### 2. Unit Tests (5 new + existing)
```bash
cargo test zenit:: --lib
```
**Expected Output**:
```
test zenit::tests::is_transient_error_detects_timeout ... ok
test zenit::tests::is_transient_error_detects_connection_errors ... ok
test zenit::tests::is_transient_error_detects_server_errors ... ok
test zenit::tests::is_transient_error_rejects_permanent_errors ... ok
test zenit::tests::backoff_duration_increases_exponentially ... ok
test zenit::tests::line_query_matches_browser_capture_shape ... ok
test zenit::tests::parse_response_supports_string_dates_and_numeric_strings ... ok
test zenit::tests::parse_date_value_accepts_short_formats ... ok

test result: ok. 8 passed
```

### 3. Verbose Test Logging
```bash
RUST_LOG=debug cargo test zenit:: --lib -- --nocapture
```
**Look for**:
- "Zenit retry attempt"
- "Zenit transient error, retrying after backoff"
- "Zenit operation succeeded after retries"
- "Zenit permanent error (not retrying)"

### 4. Runtime Network Test (requires API access)
```bash
cargo test zenit_runtime_counts_against_live_output -- --ignored --nocapture
```
**Expected Output**:
```
zenit runtime counts: live=XXX, prematch=YYYY, total=ZZZZ
```
**Success criteria**:
- total > 0 (not zero)
- prematch > 150 (at least some events)

### 5. Live Request Branch Probe Test
```bash
cargo test zenit_runtime_request_branch_probe -- --ignored --nocapture
```
**Expected Output**:
```
zenit branch probe: sports=X, football_line_raw=Y, football_line_events=Z, ...
- sports > 0
- football_line_raw > 0
- football_line_events > 0
- live_raw > 0
- live_events > 0
```

---

## 🔍 Manual Testing Scenarios

### Scenario 1: Normal Operation Test
**Goal**: Verify no performance regression when API is healthy

**Steps**:
1. Set up clean environment: `export RUST_LOG=info`
2. Run parser: `cargo test zenit_runtime_counts_against_live_output -- --ignored`
3. Check logs: Should NOT see any "retry", "backoff", or "error" messages
4. Verify events returned: > 0

**Expected**: ~1-2 seconds, no retries

### Scenario 2: Error Handling Test (requires network disruption)
**Goal**: Verify transient errors are retried

**Steps**:
1. Set RUST_LOG: `export RUST_LOG=debug`
2. Simulate network issue (e.g., kill/restart network interface)
3. Run parser during disruption: `cargo test zenit_runtime_counts_against_live_output -- --ignored`
4. Check logs for retry sequence

**Expected Log Sequence**:
```
debug: Zenit fetch_page request (attempt 0)
error: Zenit fetch_page HTTP error: timeout
warn: Zenit transient error, retrying after backoff (500ms)
debug: Zenit retry attempt (attempt 1)
debug: Zenit fetch_page request (attempt 1)
debug: Zenit fetch_page response (status 200)
info: Zenit operation succeeded after retries (attempt 1)
```

### Scenario 3: Rate Limit Handling Test
**Goal**: Verify 429 errors are retried

**Steps**:
1. Set up rate limiter that returns 429
2. Run parser: `cargo test zenit_runtime_counts_against_live_output -- --ignored`
3. Check logs for 429 handling

**Expected Log Sequence**:
```
error: Zenit API returned HTTP 429 for sport...
warn: Zenit transient error (429), retrying after 500ms
debug: Zenit retry attempt (attempt 1)
... (retry succeeds on 2nd attempt)
info: Zenit operation succeeded after retries (attempt 1)
```

### Scenario 4: Permanent Error Test
**Goal**: Verify permanent errors are NOT retried

**Steps**:
1. Simulate 404 error (e.g., change endpoint URL)
2. Run parser: `cargo test --lib`
3. Check logs

**Expected Log Sequence**:
```
error: Zenit API returned HTTP 404
error: Zenit permanent error (not retrying)
[immediate failure, no retry attempts]
```

---

## 📊 Performance Baseline

### Before Fix (Legacy)
- Normal case: ~1-2 seconds, 1 request
- Error case: ~30 seconds (timeout), 1 request, then fails

### After Fix
- Normal case: ~1-2 seconds, 1 request (identical)
- Transient error: ~2-3 seconds, 2-3 requests (with backoff)
- Permanent error: ~1-2 seconds, 1 request (no retry)

**Overhead**: +0 seconds for normal case, +recovery for transient cases

---

## 🐛 Debugging Commands

### Enable all Rust logs
```bash
RUST_LOG=trace cargo test zenit:: --lib -- --nocapture
```

### Enable only Zenit logs
```bash
RUST_LOG=zenit=debug cargo test zenit:: --lib -- --nocapture
```

### Filter to specific test
```bash
cargo test zenit::tests::backoff_duration_increases_exponentially -- --nocapture
```

### Check if function exists
```bash
grep -n "fn is_transient_error" crates/parsers/src/zenit.rs
grep -n "fn backoff_duration" crates/parsers/src/zenit.rs
grep -n "fn retry_with_backoff" crates/parsers/src/zenit.rs
```

### Count changes
```bash
wc -l crates/parsers/src/zenit.rs
# Should be slightly more than before (added ~150 lines)
```

---

## ✅ Integration Testing

### Test with ParserFactory
Ensure Zenit parser still integrates with factory:
```rust
let parser = ParserFactory::create("zenit")?;
let result = parser.fetch_all().await?;
assert!(result.events.len() > 0);
```

### Test with Event Pool
Ensure events are properly added to pool:
```rust
let (events, odds) = parser.fetch_runtime_data().await?;
// Should not return empty
assert!(events.len() > 0);
```

### Test with full pipeline
Run full nightly:
```bash
cargo run --release -- parser:zenit
# Check logs for:
# - No "Zenit operation failed" messages
# - "Zenit events parsed: XXXX" (not 0)
```

---

## 🎯 Acceptance Criteria

✅ Code compiles without warnings
✅ All 13 tests pass (5 new + 8 existing)
✅ Logs show retry attempts when appropriate
✅ No retries on permanent errors (404, 401, 400)
✅ Backoff duration increases exponentially
✅ Normal API calls not affected (no extra latency)
✅ Transient errors recovered (retry succeeds)
✅ Max 3 retries respected
✅ No changes to public API
✅ Documentation complete

---

## 📋 Merge Approval Checklist

Before merging, verify:
- [ ] All tests pass locally: `cargo test zenit:: --lib`
- [ ] No compiler warnings: `cargo build --release`
- [ ] Logs show correct format
- [ ] Backoff duration is exponential (500, 1000, 2000ms)
- [ ] Does not retry on permanent errors
- [ ] Request timeout is 30 seconds
- [ ] fetch_page has retry
- [ ] fetch_live_page has retry
- [ ] fetch_available_sports has retry
- [ ] Error messages include response body
- [ ] is_transient_error correctly identifies errors
- [ ] Documentation is complete (ZENIT_FIX_REPORT.md)

---

## 🚀 Deployment Steps

1. **Merge to develop**
   ```bash
   git checkout develop
   git merge --no-ff feature/zenit-retry-logic
   ```

2. **Build and test**
   ```bash
   cargo build --release
   cargo test --release
   ```

3. **Deploy to staging**
   ```bash
   ./deploy.sh staging
   ```

4. **Monitor nightly run**
   - Watch for "Zenit events parsed: XXXX"
   - Should be ~4000, not 0
   - Check logs for any "operation failed" messages

5. **Promote to production**
   ```bash
   ./deploy.sh production
   ```

---

## 📞 Support

If tests fail:
1. Check: `crates/parsers/src/zenit.rs` line 1, must have `use tokio::time::sleep;`
2. Check: `crates/parsers/src/zenit.rs` line ~250, must have `fn is_transient_error()`
3. Check: `crates/parsers/src/zenit.rs` line ~260, must have `fn backoff_duration()`
4. Check: `crates/parsers/src/zenit.rs` line ~270, must have `async fn retry_with_backoff()`
5. Check: `crates/parsers/src/zenit.rs` line ~1179, must have test `is_transient_error_detects_timeout`

All should exist in the updated file.
