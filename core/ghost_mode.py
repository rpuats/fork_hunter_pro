# core/ghost_mode.py
"""
Ghost Mode - Anti-Fraud Protection System
Protects accounts from detection and limit cuts
"""
import random
import time
import hashlib
from typing import Dict, Optional, List
from dataclasses import dataclass, field
from collections import defaultdict
import logging

logger = logging.getLogger(__name__)


@dataclass
class AccountHeat:
    """Tracks account "temperature" - risk of limit cut"""
    bookmaker: str
    heat_level: float = 0.0  # 0-100
    wins_streak: int = 0
    loss_streak: int = 0
    bets_today: int = 0
    total_profit: float = 0.0
    last_bet_time: float = 0.0
    suspicious_patterns: List[str] = field(default_factory=list)
    
    def add_win(self, amount: float):
        self.wins_streak += 1
        self.loss_streak = 0
        self.total_profit += amount
        self.bets_today += 1
        self.last_bet_time = time.time()
        self._update_heat()
    
    def add_loss(self, amount: float):
        self.loss_streak += 1
        self.wins_streak = 0
        self.total_profit -= amount
        self.bets_today += 1
        self.last_bet_time = time.time()
        self._update_heat()
    
    def _update_heat(self):
        """Calculate heat level based on patterns"""
        heat = 0.0
        
        # Win streak increases heat
        if self.wins_streak >= 5:
            heat += 30
        elif self.wins_streak >= 3:
            heat += 15
        
        # High volume increases heat
        if self.bets_today > 20:
            heat += 25
        elif self.bets_today > 10:
            heat += 15
        elif self.bets_today > 5:
            heat += 5
        
        # Large profit increases heat
        if self.total_profit > 50000:
            heat += 30
        elif self.total_profit > 20000:
            heat += 20
        elif self.total_profit > 10000:
            heat += 10
        
        # Suspicious patterns
        heat += len(self.suspicious_patterns) * 10
        
        self.heat_level = min(100.0, max(0.0, heat))


class GhostMode:
    """
    Anti-fraud protection system that:
    - Rounds stakes to "natural" amounts
    - Randomizes bet timing
    - Obscures betting patterns
    - Tracks account heat
    """
    
    def __init__(self):
        self.accounts: Dict[str, AccountHeat] = defaultdict(
            lambda: AccountHeat(bookmaker="unknown")
        )
        self.stake_rounding_enabled = True
        self.timing_randomization_enabled = True
        self.pattern_obfuscation_enabled = True
        self.loss_simulation_enabled = True
    
    def get_account_heat(self, bookmaker: str) -> float:
        """Get heat level for an account (0-100)"""
        return self.accounts[bookmaker].heat_level
    
    def should_slow_down(self, bookmaker: str) -> bool:
        """Check if account should take a break"""
        heat = self.get_account_heat(bookmaker)
        return heat > 70
    
    def get_delay_seconds(self, bookmaker: str) -> float:
        """Get recommended delay between bets"""
        heat = self.get_account_heat(bookmaker)
        
        # Higher heat = longer delays
        if heat > 80:
            return random.uniform(120, 300)  # 2-5 min
        elif heat > 60:
            return random.uniform(60, 120)   # 1-2 min
        elif heat > 40:
            return random.uniform(30, 60)   # 30s-1min
        else:
            return random.uniform(3, 15)     # 3-15 sec
    
    def round_stake(self, stake: float, bookmaker: str) -> float:
        """
        Round stake to "natural" amounts to avoid detection.
        Instead of 1347 RUB, use 1350 or 1400.
        """
        if not self.stake_rounding_enabled:
            return stake
        
        # Determine rounding based on amount
        if stake < 100:
            return round(stake, -1)  # 53 -> 50
        elif stake < 500:
            return round(stake / 50) * 50  # 347 -> 350
        elif stake < 2000:
            return round(stake / 100) * 100  # 1347 -> 1300
        elif stake < 10000:
            return round(stake / 500) * 500  # 6750 -> 7000
        else:
            return round(stake / 1000) * 1000  # 28500 -> 28000
    
    def get_random_delay(self, base_seconds: float = 5.0) -> float:
        """Get random delay for bet timing randomization"""
        if not self.timing_randomization_enabled:
            return 0
        
        # Add randomness to mimic human behavior
        variance = base_seconds * 0.5
        return base_seconds + random.uniform(-variance, variance * 2)
    
    def should_place_decoy_bet(self, bookmaker: str) -> bool:
        """
        Determine if we should place a "wrong" bet to obscure pattern.
        Only when heat is medium-high and we have enough profit.
        """
        if not self.loss_simulation_enabled:
            return False
        
        heat = self.get_account_heat(bookmaker)
        
        # Higher heat + good profit = more decoy bets
        if heat > 70 and self.accounts[bookmaker].total_profit > 20000:
            return random.random() < 0.15  # 15% chance
        elif heat > 50 and self.accounts[bookmaker].total_profit > 10000:
            return random.random() < 0.08  # 8% chance
        
        return False
    
    def get_decoy_stake(self, real_stake: float) -> float:
        """Get stake for decoy bet (much smaller)"""
        return real_stake * random.uniform(0.05, 0.2)  # 5-20% of real stake
    
    def get_decoy_selection(self) -> Dict:
        """Get a "wrong" selection for decoy bet"""
        # Random sport and market type
        sports = ['football', 'tennis', 'basketball']
        selections = [
            {'market': '1x2', 'selection': random.choice(['П1', 'Ничья', 'П2'])},
            {'market': 'total', 'selection': random.choice(['ТБ 2.5', 'ТМ 2.5'])},
            {'market': 'handicap', 'selection': random.choice(['Ф1 +1.5', 'Ф2 -1.5'])},
        ]
        
        sport = random.choice(sports)
        sel = random.choice(selections)
        
        return {
            'sport': sport,
            **sel,
            'odds': round(random.uniform(1.5, 3.5), 2)
        }
    
    def record_bet_result(self, bookmaker: str, won: bool, amount: float):
        """Record result of a bet for heat tracking"""
        account = self.accounts[bookmaker]
        
        if won:
            account.add_win(amount)
        else:
            account.add_loss(amount)
        
        logger.info(
            f"Ghost Mode: {bookmaker} bet result - "
            f"{'WIN' if won else 'LOSS'} {amount:.0f} RUB, "
            f"Heat: {account.heat_level:.0f}%"
        )
    
    def get_remaining_budget(self, bookmaker: str, daily_limit: float = 50000) -> float:
        """Calculate remaining betting budget for today"""
        account = self.accounts[bookmaker]
        
        # If high heat, suggest taking a break
        if account.heat_level > 80:
            return 0
        
        # Calculate remaining based on heat
        heat_multiplier = 1.0 - (account.heat_level / 100) * 0.5
        
        # Check time since last bet
        time_since_last = time.time() - account.last_bet_time
        if time_since_last < 300:  # Less than 5 min
            heat_multiplier *= 0.5
        
        return daily_limit * heat_multiplier - account.total_profit
    
    def generate_session_id(self) -> str:
        """Generate unique session ID for fingerprint rotation"""
        return hashlib.md5(
            f"{time.time()}{random.random()}".encode()
        ).hexdigest()[:16]
    
    def get_fake_fingerprint(self) -> Dict:
        """Generate fake browser fingerprint for rotating"""
        screens = ['1920x1080', '1366x768', '1536x864', '2560x1440']
        timezones = ['Europe/Moscow', 'Europe/Kiev', 'Europe/Minsk']
        languages = ['ru-RU,ru', 'ru-RU,ru;q=0.9,en']
        
        return {
            'screen_resolution': random.choice(screens),
            'timezone': random.choice(timezones),
            'language': random.choice(languages),
            'user_agent': self._generate_user_agent(),
            'platform': random.choice(['Win32', 'MacIntel', 'Linux x86_64']),
        }
    
    def _generate_user_agent(self) -> str:
        """Generate random user agent"""
        chrome_versions = ['120.0.0.0', '121.0.0.0', '122.0.0.0']
        firefox_versions = ['121.0', '122.0', '123.0']
        
        if random.random() > 0.3:  # 70% Chrome
            return (
                f"Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                f"AppleWebKit/537.36 (KHTML, like Gecko) "
                f"Chrome/{random.choice(chrome_versions)} "
                f"Safari/537.36"
            )
        else:  # 30% Firefox
            return (
                f"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:{random.choice(firefox_versions)}) "
                f"Gecko/20100101 Firefox/{random.choice(firefox_versions)}"
            )
    
    def get_all_heats(self) -> Dict[str, float]:
        """Get heat levels for all tracked accounts"""
        return {
            bk: acc.heat_level 
            for bk, acc in self.accounts.items()
        }


# Global instance
ghost_mode = GhostMode()
