# Zenit Parser — Retry Logic Architecture

## 🔄 Retry Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ fetch_page(sport, offset, games)                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
        ┌─────────────────────────────┐
        │ retry_with_backoff(         │
        │   "fetch_page(...)",        │
        │   operation closure         │
        │ )                           │
        └──────────────┬──────────────┘
                       │
         ┌─────────────┴─────────────┐
         │                           │
    ┌────▼────────┐          ┌───────▼──────┐
    │  Attempt 0  │          │  Attempt 1   │
    │   (MAX: 3)  │          │ (if failed)  │
    └─┬──────────┬┘          └───────┬──────┘
      │          │                   │
  Success    ├─► Error?              └──► (backoff 500ms)
      │      │                             ├──► Error?
      │      │   Transient?                │
      │      │   (429/502/503/timeout...)  │
      │      │                             ├──► Permanent?
      │      ├─► YES: backoff & retry      │    (404/401...)
      │      │   (wait: 500ms * 2^N)       │
      │      │                             └──► YES: Return Error
      │      │                             
      │      └─► NO: Return Error Immediately
      │
      └──────────────────► Return Result
```

---

## 📊 State Machine

```
                    ┌─────────────┐
                    │   START     │
                    └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ Attempt #N  │ (N = 0, 1, 2)
                    │  (attempt++)│
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼
         ┌─────────┐          ┌──────────────────┐
         │ Execute │          │ Reached MAX_RET? │
         │Operation│          │    (3)           │
         └────┬────┘          └──────┬───────────┘
              │                      │
    ┌─────────┴─────────┐            ▼
    │                   │       ┌──────────┐
 Success            Error       │  Return  │
    │                   │       │  Error   │
    ▼                   ▼       └──────────┘
  ✅ OK          Is Transient?
              (timeout/429/502...)
              ┌────────┬──────┐
              │        │      │
              ▼        ▼      │
           YES        NO     │
          ┌──────┐  ┌───────┐│
          │Sleep │  │Return ││
          │(ms)  │  │Error  ││
          └──┬───┘  └───────┘│
             │               │
             └──► Retry ◄────┘
```

---

## 🎯 Decision Tree

```
Operation Failed?
├─ YES
│  │
│  └─ Transient Error?
│     ├─ YES (timeout, 429, 502, 503, 504, connection)
│     │  │
│     │  ├─ Attempts Left? (MAX_RETRIES = 3)
│     │  │  ├─ YES
│     │  │  │  │
│     │  │  │  ├─ Calculate backoff
│     │  │  │  │  └─ Duration = 500ms * 2^attempt
│     │  │  │  │     ├─ Attempt 0 → 500ms
│     │  │  │  │     ├─ Attempt 1 → 1000ms
│     │  │  │  │     └─ Attempt 2 → 2000ms (capped)
│     │  │  │  │
│     │  │  │  ├─ Log: "retrying after Xms"
│     │  │  │  │
│     │  │  │  └─ Sleep(backoff)
│     │  │  │     └─ Loop back to Operation
│     │  │  │
│     │  │  └─ NO (out of retries)
│     │  │     └─ Log: "failed after 3 retries"
│     │  │     └─ Return Error
│     │  │
│     │  └─ NO (permanent error 404/401/400/JSON parse)
│     │     └─ Log: "permanent error, not retrying"
│     │     └─ Return Error Immediately
│     │
│     └─ NO
│        └─ Return Error
│
└─ NO (Success)
   └─ Log: "operation succeeded"
      └─ Return Result
```

---

## 📈 Backoff Timeline

```
Attempt 0 (t=0s)
    │
    ├─ Send Request
    │  └─ ERROR: timeout
    │
    └─ Backoff: 500ms
       │
       ├─ sleep(500ms)
       │
       └─ t=0.5s
           │
           Attempt 1 (t=0.5s)
           │
           ├─ Send Request
           │  └─ ERROR: 429 (rate limit)
           │
           └─ Backoff: 1000ms (500 * 2^1)
              │
              ├─ sleep(1000ms)
              │
              └─ t=1.5s
                  │
                  Attempt 2 (t=1.5s)
                  │
                  ├─ Send Request
                  │  └─ ✅ SUCCESS
                  │
                  └─ Return Result
                     (total time: ~1.5 seconds)
```

---

## 🔗 Function Call Chain

```
ParserFactory.create("zenit")
├─ Creates ZenitParser
└─ Returns: Arc<ZenitParser>
   │
   ├─ fetch_all()
   │  │
   │  ├─ fetch_available_sports()
   │  │  └─ retry_with_backoff("fetch_available_sports")
   │  │     └─ HTTP GET left_menu/get
   │  │
   │  ├─ For each sport:
   │  │  │
   │  │  └─ fetch_sport()
   │  │     │
   │  │     └─ fetch_page("prematch", sport_id)
   │  │        └─ retry_with_backoff("fetch_page(...)")
   │  │           └─ HTTP GET ajax/line/printer/react
   │  │
   │  └─ fetch_live()
   │     │
   │     └─ fetch_live_page()
   │        └─ retry_with_backoff("fetch_live_page")
   │           └─ HTTP GET ajax/live/printer/react
   │
   └─ Return ParserResult { events, odds, elapsed }
```

---

## 🔍 Error Classification

```
Error Message (String)
│
├─ Contains "timeout"?
│  └─ YES: Transient → Retry ✅
│
├─ Contains "connection"?
│  └─ YES: Transient → Retry ✅
│
├─ Contains "ConnectError"?
│  └─ YES: Transient → Retry ✅
│
├─ Contains "429"?
│  └─ YES: Transient (Rate Limit) → Retry ✅
│
├─ Contains "502"?
│  └─ YES: Transient (Bad Gateway) → Retry ✅
│
├─ Contains "503"?
│  └─ YES: Transient (Service Unavailable) → Retry ✅
│
├─ Contains "504"?
│  └─ YES: Transient (Gateway Timeout) → Retry ✅
│
├─ Contains "Temporary failure"?
│  └─ YES: Transient (DNS) → Retry ✅
│
├─ Contains "Too Many Requests"?
│  └─ YES: Transient (Rate Limit) → Retry ✅
│
└─ Otherwise: Permanent → No Retry ❌
   │
   ├─ 404 (Not Found)
   ├─ 401 (Unauthorized)
   ├─ 400 (Bad Request)
   ├─ JSON Parse Error
   └─ Other errors
```

---

## 📊 Logging Levels

```
┌──────────────────────────────────────────────┐
│ Operation Flow                               │
├──────────────────────────────────────────────┤
│                                              │
│ DEBUG: "Zenit fetch_page request"            │ ← Every attempt
│        (sport, offset, headers logged)       │
│                                              │
│ DEBUG: "Zenit fetch_page response"           │ ← Status code
│        (status = 200/429/502...)             │
│                                              │
│ ERROR: "Zenit fetch_page HTTP error"         │ ← HTTP error
│        (status + body logged)                │
│                                              │
│ DEBUG: "Zenit retry attempt"                 │ ← Each attempt #
│        (attempt = 0/1/2)                     │
│                                              │
│ WARN: "Zenit transient error, retrying"      │ ← Retry decision
│       (error, backoff_ms logged)             │
│                                              │
│ ERROR: "Zenit permanent error"               │ ← Won't retry
│        (error logged, not retrying)          │
│                                              │
│ INFO: "Zenit operation succeeded"            │ ← Success after retry
│       (attempt # logged)                     │
│                                              │
│ ERROR: "Zenit operation failed after N"      │ ← All retries exhausted
│        (max_retries, error logged)           │
│                                              │
└──────────────────────────────────────────────┘
```

---

## 🎛️ Configuration Parameters

```
┌──────────────────────────┬─────────┬──────────────┐
│ Parameter                │ Value   │ Purpose      │
├──────────────────────────┼─────────┼──────────────┤
│ MAX_RETRIES              │ 3       │ Max attempts │
│ INITIAL_BACKOFF_MS       │ 500     │ Initial wait │
│ MAX_BACKOFF_MS           │ 5000    │ Wait cap     │
│ REQUEST_TIMEOUT_SECS     │ 30      │ Request ttl  │
├──────────────────────────┼─────────┼──────────────┤
│ Backoff Formula          │         │              │
│ Duration = Base × 2^N    │         │              │
│ N = attempt number       │         │              │
│ Min: 500ms, Max: 5000ms  │         │              │
└──────────────────────────┴─────────┴──────────────┘

Examples:
- N=0: 500ms × 2^0 = 500ms × 1 = 500ms
- N=1: 500ms × 2^1 = 500ms × 2 = 1000ms
- N=2: 500ms × 2^2 = 500ms × 4 = 2000ms
- N=3: 500ms × 2^3 = 500ms × 8 = 4000ms
- N=4: 500ms × 2^4 = 500ms × 16 = 8000ms → Capped at 5000ms
```

---

## 📊 Success Scenarios

### Scenario 1: Normal (API healthy)
```
Attempt 0
├─ Send HTTP GET
├─ Receive 200 OK
├─ Parse JSON
└─ ✅ Return Result
   Total: ~1 second, 0 retries
```

### Scenario 2: One Transient Error
```
Attempt 0
├─ Send HTTP GET
├─ Timeout (socket timeout)
├─ Detect: transient
└─ Wait 500ms
   │
   Attempt 1
   ├─ Send HTTP GET
   ├─ Receive 200 OK
   ├─ Parse JSON
   └─ ✅ Return Result
      Total: ~1.5 seconds, 1 retry
```

### Scenario 3: Multiple Transient Errors
```
Attempt 0
├─ Send HTTP GET
├─ Receive 429 (Rate Limit)
├─ Detect: transient
└─ Wait 500ms
   │
   Attempt 1
   ├─ Send HTTP GET
   ├─ Receive 503 (Service Unavailable)
   ├─ Detect: transient
   └─ Wait 1000ms
      │
      Attempt 2
      ├─ Send HTTP GET
      ├─ Receive 200 OK
      ├─ Parse JSON
      └─ ✅ Return Result
         Total: ~2.5 seconds, 2 retries
```

### Scenario 4: Permanent Error
```
Attempt 0
├─ Send HTTP GET
├─ Receive 404 Not Found
├─ Detect: permanent
└─ ❌ Return Error Immediately
   Total: ~1 second, 0 retries (correct!)
```

---

## 🔐 Safety Guarantees

```
┌─────────────────────────────────┐
│ Retry Safety Checks             │
├─────────────────────────────────┤
│ ✅ Max 3 retries enforced       │
│    └─ Can't retry infinitely    │
│                                 │
│ ✅ Exponential backoff enforced │
│    └─ Prevents thundering herd  │
│                                 │
│ ✅ Caps at 5 seconds backoff    │
│    └─ Total timeout ~7 seconds  │
│                                 │
│ ✅ Permanent errors NOT retried │
│    └─ 404, 401, 400 fail fast   │
│                                 │
│ ✅ Distinguishes error types    │
│    └─ Timeout vs 404, etc       │
│                                 │
│ ✅ No infinite loops            │
│    └─ Always terminates         │
│                                 │
│ ✅ Proper error propagation     │
│    └─ Final error returned      │
│                                 │
│ ✅ Resource cleanup guaranteed  │
│    └─ Tokio sleep doesn't leak  │
└─────────────────────────────────┘
```

---

## 🎯 Expected Improvements

```
Before Fix:
Nightly Run → Zenit API Timeout → 0 Events → Pipeline Fails

After Fix:
Nightly Run → Zenit API Timeout → Retry (500ms) → Success → ~4000 Events ✅
```

```
Success Rate:
- Normal case: 100% (same as before)
- With 1 transient error: 100% (was 0%)
- With 2 transient errors: 100% (was 0%)
- With permanent error: 0% (same as before, correct!)
```

