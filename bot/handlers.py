# bot/handlers.py
import asyncio
from typing import Optional
from aiogram import Dispatcher, types
from aiogram.filters import Command
from aiogram.types import InlineKeyboardMarkup, InlineKeyboardButton
from aiogram.types import InlineQuery, InlineQueryResultArticle, InputTextMessageContent
from datetime import datetime
import logging

from bot.notifications import SurebetNotifier, NotificationLevel

logger = logging.getLogger(__name__)

dp_instance = Dispatcher()
scanner_ref = None
bot_instance = None
notifier_ref: Optional[SurebetNotifier] = None
admin_ids: set = set()
parser_states: dict = {}


def register_handlers(dp: Dispatcher, scanner, bot, notifier: Optional[SurebetNotifier] = None, admins: Optional[list] = None):
    global dp_instance, scanner_ref, bot_instance, notifier_ref, admin_ids
    dp_instance = dp
    scanner_ref = scanner
    bot_instance = bot
    notifier_ref = notifier
    admin_ids = set(admins) if admins else set()
    
    dp.message.register(cmd_start, Command("start"))
    dp.message.register(cmd_help, Command("help"))
    dp.message.register(cmd_scanner, Command("scanner"))
    dp.message.register(cmd_surebets, Command("surebets"))
    dp.message.register(cmd_top, Command("top"))
    dp.message.register(cmd_stats, Command("stats"))
    dp.message.register(cmd_bonuses, Command("bonuses"))
    dp.message.register(cmd_calculator, Command("calculator"))
    dp.message.register(handle_calc, Command("calc"))
    dp.message.register(cmd_settings, Command("settings"))
    dp.message.register(cmd_subscribe, Command("subscribe"))
    dp.message.register(cmd_bet, Command("bet"))
    dp.message.register(cmd_mystats, Command("mystats"))
    
    dp.message.register(cmd_admin_broadcast, Command("admin"))
    dp.message.register(cmd_admin_stats, Command("admin_stats"))
    dp.message.register(cmd_admin_toggle_parser, Command("admin_toggle_parser"))
    
    dp.callback_query.register(callback_calculate, lambda c: c.data and c.data.startswith("calc_"))
    dp.callback_query.register(callback_place_bet, lambda c: c.data and c.data.startswith("bet_"))
    dp.callback_query.register(callback_ignore, lambda c: c.data and c.data.startswith("ignore_"))
    dp.callback_query.register(callback_subscribe_level, lambda c: c.data and c.data.startswith("sub_level_"))
    dp.callback_query.register(callback_subscribe_level, lambda c: c.data == "sub_unsubscribe")
    dp.callback_query.register(callback_admin_action, lambda c: c.data and c.data.startswith("admin_"))
    dp.callback_query.register(callback_scanner_control, lambda c: c.data and c.data.startswith("scanner_"))
    
    dp.inline_query.register(inline_search)
    
    logger.info("Telegram handlers registered")


async def cmd_start(message: types.Message):
    if notifier_ref and message.from_user:
        notifier_ref.add_subscriber(
            message.from_user.id,
            message.from_user.username or 'Unknown'
        )
    
    web_app_url = "https://ghost-imperium.web.app"
    
    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [
            InlineKeyboardButton(
                text="📊 Open Dashboard",
                web_app=types.WebAppInfo(url=web_app_url)
            ),
        ],
        [
            InlineKeyboardButton(
                text="🔔 Notifications",
                callback_data="sub_settings"
            ),
        ]
    ])
    
    await message.answer(
        "👻 <b>GHOST IMPERIUM</b>\n\n"
        "Professional fork scanner v2.0\n\n"
        "📋 <b>Commands:</b>\n"
        "/scanner - Scanner status\n"
        "/surebets - Fork list\n"
        "/top - Top forks\n"
        "/stats - Statistics\n"
        "/bonuses - Bookmaker bonuses\n"
        "/calculator - Fork calculator\n"
        "/subscribe - Enable notifications\n"
        "/mystats - Your personal stats\n"
        "/bet - Place bet\n"
        "/help - Help\n\n"
        "💡 <b>Tip:</b> Type @ghostbot in any chat to search for surebets!",
        reply_markup=keyboard
    )


async def cmd_help(message: types.Message):
    await message.answer(
        "📖 <b>Help</b>\n\n"
        "🔍 <b>Scanner</b> - automatically finds forks\n"
        "📊 <b>Forks</b> - list of arbitrage opportunities\n"
        "🧮 <b>Calculator</b> - calculate fork manually\n"
        "💰 <b>Bet</b> - place bet\n\n"
        "⚠️ Forks with profit > 5% are sent automatically\n\n"
        "🔍 <b>Inline Mode:</b>\n"
        "Type @ghostbot in any chat to search for surebets\n\n"
        "📊 <b>Mini App:</b>\n"
        "Open the web dashboard directly in Telegram via /start\n\n"
        "👤 <b>Your Stats:</b>\n"
        "Use /mystats to see your personal statistics"
    )


async def cmd_scanner(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    stats = scanner_ref.get_stats()
    
    status = "✅ Active" if stats.get('is_running') else "❌ Stopped"
    events = stats.get('total_events', 0)
    forks = stats.get('total_surebets', 0)
    cycle_time = stats.get('avg_cycle_time_ms', 0)
    
    await message.answer(
        f"📡 <b>Scanner Status</b>\n\n"
        f"Status: {status}\n"
        f"Cycles: {stats.get('total_cycles', 0)}\n"
        f"Events: {events}\n"
        f"Forks: {forks}\n"
        f"Avg cycle: {cycle_time:.0f}ms\n"
        f"Sources: {len(stats.get('sources', []))}"
    )


async def cmd_surebets(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    text = message.text or ""
    args = text.split()[1:] if len(text.split()) > 1 else []
    limit = int(args[0]) if args and args[0].isdigit() else 10
    
    surebets = scanner_ref.get_top_surebets(limit)
    
    if not surebets:
        await message.answer("😴 No forks found")
        return
    
    result_text = f"💰 <b>Forks</b> ({len(surebets)} found)\n\n"
    
    for i, sb in enumerate(surebets[:10], 1):
        profit = sb.get('profit_percent', 0)
        event = sb.get('event_name', 'Unknown')[:40]
        bks = ', '.join(sb.get('bookmakers', [])[:2])
        live = "🔴" if sb.get('is_live') else "📅"
        
        result_text += f"{i}. {live} {event}\n"
        result_text += f"   💰 +{profit:.2f}% | {bks}\n\n"
    
    await message.answer(result_text)


async def cmd_top(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    surebets = scanner_ref.get_top_surebets(5)
    
    if not surebets:
        await message.answer("😴 No forks found")
        return
    
    result_text = "🏆 <b>TOP FORKS</b>\n\n"
    
    for i, sb in enumerate(surebets, 1):
        profit = sb.get('profit_percent', 0)
        event = sb.get('event_name', 'Unknown')[:50]
        
        emoji = ["🥇", "🥈", "🥉", "4️⃣", "5️⃣"][i-1] if i <= 5 else f"{i}."
        
        result_text += f"{emoji} <b>+{profit:.2f}%</b>\n"
        result_text += f"   {event}\n"
        result_text += f"   {', '.join(sb.get('bookmakers', []))}\n\n"
    
    await message.answer(result_text)


async def cmd_stats(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    stats = scanner_ref.get_stats()
    sources = stats.get('parsers', {})
    
    result_text = "📈 <b>Statistics</b>\n\n"
    result_text += f"🔄 Cycles: {stats.get('total_cycles', 0)}\n"
    result_text += f"📊 Events: {stats.get('total_events', 0)}\n"
    result_text += f"💰 Forks: {stats.get('total_surebets', 0)}\n"
    result_text += f"⚡ Avg time: {stats.get('avg_cycle_time_ms', 0):.0f}ms\n\n"
    
    result_text += "📍 <b>Sources:</b>\n"
    for name, data in sources.items():
        events = data.get('events', 0)
        errors = data.get('errors', 0)
        status = "✅" if events > 0 else "❌"
        result_text += f"   {status} {name}: {events} events"
        if errors > 0:
            result_text += f" ({errors} errors)"
        result_text += "\n"
    
    await message.answer(result_text)


async def cmd_bonuses(message: types.Message):
    bonuses = [
        ("Winline", "100% first deposit", "up to 10,000₽", "Wager x10"),
        ("Olimp", "Freebet", "500₽", "Express 3+"),
        ("Pari", "100% deposit", "up to 15,000₽", "Wager x10"),
        ("Fonbet", "Freebet", "2,000₽", "Express 3+"),
        ("1xBet", "Deposit bonus", "3,000₽", "Wager x5"),
        ("Marathon", "Cashback", "Weekly", "Auto"),
        ("BetBoom", "100% deposit", "up to 20,000₽", "Wager x8"),
        ("Leon", "Freebet", "500₽", "Registration"),
    ]
    
    result_text = "🎁 <b>Bookmaker Bonuses</b>\n\n"
    
    for name, bonus, amount, cond in bonuses:
        result_text += f"🏅 <b>{name}</b>\n"
        result_text += f"   {bonus}: <b>{amount}</b>\n"
        result_text += f"   Conditions: {cond}\n\n"
    
    await message.answer(result_text)


async def cmd_calculator(message: types.Message):
    await message.answer(
        "🧮 <b>Fork Calculator</b>\n\n"
        "Enter odds separated by space:\n"
        "Example: <code>/calc 2.10 2.15</code>\n\n"
        "Or use the web calculator at:\n"
        "http://localhost:8000"
    )


async def handle_calc(message: types.Message):
    text = message.text or ""
    args = text.split()[1:]
    if len(args) < 2:
        await message.answer("❌ Enter at least 2 odds\nExample: /calc 2.10 2.15")
        return
    
    try:
        odds = [float(o.replace(',', '.')) for o in args]
        
        inverses = [1/o for o in odds]
        sum_inv = sum(inverses)
        
        if sum_inv >= 1:
            margin = (sum_inv - 1) * 100
            await message.answer(
                f"⚠️ <b>NO FORK</b>\n\n"
                f"Margin: {margin:.2f}%\n\n"
                f"For a fork, sum of inverse odds must be < 1"
            )
            return
        
        profit = (1/sum_inv - 1) * 100
        stakes = [10000 * inv / sum_inv for inv in inverses]
        
        result_text = f"✅ <b>FORK FOUND!</b>\n\n"
        result_text += f"💰 Profit: <b>+{profit:.2f}%</b>\n"
        result_text += f"📈 Profit ₽: {10000 * profit / 100:.0f}₽\n\n"
        result_text += "📊 Stakes for 10,000₽:\n"
        
        labels = ['P1', 'P2', 'X']
        for i, (o, s) in enumerate(zip(odds, stakes)):
            result_text += f"   {labels[i]}: K{o:.2f} → {s:.0f}₽\n"
        
        await message.answer(result_text)
        
    except ValueError:
        await message.answer("❌ Invalid odds format")


async def cmd_settings(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    config = scanner_ref.get_config()
    
    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [
            InlineKeyboardButton(text="🔴 Stop Scanner", callback_data="scanner_stop"),
            InlineKeyboardButton(text="🟢 Start Scanner", callback_data="scanner_start"),
        ],
        [
            InlineKeyboardButton(text="📊 Start/Stop", callback_data="scanner_toggle"),
        ]
    ])
    
    status = "🟢 Running" if scanner_ref.is_running else "🔴 Stopped"
    sources = ", ".join(config.get('enabled_sources', [])) or "None"
    
    await message.answer(
        f"⚙️ <b>Scanner Settings</b>\n\n"
        f"<b>Status:</b> {status}\n"
        f"<b>Min Profit:</b> {config.get('min_profit', 0.5)}%\n"
        f"<b>Cycle Interval:</b> {config.get('cycle_interval', 3)}s\n"
        f"<b>Live Only:</b> {'Yes' if config.get('live_only') else 'No'}\n"
        f"<b>Sources:</b> {sources}\n\n"
        f"<b>Use /subscribe</b> to configure notifications\n"
        f"<b>Use /scanner</b> for detailed status",
        reply_markup=keyboard
    )


async def cmd_subscribe(message: types.Message):
    if not notifier_ref:
        await message.answer("⏳ Notification system initializing...")
        return
    
    user_id = message.from_user.id if message.from_user else 0
    username = message.from_user.username or 'Unknown' if message.from_user else 'Unknown'
    
    sub = notifier_ref.get_subscriber(user_id)
    current_level = sub.level if sub else NotificationLevel.HIGH
    
    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [
            InlineKeyboardButton(
                text="✅ All (0%+)" if current_level == NotificationLevel.ALL else "📊 All (0%+)",
                callback_data="sub_level_all"
            ),
        ],
        [
            InlineKeyboardButton(
                text="✅ Low (1%+)" if current_level == NotificationLevel.LOW else "📈 Low (1%+)",
                callback_data="sub_level_low"
            ),
        ],
        [
            InlineKeyboardButton(
                text="✅ Medium (3%+)" if current_level == NotificationLevel.MEDIUM else "💰 Medium (3%+)",
                callback_data="sub_level_medium"
            ),
        ],
        [
            InlineKeyboardButton(
                text="✅ High (5%+)" if current_level == NotificationLevel.HIGH else "🔥 High (5%+)",
                callback_data="sub_level_high"
            ),
        ],
        [
            InlineKeyboardButton(
                text="✅ Critical (10%+)" if current_level == NotificationLevel.CRITICAL else "🚀 Critical (10%+)",
                callback_data="sub_level_critical"
            ),
        ],
        [
            InlineKeyboardButton(text="❌ Unsubscribe", callback_data="sub_unsubscribe"),
        ]
    ])
    
    await message.answer(
        "🔔 <b>Notification Settings</b>\n\n"
        "Choose your notification level:\n"
        "• <b>All</b> - every fork found\n"
        "• <b>Low</b> - forks with 1%+ profit\n"
        "• <b>Medium</b> - forks with 3%+ profit\n"
        "• <b>High</b> - forks with 5%+ profit\n"
        "• <b>Critical</b> - forks with 10%+ profit\n\n"
        f"Current level: <b>{current_level.label}</b>",
        reply_markup=keyboard
    )


async def callback_subscribe_level(callback: types.CallbackQuery):
    if not notifier_ref or not callback.data or not callback.message:
        await callback.answer("Notification system not ready", show_alert=True)
        return
    
    user_id = callback.from_user.id if callback.from_user else 0
    username = callback.from_user.username or 'Unknown' if callback.from_user else 'Unknown'
    data = callback.data
    
    if data == "sub_unsubscribe":
        notifier_ref.remove_subscriber(user_id)
        await callback.answer("🔕 Unsubscribed from notifications")
        if isinstance(callback.message, types.Message):
            await callback.message.edit_text("🔕 <b>Unsubscribed</b>\n\nYou will no longer receive fork notifications.\nUse /subscribe to enable again.")
        return
    
    level_map = {
        "sub_level_all": NotificationLevel.ALL,
        "sub_level_low": NotificationLevel.LOW,
        "sub_level_medium": NotificationLevel.MEDIUM,
        "sub_level_high": NotificationLevel.HIGH,
        "sub_level_critical": NotificationLevel.CRITICAL,
    }
    
    level = level_map.get(data)
    if not level:
        await callback.answer("Invalid option", show_alert=True)
        return
    
    notifier_ref.add_subscriber(user_id, username, level)
    
    await callback.answer(f"✅ Notifications: {level.label}")
    await callback.message.edit_text(
        f"✅ <b>Notifications Updated</b>\n\n"
        f"Level: <b>{level.label}</b>\n"
        f"You will receive forks with profit ≥ {level.min_profit}%\n\n"
        "Use /subscribe to change settings again."
    )


async def cmd_bet(message: types.Message):
    if not scanner_ref:
        await message.answer("⏳ Scanner initializing...")
        return
    
    surebets = scanner_ref.get_top_surebets(3)
    
    if not surebets:
        await message.answer("😴 No forks available")
        return
    
    result_text = "💰 <b>Quick Bet</b>\n\n"
    
    for i, sb in enumerate(surebets[:3], 1):
        profit = sb.get('profit_percent', 0)
        event = sb.get('event_name', 'Unknown')[:30]
        total = sb.get('total_stake', 10000)
        
        result_text += f"{i}. +{profit:.2f}% | {event}\n"
        result_text += f"   Stake: {total:.0f}₽ | Profit: {sb.get('estimated_profit', 0):.0f}₽\n\n"
    
    result_text += "\nℹ️ Use the web interface for full betting"
    
    await message.answer(result_text)


async def send_notification(chat_id: int, surebet: dict):
    if notifier_ref:
        await notifier_ref.send_notification(surebet)
        return
    
    if not bot_instance:
        return
    
    profit = surebet.get('profit_percent', 0)
    
    event = surebet.get('event_name', 'Unknown')
    bks = ', '.join(surebet.get('bookmakers', []))
    profit_rub = surebet.get('estimated_profit', 0)
    
    text = (
        f"💰 <b>FORCE Fork {profit:.2f}%</b>\n\n"
        f"🏆 {event}\n"
        f"📍 {bks}\n"
        f"💵 Profit: {profit_rub:.0f}₽\n\n"
        "⚡ Act fast!"
    )
    
    try:
        await bot_instance.send_message(chat_id, text)
    except Exception as e:
        logger.error(f"Notification error: {e}")


async def callback_calculate(callback: types.CallbackQuery):
    if not callback.data or not callback.message:
        await callback.answer("Invalid request")
        return
        
    surebet_id = callback.data.replace("calc_", "")
    
    if not scanner_ref:
        await callback.answer("Scanner not available", show_alert=True)
        return
    
    surebets = scanner_ref.get_top_surebets(50)
    surebet = next((sb for sb in surebets if sb.get('id') == surebet_id), None)
    
    if not surebet:
        await callback.answer("Fork not found", show_alert=True)
        return
    
    legs = surebet.get('legs', [])
    profit = surebet.get('profit_percent', 0)
    total_stake = surebet.get('total_stake', 10000)
    
    calc_text = f"🧮 <b>Calculator</b>\n\n"
    calc_text += f"💰 Profit: <b>+{profit:.2f}%</b>\n"
    calc_text += f"📊 Total stake: {total_stake:.0f}₽\n\n"
    calc_text += f"📋 <b>Stakes:</b>\n"
    
    for leg in legs:
        bm = leg.get('bookmaker', 'Unknown')
        odds = leg.get('odds', 0)
        selection = leg.get('selection', '')
        stake = leg.get('calculated_stake', 0)
        calc_text += f"   • {bm}: {selection} @ {odds:.2f} → {stake:.0f}₽\n"
    
    calc_text += f"\n💵 Estimated profit: {surebet.get('estimated_profit', 0):.0f}₽"
    
    if notifier_ref and callback.from_user:
        notifier_ref.track_calculate_click(callback.from_user.id)
    
    await callback.answer()
    await callback.message.answer(calc_text, parse_mode='HTML')


async def callback_place_bet(callback: types.CallbackQuery):
    if not callback.data or not callback.message:
        await callback.answer("Invalid request")
        return
        
    surebet_id = callback.data.replace("bet_", "")
    
    await callback.answer("Opening bet interface...", show_alert=False)
    
    web_app_url = "https://ghost-imperium.web.app"
    keyboard = InlineKeyboardMarkup(inline_keyboard=[
        [
            InlineKeyboardButton(
                text="📊 Open Betting Panel",
                web_app=types.WebAppInfo(url=web_app_url)
            ),
        ]
    ])
    
    await callback.message.answer(
        f"💰 <b>Placing Bet</b>\n\n"
        f"Surebet ID: <code>{surebet_id}</code>\n\n"
        "Use the web interface for full betting:",
        reply_markup=keyboard
    )


async def callback_ignore(callback: types.CallbackQuery):
    if not callback.data or not callback.message:
        await callback.answer("Invalid request")
        return
        
    surebet_id = callback.data.replace("ignore_", "")
    
    await callback.answer("Fork ignored", show_alert=False)
    
    try:
        await callback.message.delete()
    except Exception:
        await callback.message.edit_text("❌ <b>Fork Ignored</b>")


async def callback_scanner_control(callback: types.CallbackQuery):
    if not callback.data:
        await callback.answer("Invalid request")
        return
    
    action = callback.data.replace("scanner_", "")
    
    if action == "start":
        if scanner_ref and not scanner_ref.is_running:
            await scanner_ref.start()
            await callback.answer("Scanner started")
        else:
            await callback.answer("Scanner already running")
    elif action == "stop":
        if scanner_ref and scanner_ref.is_running:
            await scanner_ref.stop()
            await callback.answer("Scanner stopped")
        else:
            await callback.answer("Scanner already stopped")
    elif action == "toggle":
        if scanner_ref:
            if scanner_ref.is_running:
                await scanner_ref.stop()
                await callback.answer("Scanner stopped")
            else:
                await scanner_ref.start()
                await callback.answer("Scanner started")
        else:
            await callback.answer("Scanner not initialized")
    
    if callback.message:
        await callback.message.edit_reply_markup(None)


async def inline_search(inline_query: InlineQuery):
    if not scanner_ref:
        await inline_query.answer([], switch_pm_text="Scanner not ready", switch_pm_parameter="start")
        return
    
    query = inline_query.query.strip().lower()
    
    surebets = scanner_ref.get_top_surebets(50)
    
    if query:
        surebets = [
            sb for sb in surebets
            if query in sb.get('event_name', '').lower()
            or query in ' '.join(sb.get('bookmakers', [])).lower()
            or query in sb.get('sport', '').lower()
        ]
    
    results = []
    for sb in surebets[:20]:
        surebet_id = sb.get('id', 'unknown')
        profit = sb.get('profit_percent', 0)
        event = sb.get('event_name', 'Unknown')
        bks = ' vs '.join(sb.get('bookmakers', []))
        sport = sb.get('sport', 'Unknown')
        
        emoji = "🚀" if profit > 5 else "🔥" if profit > 3 else "💰"
        
        title = f"{emoji} {profit:.2f}% — {event[:40]}"
        description = f"{bks} | {sport}"
        
        message_text = (
            f"{emoji} <b>FORK {profit:.2f}%</b>\n\n"
            f"🏆 {event}\n"
            f"🏅 {sport}\n"
            f"📍 {bks}\n\n"
            f"💵 Stake: {sb.get('total_stake', 0):.0f}₽\n"
            f"💰 Profit: +{sb.get('estimated_profit', 0):.0f}₽\n\n"
            f"⚡ Act fast!"
        )
        
        results.append(
            InlineQueryResultArticle(
                id=surebet_id,
                title=title,
                description=description,
                input_message_content=InputTextMessageContent(
                    message_text=message_text,
                    parse_mode='HTML'
                ),
                url=f"https://t.me/{bot_instance.username}?start=surebet_{surebet_id}" if bot_instance else None,
            )
        )
    
    await inline_query.answer(
        results,
        cache_time=10,
        switch_pm_text="Open Ghost Imperium" if not query else None,
        switch_pm_parameter="start" if not query else None,
    )


async def cmd_mystats(message: types.Message):
    if not notifier_ref or not message.from_user:
        await message.answer("⏳ System initializing...")
        return
    
    user_id = message.from_user.id
    user_stats = notifier_ref.get_user_stats(user_id)
    
    if not user_stats:
        await message.answer(
            "👤 <b>Your Stats</b>\n\n"
            "You haven't subscribed to notifications yet.\n"
            "Use /subscribe to start tracking your stats!"
        )
        return
    
    top_bks_text = ""
    if user_stats['top_bookmakers']:
        top_bks_text = "\n📍 <b>Top Bookmakers:</b>\n"
        for bm, count in user_stats['top_bookmakers']:
            top_bks_text += f"   • {bm}: {count} views\n"
    
    await message.answer(
        f"👤 <b>Your Stats</b>\n\n"
        f"📊 Surebets received: <b>{user_stats['total_surebets_received']}</b>\n"
        f"🧮 Calculates clicked: <b>{user_stats['total_calculates_clicked']}</b>\n"
        f"🔔 Subscription: <b>{user_stats['level']}</b>\n"
        f"⏱️ Duration: <b>{user_stats['subscription_duration']}</b>\n"
        f"{top_bks_text}"
    )


def _is_admin(user_id: int) -> bool:
    return user_id in admin_ids


async def cmd_admin_broadcast(message: types.Message):
    if not message.from_user or not _is_admin(message.from_user.id):
        await message.answer("🔒 Admin access required")
        return
    
    text_parts = message.text.split(maxsplit=1)
    if len(text_parts) < 2:
        await message.answer(
            "📢 <b>Broadcast</b>\n\n"
            "Usage: /admin broadcast <message>\n"
            "Example: /admin broadcast Server maintenance at 3:00 AM"
        )
        return
    
    broadcast_text = text_parts[1]
    
    if not notifier_ref:
        await message.answer("⏳ Notification system not ready")
        return
    
    await message.answer("📢 Sending broadcast...")
    
    result = await notifier_ref.broadcast_message(broadcast_text)
    
    await message.answer(
        f"✅ <b>Broadcast Complete</b>\n\n"
        f"Sent: {result['sent']}\n"
        f"Failed: {result['failed']}"
    )


async def cmd_admin_stats(message: types.Message):
    if not message.from_user or not _is_admin(message.from_user.id):
        await message.answer("🔒 Admin access required")
        return
    
    if not notifier_ref:
        await message.answer("⏳ Notification system not ready")
        return
    
    stats = notifier_ref.get_stats()
    
    total_users = stats['total_subscribers']
    active_users = stats['active_subscribers']
    
    level_breakdown = "\n".join(
        f"   • {level}: {count} users"
        for level, count in stats['levels'].items()
        if count > 0
    )
    
    await message.answer(
        f"📊 <b>Bot Admin Stats</b>\n\n"
        f"👥 Total users: <b>{total_users}</b>\n"
        f"✅ Active subscribers: <b>{active_users}</b>\n"
        f"📬 Notifications sent: <b>{stats['notifications_sent']}</b>\n\n"
        f"📋 <b>Notification Levels:</b>\n"
        f"{level_breakdown}"
    )


async def cmd_admin_toggle_parser(message: types.Message):
    if not message.from_user or not _is_admin(message.from_user.id):
        await message.answer("🔒 Admin access required")
        return
    
    text_parts = message.text.split()
    if len(text_parts) < 3:
        await message.answer(
            "🔧 <b>Toggle Parser</b>\n\n"
            "Usage: /admin toggle_parser <parser_name> <on|off>\n"
            "Example: /admin toggle_parser winline off"
        )
        return
    
    parser_name = text_parts[1].lower()
    action = text_parts[2].lower()
    
    if action not in ('on', 'off'):
        await message.answer("❌ Action must be 'on' or 'off'")
        return
    
    enabled = action == 'on'
    parser_states[parser_name] = enabled
    
    status = "✅ enabled" if enabled else "❌ disabled"
    
    await message.answer(f"🔧 Parser <b>{parser_name}</b> {status}")


async def callback_admin_action(callback: types.CallbackQuery):
    if not callback.data or not callback.from_user:
        return
    
    if not _is_admin(callback.from_user.id):
        await callback.answer("🔒 Admin access required", show_alert=True)
        return
    
    data = callback.data
    
    if data == "admin_toggle_smart_filter":
        if notifier_ref:
            current = notifier_ref._smart_filter_enabled
            notifier_ref.enable_smart_filter(not current)
            status = "enabled" if not current else "disabled"
            await callback.answer(f"Smart filter {status}")
            await callback.message.edit_text(
                f"✅ Smart filter {'enabled' if not current else 'disabled'}\n\n"
                f"Profit sweet spot: 2-8%\n"
                f"Reliable bookmakers only\n"
                f"Odds stability: 30s"
            )
    elif data == "admin_set_channel":
        if not callback.message:
            return
        await callback.message.answer(
            "📢 Send me the channel ID (e.g., @my_channel or -1001234567890)"
        )
