# bot/main.py
import os
import logging
from aiogram import Bot, Dispatcher
from aiogram.fsm.storage.memory import MemoryStorage
from aiogram.types import BotCommand
from dotenv import load_dotenv

load_dotenv()

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN", "")
ADMIN_IDS = [int(x.strip()) for x in os.getenv("ADMIN_IDS", "").split(",") if x.strip()]
FEED_CHANNEL_ID = os.getenv("FEED_CHANNEL_ID", "") or None

bot = Bot(token=BOT_TOKEN)
dp = Dispatcher(storage=MemoryStorage())


async def setup_bot_commands():
    """Set up bot command menu in Telegram"""
    commands = [
        BotCommand(command="start", description="Start bot & open dashboard"),
        BotCommand(command="surebets", description="List current forks"),
        BotCommand(command="top", description="Top 5 profitable forks"),
        BotCommand(command="scanner", description="Scanner status"),
        BotCommand(command="stats", description="System statistics"),
        BotCommand(command="mystats", description="Your personal stats"),
        BotCommand(command="subscribe", description="Notification settings"),
        BotCommand(command="calculator", description="Fork calculator"),
        BotCommand(command="calc", description="Quick calc: /calc 2.1 2.1"),
        BotCommand(command="bonuses", description="Bookmaker bonuses"),
        BotCommand(command="help", description="Help & features"),
    ]
    await bot.set_my_commands(commands)


async def main(scanner=None):
    from bot.handlers import register_handlers
    from bot.notifications import SurebetNotifier

    notifier = SurebetNotifier(bot=bot, channel_id=FEED_CHANNEL_ID)
    notifier.enable_smart_filter(True)
    notifier.set_sweet_spot(2.0, 8.0)

    await setup_bot_commands()

    register_handlers(dp, scanner, bot, notifier, admins=ADMIN_IDS)

    logger.info(f"Bot starting... (admins: {ADMIN_IDS}, feed channel: {FEED_CHANNEL_ID})")
    await dp.start_polling(bot)


if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
