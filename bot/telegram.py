# bot/telegram.py
import os
import asyncio
import logging
from typing import Dict, List, Optional, Callable
from dataclasses import dataclass
from datetime import datetime

from aiogram import Bot, Dispatcher, types
from aiogram.filters import Command
from aiogram.types import InlineKeyboardMarkup, InlineKeyboardButton

logger = logging.getLogger(__name__)


@dataclass
class TelegramUser:
    id: int
    username: str
    subscribed: bool = True
    min_profit: float = 5.0
    chat_id: Optional[int] = None


class GhostTelegramBot:
    """
    Telegram bot for Ghost Imperium
    Sends fork notifications and provides commands
    """
    
    def __init__(self, token: str = None):
        self.token = token or os.getenv('TELEGRAM_BOT_TOKEN', '')
        self.bot: Optional[Bot] = None
        self.dp: Optional[Dispatcher] = None
        self.users: Dict[int, TelegramUser] = {}
        self.surebet_callback: Optional[Callable] = None
        self.is_running = False
    
    async def init(self):
        if not self.token:
            logger.warning("Telegram bot token not configured")
            return
        
        self.bot = Bot(token=self.token)
        self.dp = Dispatcher()
        self._register_handlers()
    
    def _register_handlers(self):
        if not self.dp:
            return
        
        self.dp.message.register(self.cmd_start, Command('start'))
        self.dp.message.register(self.cmd_help, Command('help'))
        self.dp.message.register(self.cmd_surebets, Command('surebets'))
        self.dp.message.register(self.cmd_stats, Command('stats'))
        self.dp.message.register(self.cmd_bonuses, Command('bonuses'))
        self.dp.message.register(self.cmd_settings, Command('settings'))
        self.dp.message.register(self.cmd_calculator, Command('calculator'))
        self.dp.message.register(self.cmd_subscribe, Command('subscribe'))
    
    async def start(self):
        if not self.bot or not self.dp:
            await self.init()
        
        if not self.token:
            logger.error("Cannot start bot: no token")
            return
        
        self.is_running = True
        logger.info("Telegram bot started")
        
        try:
            await self.dp.start_polling(self.bot)
        except Exception as e:
            logger.error(f"Bot error: {e}")
    
    async def stop(self):
        self.is_running = False
        if self.bot:
            await self.bot.session.close()
    
    async def cmd_start(self, message: types.Message):
        user_id = message.from_user.id
        
        self.users[user_id] = TelegramUser(
            id=user_id,
            username=message.from_user.username or 'Unknown',
            chat_id=message.chat.id
        )
        
        await message.answer(
            "👻 <b>Ghost Imperium Bot</b>\n\n"
            "Professional fork scanner is ready!\n\n"
            "📋 <b>Commands:</b>\n"
            "/surebets - Active forks\n"
            "/stats - Statistics\n"
            "/bonuses - Bookmaker bonuses\n"
            "/calculator - Fork calculator\n"
            "/settings - Settings\n"
            "/help - Help",
            parse_mode='HTML'
        )
    
    async def cmd_help(self, message: types.Message):
        await message.answer(
            "📖 <b>Help</b>\n\n"
            "🔍 <b>Scanner</b> - automatically finds forks\n"
            "💰 <b>Forks</b> - arbitrage opportunities\n"
            "🧮 <b>Calculator</b> - calculate stakes\n"
            "🎁 <b>Bonuses</b> - bookmaker bonuses\n\n"
            "⚠️ Forks > 5% are sent automatically",
            parse_mode='HTML'
        )
    
    async def cmd_surebets(self, message: types.Message):
        if self.surebet_callback:
            surebets = self.surebet_callback()
            
            if not surebets:
                await message.answer("😴 No forks found")
                return
            
            text = f"💰 <b>Active Forks</b> ({len(surebets)})\n\n"
            
            for i, sb in enumerate(surebets[:10], 1):
                profit = sb.get('profit_percent', 0)
                event = sb.get('event_name', 'Unknown')[:40]
                bks = ', '.join(sb.get('bookmakers', [])[:2])
                
                text += f"{i}. {event}\n"
                text += f"   💰 +{profit:.2f}% | {bks}\n\n"
            
            await message.answer(text, parse_mode='HTML')
        else:
            await message.answer("🔄 Scanner is initializing...")
    
    async def cmd_stats(self, message: types.Message):
        await message.answer(
            "📊 <b>Statistics</b>\n\n"
            "Scanner is running\n"
            "All systems operational",
            parse_mode='HTML'
        )
    
    async def cmd_bonuses(self, message: types.Message):
        bonuses = [
            ("🏅 Winline", "100% до 10,000₽", "Вейджер x10"),
            ("🏅 Olimp", "Фрибет 500₽", "Экспресс 3+"),
            ("🏅 Pari", "100% до 15,000₽", "Вейджер x10"),
            ("🏅 Fonbet", "Фрибет 2,000₽", "Экспресс 3+"),
            ("🏅 1xBet", "3,000₽", "Вейджер x5"),
        ]
        
        text = "🎁 <b>Bookmaker Bonuses</b>\n\n"
        for name, bonus, cond in bonuses:
            text += f"{name}\n"
            text += f"   {bonus}\n"
            text += f"   📋 {cond}\n\n"
        
        await message.answer(text, parse_mode='HTML')
    
    async def cmd_settings(self, message: types.Message):
        user = self.users.get(message.from_user.id)
        
        await message.answer(
            "⚙️ <b>Settings</b>\n\n"
            f"Min profit: {user.min_profit if user else 5.0}%\n"
            "Status: Active\n\n"
            "Use /subscribe to change settings",
            parse_mode='HTML'
        )
    
    async def cmd_calculator(self, message: types.Message):
        await message.answer(
            "🧮 <b>Calculator</b>\n\n"
            "Send odds in format:\n"
            "<code>2.10 2.15</code>\n\n"
            "Or use the web calculator:\n"
            "http://localhost:8000/web",
            parse_mode='HTML'
        )
    
    async def cmd_subscribe(self, message: types.Message):
        user = self.users.get(message.from_user.id)
        if user:
            user.subscribed = not user.subscribed
            status = "enabled" if user.subscribed else "disabled"
            await message.answer(f"🔔 Notifications {status}")
        else:
            await message.answer("Use /start first")
    
    async def send_surebet_alert(self, surebet: Dict):
        if not self.bot:
            return
        
        for user in self.users.values():
            if not user.subscribed:
                continue
            
            profit = surebet.get('profit_percent', 0)
            if profit < user.min_profit:
                continue
            
            event = surebet.get('event_name', 'Unknown')
            bks = ' vs '.join(surebet.get('bookmakers', [])[:2])
            estimated = surebet.get('estimated_profit', 0)
            
            keyboard = InlineKeyboardMarkup(inline_keyboard=[
                [InlineKeyboardButton(text="💰 Bet Now", url=f"https://example.com/bet/{surebet.get('id')}")]
            ])
            
            text = (
                f"💰 <b>FORK {profit:.2f}%</b>\n\n"
                f"🏆 {event}\n"
                f"📍 {bks}\n"
                f"💵 Profit: +{estimated:.0f}₽\n\n"
                "⚡ Bet fast!"
            )
            
            try:
                if user.chat_id:
                    await self.bot.send_message(
                        user.chat_id,
                        text,
                        parse_mode='HTML',
                        reply_markup=keyboard
                    )
            except Exception as e:
                logger.error(f"Failed to send message to {user.chat_id}: {e}")
    
    def set_surebet_callback(self, callback: Callable):
        self.surebet_callback = callback
