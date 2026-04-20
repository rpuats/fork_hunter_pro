# Telegram Bot Configuration Example
# This file shows how to configure Telegram alerts for the Ghost Imperium system

# =============================================================================
# ENVIRONMENT VARIABLES (recommended for production)
# =============================================================================
# TELEGRAM_BOT_TOKEN=<your_bot_token_from_botfather>
# TELEGRAM_ADMIN_CHATS=<chat_id_1>,<chat_id_2>,<chat_id_3>

# Or set in config.yaml:

telegram:
  # Telegram bot token from BotFather
  bot_token: "${TELEGRAM_BOT_TOKEN}"
  
  # Admin chat IDs (comma-separated list)
  # Get your chat ID: @userinfobot
  admin_chat_ids:
    - 123456789
    - 987654321
  
  # Minimum ROI percentage to trigger alerts (default: 2.0)
  notify_min_profit: 2.0
  
  # Silent mode - if true, only respond to commands, no auto-alerts
  silent_mode: false

# =============================================================================
# TELEGRAM BOT COMMANDS
# =============================================================================
# Once bot is running, you can use these commands:

# /start          - Show introduction and quick command list
# /status         - Bridge status and uptime metrics
# /health         - EventBus and parser health rollup
# /recent         - Last 5 surebets detected
# /top            - Highest ROI recent surebets
# /alerts         - Alert statistics and recent alerts
# /settings       - Current alert configuration and stats
# /history        - Last 10 alerts with status
# /help           - Show all available commands

# =============================================================================
# API ENDPOINTS FOR TELEGRAM MANAGEMENT
# =============================================================================

# Check telegram status:
# GET /api/v1/telegram/status

# Update alert configuration:
# POST /api/v1/telegram/config
# {
#   "min_roi_percent": 2.5,
#   "max_alerts_per_minute": 10.0,
#   "only_verified": false,
#   "only_live": false
# }

# View alert history:
# GET /api/v1/telegram/history

# =============================================================================
# MESSAGE FORMAT EXAMPLE
# =============================================================================
# When a surebet is found that meets the criteria:
#
# 🔥 SUREBET FOUND
# 💰 ROI: 3.45%
# 💵 Profit: 345 RUB
# 📊 Match: Arsenal vs Chelsea
# 🏆 League: Premier League
# ⏰ Start: 13.04 15:00 UTC
# Status: ✅ Verified | 🔴 LIVE
#
# Legs:
# 1. pari 1X2 @ 2.10 | 1 (2.10x)
# 2. fonbet 1X2 @ 2.05 | X (2.05x)
# 3. leon 1X2 @ 1.95 | 2 (1.95x)
#
# Total Stake: 1000 RUB
# Expected Payout: 2100 RUB
# ID: 550e8400-e29b-41d4-a716-446655440000

# =============================================================================
# RATE LIMITING
# =============================================================================
# The system includes automatic rate limiting to prevent message spam:
#
# - Default: 10 alerts per minute (configurable)
# - Uses token bucket algorithm
# - Throttled alerts are recorded in history
# - System prevents duplicate alerts for same opportunity

# =============================================================================
# FILTERING AND SETTINGS
# =============================================================================
# You can control what gets alerted:
#
# min_roi_percent:        Only alert for ROI >= this value
# max_alerts_per_minute:  Maximum alerts sent per minute
# only_verified:          Only alert for verified surebets
# only_live:              Only alert for live matches
# alert_on_verified_only: More lenient verified filter

# =============================================================================
# SETUP STEPS
# =============================================================================
#
# 1. Create Telegram Bot:
#    - Message @BotFather on Telegram
#    - Type /newbot
#    - Follow the prompts to create your bot
#    - Copy the bot token
#
# 2. Get Your Chat ID:
#    - Message @userinfobot on Telegram
#    - It will reply with your chat ID
#    - For group chats, add the bot and get the group chat ID
#
# 3. Configure Ghost Imperium:
#    - Set TELEGRAM_BOT_TOKEN environment variable or in config.yaml
#    - Set TELEGRAM_ADMIN_CHATS to your chat IDs
#
# 4. Start the application:
#    - The bot will authenticate and start receiving alerts
#    - Test with /help command
#
# 5. (Optional) Use API endpoints to update settings:
#    - POST /api/v1/telegram/config to adjust alert thresholds
#    - GET /api/v1/telegram/status to check bot status
#
# =============================================================================
# TROUBLESHOOTING
# =============================================================================
#
# Bot not sending alerts:
# - Check TELEGRAM_BOT_TOKEN is correct
# - Verify chat IDs are correct (use @userinfobot)
# - Check silent_mode is false
# - Verify ROI threshold is being met
#
# Messages not formatted correctly:
# - The system uses HTML formatting
# - Some Telegram clients may not support all HTML tags
# - Try updating your Telegram client
#
# Rate limiting kicks in:
# - If you get 10+ opportunities per minute
# - Alerts will be queued and sent at next available slot
# - Check /alerts to see throttled count
#
# Too many alerts:
# - Increase min_roi_percent threshold
# - Set only_verified = true
# - Reduce max_alerts_per_minute
# - Use /settings to check current config
