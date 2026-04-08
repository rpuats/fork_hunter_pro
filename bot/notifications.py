# bot/notifications.py
import logging
from typing import Dict, List, Optional, Set
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
import time

from aiogram import Bot
from aiogram.types import InlineKeyboardMarkup, InlineKeyboardButton

logger = logging.getLogger(__name__)


class NotificationLevel(Enum):
    ALL = "all"
    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"
    CRITICAL = "critical"

    @property
    def min_profit(self) -> float:
        return {
            NotificationLevel.ALL: 0.0,
            NotificationLevel.LOW: 1.0,
            NotificationLevel.MEDIUM: 3.0,
            NotificationLevel.HIGH: 5.0,
            NotificationLevel.CRITICAL: 10.0,
        }[self]

    @property
    def emoji(self) -> str:
        return {
            NotificationLevel.ALL: "📊",
            NotificationLevel.LOW: "📈",
            NotificationLevel.MEDIUM: "💰",
            NotificationLevel.HIGH: "🔥",
            NotificationLevel.CRITICAL: "🚀",
        }[self]

    @property
    def label(self) -> str:
        return {
            NotificationLevel.ALL: "All Forks (0%+)",
            NotificationLevel.LOW: "Low (1%+)",
            NotificationLevel.MEDIUM: "Medium (3%+)",
            NotificationLevel.HIGH: "High (5%+)",
            NotificationLevel.CRITICAL: "Critical (10%+)",
        }[self]


@dataclass
class Subscriber:
    chat_id: int
    username: str = "Unknown"
    level: NotificationLevel = NotificationLevel.HIGH
    subscribed: bool = True
    created_at: datetime = field(default_factory=datetime.now)
    total_surebets_received: int = 0
    total_calculates_clicked: int = 0
    bookmaker_views: Dict[str, int] = field(default_factory=dict)


@dataclass
class NotificationRecord:
    surebet_id: str
    chat_id: int
    sent_at: datetime = field(default_factory=datetime.now)


@dataclass
class OddsSnapshot:
    surebet_id: str
    odds: List[float]
    timestamp: float = field(default_factory=time.time)


class SurebetNotifier:
    """
    Sends formatted surebet notifications to Telegram
    Supports different notification levels and tracks sent notifications
    Smart filtering: profit sweet spot, reliable bookmakers, stable odds
    """

    def __init__(self, bot: Bot, max_history_size: int = 1000, channel_id: Optional[str] = None):
        self.bot = bot
        self.subscribers: Dict[int, Subscriber] = {}
        self.sent_notifications: List[NotificationRecord] = []
        self.max_history_size = max_history_size
        self._cleanup_interval = 300
        self._last_cleanup = datetime.now()
        self.channel_id = channel_id
        self.reliable_bookmakers: Set[str] = {
            "winline", "olimp", "pari", "betboom", "leon", "1xstavka"
        }
        self.min_reliability_score: float = 0.6
        self._odds_history: Dict[str, List[OddsSnapshot]] = {}
        self._stability_window: float = 30.0
        self._sweet_spot_min: float = 2.0
        self._sweet_spot_max: float = 8.0
        self._smart_filter_enabled: bool = False

    def enable_smart_filter(self, enabled: bool = True):
        self._smart_filter_enabled = enabled

    def set_sweet_spot(self, min_profit: float = 2.0, max_profit: float = 8.0):
        self._sweet_spot_min = min_profit
        self._sweet_spot_max = max_profit

    def set_reliable_bookmakers(self, bookmakers: Set[str]):
        self.reliable_bookmakers = bookmakers

    def _check_odds_stability(self, surebet_id: str, current_odds: List[float]) -> bool:
        if surebet_id not in self._odds_history:
            self._odds_history[surebet_id] = []
        
        now = time.time()
        self._odds_history[surebet_id].append(OddsSnapshot(
            surebet_id=surebet_id,
            odds=current_odds.copy(),
            timestamp=now
        ))
        
        cutoff = now - self._stability_window
        self._odds_history[surebet_id] = [
            snap for snap in self._odds_history[surebet_id]
            if snap.timestamp > cutoff
        ]
        
        history = self._odds_history[surebet_id]
        if len(history) < 2:
            return not self._smart_filter_enabled
        
        first_odds = history[0].odds
        if len(first_odds) != len(current_odds):
            return False
        
        for i, (old, new) in enumerate(zip(first_odds, current_odds)):
            if abs(old - new) > 0.01:
                return False
        
        return True

    def _check_bookmaker_reliability(self, surebet: dict) -> bool:
        bookmakers = surebet.get('bookmakers', [])
        if not self._smart_filter_enabled:
            return True
        
        reliable_count = sum(
            1 for bm in bookmakers
            if bm.lower() in self.reliable_bookmakers
        )
        
        return reliable_count >= len(bookmakers) * self.min_reliability_score

    def _is_in_sweet_spot(self, profit: float) -> bool:
        if not self._smart_filter_enabled:
            return True
        return self._sweet_spot_min <= profit <= self._sweet_spot_max

    def add_subscriber(self, chat_id: int, username: str = "Unknown", level: NotificationLevel = NotificationLevel.HIGH) -> Subscriber:
        if chat_id in self.subscribers:
            sub = self.subscribers[chat_id]
            sub.subscribed = True
            sub.level = level
            sub.username = username
            return sub
        
        sub = Subscriber(chat_id=chat_id, username=username, level=level)
        self.subscribers[chat_id] = sub
        logger.info(f"New subscriber: {username} (chat_id={chat_id}, level={level.value})")
        return sub

    def remove_subscriber(self, chat_id: int) -> bool:
        if chat_id in self.subscribers:
            self.subscribers[chat_id].subscribed = False
            logger.info(f"Subscriber unsubscribed: chat_id={chat_id}")
            return True
        return False

    def update_level(self, chat_id: int, level: NotificationLevel) -> bool:
        if chat_id in self.subscribers:
            self.subscribers[chat_id].level = level
            logger.info(f"Subscriber level updated: chat_id={chat_id}, level={level.value}")
            return True
        return False

    def get_subscriber(self, chat_id: int) -> Optional[Subscriber]:
        return self.subscribers.get(chat_id)

    def is_already_sent(self, surebet_id: str, chat_id: int) -> bool:
        cutoff = datetime.now() - timedelta(hours=1)
        for record in self.sent_notifications:
            if record.surebet_id == surebet_id and record.chat_id == chat_id and record.sent_at > cutoff:
                return True
        return False

    def mark_sent(self, surebet_id: str, chat_id: int):
        self.sent_notifications.append(NotificationRecord(surebet_id=surebet_id, chat_id=chat_id))
        if len(self.sent_notifications) > self.max_history_size:
            self.sent_notifications = self.sent_notifications[-self.max_history_size:]

    def _cleanup_old_notifications(self):
        cutoff = datetime.now() - timedelta(hours=1)
        self.sent_notifications = [r for r in self.sent_notifications if r.sent_at > cutoff]
        self._last_cleanup = datetime.now()

    def _build_notification_keyboard(self, surebet: dict, inline_mode: bool = False) -> InlineKeyboardMarkup:
        surebet_id = surebet.get('id', 'unknown')
        
        if inline_mode:
            return InlineKeyboardMarkup(inline_keyboard=[
                [
                    InlineKeyboardButton(
                        text="🧮 Calculate",
                        switch_inline_query_current_chat=surebet_id
                    ),
                    InlineKeyboardButton(
                        text="📊 Open in App",
                        url=f"https://t.me/{self.bot.username}?start=surebet_{surebet_id}"
                    ),
                ],
                [
                    InlineKeyboardButton(text="❌ Ignore", callback_data=f"ignore_{surebet_id}"),
                ]
            ])
        
        return InlineKeyboardMarkup(inline_keyboard=[
            [
                InlineKeyboardButton(text="🧮 Calculate", callback_data=f"calc_{surebet_id}"),
                InlineKeyboardButton(text="💰 Place Bet", callback_data=f"bet_{surebet_id}"),
            ],
            [
                InlineKeyboardButton(text="❌ Ignore", callback_data=f"ignore_{surebet_id}"),
            ]
        ])

    def _format_notification_text(self, surebet: dict, level: NotificationLevel) -> str:
        profit = surebet.get('profit_percent', 0)
        event = surebet.get('event_name', 'Unknown')
        sport = surebet.get('sport', 'Unknown')
        bks = ' vs '.join(surebet.get('bookmakers', []))
        total_stake = surebet.get('total_stake', 0)
        estimated_profit = surebet.get('estimated_profit', 0)
        
        legs_text = ""
        legs = surebet.get('legs', [])
        if legs:
            legs_text = "\n📋 <b>Legs:</b>\n"
            for leg in legs:
                bm = leg.get('bookmaker', 'Unknown')
                odds = leg.get('odds', 0)
                selection = leg.get('selection', '')
                stake = leg.get('calculated_stake', 0)
                legs_text += f"   • {bm}: {selection} @ {odds:.2f} ({stake:.0f}₽)\n"
        
        stability_badge = ""
        if self._smart_filter_enabled:
            stability_badge = "\n✅ Odds stable for 30s"
        
        text = (
            f"{level.emoji} <b>FORK {profit:.2f}%</b>\n\n"
            f"🏆 {event}\n"
            f"🏅 Sport: {sport}\n"
            f"📍 {bks}\n\n"
            f"💵 Total stake: {total_stake:.0f}₽\n"
            f"💰 Profit: +{estimated_profit:.0f}₽\n"
            f"{legs_text}"
            f"{stability_badge}\n"
            f"⚡ Act fast! Forks disappear quickly."
        )
        
        return text

    def _format_channel_post(self, surebet: dict) -> str:
        profit = surebet.get('profit_percent', 0)
        event = surebet.get('event_name', 'Unknown')
        sport = surebet.get('sport', 'Unknown')
        bks = ' vs '.join(surebet.get('bookmakers', []))
        total_stake = surebet.get('total_stake', 0)
        estimated_profit = surebet.get('estimated_profit', 0)
        
        emoji = "🚀" if profit > 5 else "🔥" if profit > 3 else "💰"
        
        legs_text = ""
        legs = surebet.get('legs', [])
        if legs:
            legs_text = "\n📋 <b>Legs:</b>\n"
            for leg in legs:
                bm = leg.get('bookmaker', 'Unknown')
                odds = leg.get('odds', 0)
                selection = leg.get('selection', '')
                legs_text += f"   • {bm}: {selection} @ {odds:.2f}\n"
        
        return (
            f"{emoji} <b>GHOST IMPERIUM | {profit:.2f}%</b>\n\n"
            f"🏆 {event}\n"
            f"🏅 {sport}\n"
            f"📍 {bks}\n\n"
            f"💵 Stake: {total_stake:.0f}₽ | Profit: +{estimated_profit:.0f}₽\n"
            f"{legs_text}\n"
            f"👻 @ghost_imperium_bot"
        )

    async def send_notification(self, surebet: dict) -> int:
        """
        Send surebet notification to all eligible subscribers
        Returns number of notifications sent
        """
        if not self.bot:
            logger.warning("Bot not initialized, skipping notification")
            return 0
        
        profit = surebet.get('profit_percent', 0)
        surebet_id = surebet.get('id', 'unknown')
        sent_count = 0
        
        self._cleanup_old_notifications()
        
        current_odds = [leg.get('odds', 0) for leg in surebet.get('legs', [])]
        odds_stable = self._check_odds_stability(surebet_id, current_odds)
        bks_reliable = self._check_bookmaker_reliability(surebet)
        in_sweet_spot = self._is_in_sweet_spot(profit)
        
        if self._smart_filter_enabled:
            if not odds_stable:
                logger.debug(f"Surebet {surebet_id} skipped: odds not stable")
                return 0
            if not bks_reliable:
                logger.debug(f"Surebet {surebet_id} skipped: unreliable bookmakers")
                return 0
            if not in_sweet_spot:
                logger.debug(f"Surebet {surebet_id} skipped: profit {profit:.2f}% outside sweet spot")
                return 0
        
        for chat_id, sub in self.subscribers.items():
            if not sub.subscribed:
                continue
            
            if profit < sub.level.min_profit:
                continue
            
            if self.is_already_sent(surebet_id, chat_id):
                continue
            
            text = self._format_notification_text(surebet, sub.level)
            keyboard = self._build_notification_keyboard(surebet)
            
            try:
                await self.bot.send_message(
                    chat_id=chat_id,
                    text=text,
                    parse_mode='HTML',
                    reply_markup=keyboard
                )
                self.mark_sent(surebet_id, chat_id)
                sub.total_surebets_received += 1
                sent_count += 1
                
                for bm in surebet.get('bookmakers', []):
                    sub.bookmaker_views[bm] = sub.bookmaker_views.get(bm, 0) + 1
                
                logger.debug(f"Notification sent to {chat_id} for surebet {surebet_id}")
            except Exception as e:
                logger.error(f"Failed to send notification to {chat_id}: {e}")
        
        if self.channel_id:
            try:
                channel_text = self._format_channel_post(surebet)
                channel_keyboard = InlineKeyboardMarkup(inline_keyboard=[
                    [
                        InlineKeyboardButton(
                            text="🧮 Calculate",
                            url=f"https://t.me/{self.bot.username}?start=surebet_{surebet_id}"
                        ),
                    ]
                ])
                await self.bot.send_message(
                    chat_id=self.channel_id,
                    text=channel_text,
                    parse_mode='HTML',
                    reply_markup=channel_keyboard
                )
            except Exception as e:
                logger.error(f"Failed to send to channel {self.channel_id}: {e}")
        
        if sent_count > 0:
            logger.info(f"Surebet {surebet_id} ({profit:.2f}%) sent to {sent_count} subscribers")
        
        return sent_count

    async def send_custom_message(self, chat_id: int, text: str, parse_mode: str = 'HTML', keyboard: Optional[InlineKeyboardMarkup] = None):
        """Send a custom message to a specific chat"""
        if not self.bot:
            return
        
        try:
            await self.bot.send_message(
                chat_id=chat_id,
                text=text,
                parse_mode=parse_mode,
                reply_markup=keyboard
            )
        except Exception as e:
            logger.error(f"Failed to send custom message to {chat_id}: {e}")

    async def broadcast_message(self, text: str, parse_mode: str = 'HTML', keyboard: Optional[InlineKeyboardMarkup] = None) -> Dict:
        """Broadcast a message to all active subscribers"""
        if not self.bot:
            return {'sent': 0, 'failed': 0}
        
        sent = 0
        failed = 0
        
        for chat_id, sub in self.subscribers.items():
            if not sub.subscribed:
                continue
            try:
                await self.bot.send_message(
                    chat_id=chat_id,
                    text=text,
                    parse_mode=parse_mode,
                    reply_markup=keyboard
                )
                sent += 1
            except Exception as e:
                logger.error(f"Broadcast failed for {chat_id}: {e}")
                failed += 1
        
        return {'sent': sent, 'failed': failed}

    def get_stats(self) -> dict:
        """Get notification statistics"""
        return {
            'total_subscribers': len(self.subscribers),
            'active_subscribers': sum(1 for s in self.subscribers.values() if s.subscribed),
            'notifications_sent': len(self.sent_notifications),
            'levels': {
                level.value: sum(1 for s in self.subscribers.values() if s.level == level and s.subscribed)
                for level in NotificationLevel
            }
        }

    def get_user_stats(self, chat_id: int) -> Optional[Dict]:
        """Get statistics for a specific user"""
        sub = self.subscribers.get(chat_id)
        if not sub:
            return None
        
        top_bks = sorted(
            sub.bookmaker_views.items(),
            key=lambda x: x[1],
            reverse=True
        )[:5]
        
        return {
            'username': sub.username,
            'subscribed': sub.subscribed,
            'level': sub.level.label,
            'total_surebets_received': sub.total_surebets_received,
            'total_calculates_clicked': sub.total_calculates_clicked,
            'subscription_duration': str(datetime.now() - sub.created_at).split('.')[0],
            'top_bookmakers': top_bks,
        }

    def track_calculate_click(self, chat_id: int):
        """Track when a user clicks the Calculate button"""
        sub = self.subscribers.get(chat_id)
        if sub:
            sub.total_calculates_clicked += 1

    def track_bookmaker_view(self, chat_id: int, bookmaker: str):
        """Track when a user views a bookmaker"""
        sub = self.subscribers.get(chat_id)
        if sub:
            sub.bookmaker_views[bookmaker] = sub.bookmaker_views.get(bookmaker, 0) + 1
