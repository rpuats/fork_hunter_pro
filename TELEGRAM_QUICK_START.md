# Quick Start Guide - Telegram Alerts

## 30-Second Setup

### Step 1: Create Telegram Bot
```bash
# Message @BotFather on Telegram
/newbot
# Follow prompts, copy token

# Get your chat ID
# Message @userinfobot
# Copy the number it returns
```

### Step 2: Configure
```bash
export TELEGRAM_BOT_TOKEN="your_token_here"
export TELEGRAM_ADMIN_CHATS="your_chat_id_here"
```

### Step 3: Build & Run
```bash
cd fork_hunter_pro
cargo build --release
./target/release/fork_hunter
```

### Step 4: Test in Telegram
```
/start       # Bot intro
/status      # Check status
/settings    # View config
```

---

## Detailed Setup

### Get Telegram Bot Token

1. Open Telegram and search for `@BotFather`
2. Send `/newbot` command
3. Choose a name: `Ghost Imperium Bot`
4. Choose a username: `ghost_imperium_bot` (must be unique)
5. Copy the token: `123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefg`

### Get Your Chat ID

1. Search for `@userinfobot` in Telegram
2. Send any message (e.g., `/start`)
3. Bot replies with your ID: `987654321`

### Configuration Options

**Option A: Environment Variables** (Recommended for development)
```bash
export TELEGRAM_BOT_TOKEN="123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefg"
export TELEGRAM_ADMIN_CHATS="987654321"
cargo run --bin fork_hunter --release
```

**Option B: Config File** (Recommended for production)

Edit `config.yaml`:
```yaml
telegram:
  bot_token: "123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefg"
  admin_chat_ids:
    - 987654321
    - 123456789  # Multiple chats supported
  notify_min_profit: 2.0
  silent_mode: false
```

Then run:
```bash
cargo run --bin fork_hunter --release
```

---

## Testing

### Run Unit Tests

```bash
# All tests
cargo test --lib bot

# Specific module
cargo test --lib bot rate_limiter
cargo test --lib bot notifier

# With output
cargo test --lib bot -- --nocapture

# Single test
cargo test --lib bot rate_limiter::tests::limiter_starts_with_full_capacity
```

**Expected output**:
```
running 30 tests
test rate_limiter::tests::limiter_starts_with_full_capacity ... ok
test rate_limiter::tests::limiter_consumes_single_token ... ok
...
test result: ok. 30 passed in 2.45s
```

### Manual Testing in Telegram

1. **Check bot is running**
   ```
   /start
   # Should see: "👋 Ghost Imperium Bot"
   ```

2. **Check bot status**
   ```
   /status
   # Should show uptime, event counts
   ```

3. **Check settings**
   ```
   /settings
   # Should show ROI threshold, rate limit
   ```

4. **View alert history**
   ```
   /history
   # Shows last 10 alerts
   ```

5. **See all commands**
   ```
   /help
   # Lists all available commands
   ```

### Monitor Alerts

In Telegram, you'll see messages like:
```
🔥 SUREBET FOUND
💰 ROI: 3.45%
💵 Profit: 345 RUB
📊 Match: Arsenal vs Chelsea
...
```

---

## Configuration Examples

### Strict Filtering
```yaml
telegram:
  notify_min_profit: 5.0      # Only 5%+ ROI
  admin_chat_ids:
    - 987654321
```

### Relaxed Filtering
```yaml
telegram:
  notify_min_profit: 1.0      # Even 1%+ ROI
  admin_chat_ids:
    - 987654321
```

### Multiple Admin Chats
```yaml
telegram:
  admin_chat_ids:
    - 987654321      # Personal
    - 555666777      # Partner
    - 123456789      # Team
```

### High Volume (30 alerts/min)
```yaml
telegram:
  notify_min_profit: 2.0
  admin_chat_ids:
    - 987654321
```

Update via API:
```bash
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -H "Content-Type: application/json" \
  -d '{"max_alerts_per_minute": 30.0}'
```

### Silent Mode (Commands Only)
```yaml
telegram:
  silent_mode: true    # No auto-alerts, only commands
```

---

## Common Commands

| Command | Purpose | Example Output |
|---------|---------|-----------------|
| `/start` | Introduction | Bot intro message |
| `/status` | Bridge metrics | Uptime, event counts |
| `/health` | Parser health | Healthy/degraded/unhealthy counts |
| `/recent` | Last 5 opportunities | List with ROI% |
| `/top` | Highest ROI | Sorted by profit |
| `/alerts` | Statistics | Sent/throttled counts |
| `/settings` | Configuration | Current filters and stats |
| `/history` | Alert log | Last 10 with status |
| `/help` | Command list | All available commands |

---

## API Testing

### Check Bot Status
```bash
curl http://localhost:3000/api/v1/telegram/status | jq

# Output:
{
  "status": "ok",
  "data": {
    "connected": true,
    "rate_limiter_status": "operational",
    "alerts_config": {
      "min_roi_percent": 2.0,
      "max_alerts_per_minute": 10.0
    },
    "alerts_stats": {
      "total_alerts": 5,
      "sent": 4,
      "throttled": 1,
      "skipped": 0
    }
  }
}
```

### Update Configuration
```bash
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -H "Content-Type: application/json" \
  -d '{
    "min_roi_percent": 1.5,
    "max_alerts_per_minute": 15.0,
    "only_verified": false,
    "only_live": false
  }' | jq

# Output:
{
  "status": "ok",
  "data": {
    "status": "config_updated",
    "message": "Telegram alert settings have been updated"
  }
}
```

### View Alert History
```bash
curl http://localhost:3000/api/v1/telegram/history | jq

# Output:
{
  "status": "ok",
  "data": {
    "history": [
      {
        "surebet_id": "550e8400...",
        "roi_percent": 3.45,
        "teams": "Arsenal vs Chelsea",
        "timestamp": "2026-04-19T15:30:45.123Z",
        "verified": true,
        "is_live": false,
        "status": "Sent"
      }
    ],
    "limit": 20,
    "total": 42
  }
}
```

---

## Troubleshooting

### Bot Doesn't Respond to `/start`

1. **Check token is valid**
   ```bash
   curl https://api.telegram.org/botTOKEN/getMe
   # Should return bot info
   ```

2. **Check environment variable is set**
   ```bash
   echo $TELEGRAM_BOT_TOKEN
   # Should print your token
   ```

3. **Check chat ID is correct**
   ```bash
   /status  # Should show "Chats: 1"
   ```

### No Alerts Received

1. **Check ROI threshold**
   ```
   /settings
   # View Min ROI threshold
   ```

2. **Verify opportunities exist**
   ```
   /recent   # Shows detected opportunities
   /top      # Shows best ones
   ```

3. **Check rate limiter**
   ```
   /alerts
   # View "Sent" and "Throttled" counts
   ```

### Rate Limit Too Aggressive

1. **Increase capacity via API**
   ```bash
   curl -X POST http://localhost:3000/api/v1/telegram/config \
     -d '{"max_alerts_per_minute": 20.0}'
   ```

2. **Or reduce ROI threshold**
   ```bash
   curl -X POST http://localhost:3000/api/v1/telegram/config \
     -d '{"min_roi_percent": 1.5}'
   ```

### Messages Look Weird

- Some Telegram clients have limited HTML support
- Update Telegram app to latest version
- Try browser version at web.telegram.org

---

## Monitoring

### View Logs
```bash
# On Linux/Mac
tail -f ghost_imperium.log | grep -i telegram

# On Windows
Get-Content ghost_imperium.log -Tail 100 | Select-String -Pattern "telegram" -i
```

### Expected Log Messages
```
[2026-04-19T15:30:45Z INFO  bot::telegram] Telegram bot authorized as @ghost_imperium_bot
[2026-04-19T15:30:46Z INFO  bot::bridge] Telegram EventBus bridge started
[2026-04-19T15:31:00Z INFO  bot::telegram] 🔥 Telegram alert sent: 3.45% Arsenal vs Chelsea
[2026-04-19T15:31:05Z INFO  bot::telegram] Alert throttled (rate limiter)
```

### Monitor via API
```bash
# Every 5 seconds
while true; do
  curl -s http://localhost:3000/api/v1/telegram/status | jq '.data.alerts_stats'
  sleep 5
done

# Output:
{
  "total_alerts": 42,
  "sent": 35,
  "throttled": 3,
  "skipped": 4,
  "avg_roi": 2.84,
  "sent_in_last_hour": 8,
  "sent_in_last_minute": 0
}
```

---

## Performance Tuning

### For High-Volume Scanning

```bash
# Increase rate limit
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -d '{"max_alerts_per_minute": 30.0}'

# Reduce ROI threshold  
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -d '{"min_roi_percent": 1.0}'
```

### For Conservative Alerts

```bash
# Decrease rate limit
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -d '{"max_alerts_per_minute": 5.0}'

# Increase ROI threshold
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -d '{"min_roi_percent": 5.0}'

# Only verified surebets
curl -X POST http://localhost:3000/api/v1/telegram/config \
  -d '{"only_verified": true}'
```

---

## Production Deployment

### 1. Verify Tests Pass
```bash
cargo test --lib bot
# Expected: 30+ passed
```

### 2. Build Release Binary
```bash
cargo build --release
# Binary: target/release/fork_hunter
```

### 3. Set Environment Variables
```bash
export TELEGRAM_BOT_TOKEN="token_here"
export TELEGRAM_ADMIN_CHATS="chat_id_here"
```

### 4. Run in Background
```bash
# Linux/Mac
nohup ./target/release/fork_hunter > fork_hunter.log 2>&1 &

# Windows
Start-Process -NoNewWindow -FilePath "./target/release/fork_hunter.exe"
```

### 5. Verify Running
```bash
curl http://localhost:3000/api/v1/telegram/status
# Should show connected: true
```

---

## Next Steps

1. ✅ **Setup**: Follow 30-second setup above
2. ✅ **Test**: Run `cargo test --lib bot`
3. ✅ **Configure**: Set TELEGRAM_BOT_TOKEN and TELEGRAM_ADMIN_CHATS
4. ✅ **Run**: `cargo run --bin fork_hunter --release`
5. ✅ **Verify**: Test `/start` command in Telegram
6. ✅ **Monitor**: Watch for alerts as opportunities are detected

---

## Support

For more details, see:
- **Full guide**: `TELEGRAM_ALERTS_README.md`
- **Configuration**: `TELEGRAM_CONFIG_EXAMPLE.md`
- **Architecture**: `TELEGRAM_INTEGRATION_EXAMPLE.md`
- **Verification**: `TELEGRAM_VERIFICATION_CHECKLIST.md`
- **Delivery**: `TELEGRAM_DELIVERY_SUMMARY.md`

---

## Quick Reference

```bash
# Setup (one-time)
export TELEGRAM_BOT_TOKEN="..."
export TELEGRAM_ADMIN_CHATS="..."

# Build
cargo build --release

# Test
cargo test --lib bot

# Run
./target/release/fork_hunter

# API endpoints
GET  /api/v1/telegram/status        # Check bot
POST /api/v1/telegram/config        # Update settings
GET  /api/v1/telegram/history       # View alerts

# Telegram commands
/start      # Bot intro
/status     # Bridge status
/settings   # Current config
/history    # Alert log
/help       # All commands
```

**Ready to use!**
