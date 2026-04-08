# ghost_imperium.py
"""
Ghost Imperium - Main entry point
Starts: Scanner + API + Telegram Bot
"""
import asyncio
import logging
import signal
import sys
from typing import Optional
import os

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN", "")


async def run_all():
    """Run Scanner + API + Telegram Bot together"""
    from services.database import Database
    from scanner.engine import GhostScanner, ScannerConfig
    from bot.notifications import SurebetNotifier
    
    database = Database()
    await database.init()
    
    config = ScannerConfig(
        min_profit=0.5,
        cycle_interval=3.0,
        max_events_per_source=200,
        cache_ttl=10.0,
        live_only=False,
    )
    
    scanner = GhostScanner(database, config)
    
    notifier: Optional[SurebetNotifier] = None
    
    if BOT_TOKEN:
        from aiogram import Bot, Dispatcher
        from aiogram.fsm.storage.memory import MemoryStorage
        from bot.handlers import register_handlers
        
        bot = Bot(token=BOT_TOKEN)
        dp = Dispatcher(storage=MemoryStorage())
        
        notifier = SurebetNotifier(bot=bot)
        notifier.enable_smart_filter(True)
        notifier.set_sweet_spot(2.0, 8.0)
        
        def on_new_surebets(surebets):
            if notifier:
                asyncio.create_task(notify_surebets(notifier, surebets))
        
        scanner.subscribe(on_new_surebets)
        register_handlers(dp, scanner, bot, notifier)
        
        logger.info("Starting Telegram Bot...")
        bot_task = asyncio.create_task(dp.start_polling(bot))
    else:
        logger.warning("TELEGRAM_BOT_TOKEN not set - Bot disabled")
    
    logger.info("Starting Scanner...")
    await scanner.start()
    
    logger.info("=" * 50)
    logger.info("👻 Ghost Imperium is running!")
    logger.info("=" * 50)
    logger.info("Press Ctrl+C to stop")
    logger.info("")
    
    try:
        while scanner.is_running:
            await asyncio.sleep(5)
            
            stats = scanner.get_stats()
            if stats.get('total_cycles', 0) > 0:
                logger.info(
                    f"Scanner: {stats['total_events']} events, "
                    f"{stats['total_surebets']} surebets, "
                    f"{stats['last_cycle_time_ms']}ms/cycle"
                )
    except asyncio.CancelledError:
        pass
    finally:
        logger.info("Stopping...")
        await scanner.stop()
        
        if notifier:
            try:
                await bot.session.close()
            except:
                pass


async def notify_surebets(notifier: SurebetNotifier, surebets: list):
    """Send notifications for new surebets"""
    for sb in surebets:
        profit = sb.get('profit_percent', 0)
        if profit >= 5.0:
            try:
                await notifier.send_notification(sb)
            except Exception as e:
                logger.error(f"Failed to send notification: {e}")


def signal_handler(sig, frame):
    logger.info("Received signal to stop...")
    sys.exit(0)


if __name__ == "__main__":
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    logger.info("=" * 50)
    logger.info("👻 Ghost Imperium Starting...")
    logger.info("=" * 50)
    
    try:
        asyncio.run(run_all())
    except KeyboardInterrupt:
        logger.info("Interrupted by user")
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        raise
