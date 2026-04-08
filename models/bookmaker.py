# models/bookmaker.py
from datetime import datetime
from enum import Enum
from typing import Optional, Dict, Any
from pydantic import BaseModel, Field


class BookmakerStatus(str, Enum):
    ACTIVE = "active"
    MAINTENANCE = "maintenance"
    BLOCKED = "blocked"
    RATE_LIMITED = "rate_limited"


class Bookmaker(BaseModel):
    id: str
    name: str
    slug: str
    logo_url: Optional[str] = None
    
    url_live: str
    url_prematch: str
    
    status: BookmakerStatus = BookmakerStatus.ACTIVE
    priority: int = 1
    
    min_stake: float = 10.0
    max_stake: float = 100000.0
    
    config: Dict[str, Any] = Field(default_factory=dict)
    
    last_check: Optional[datetime] = None
    success_rate: float = 0.0
    
    class Config:
        use_enum_values = True


BOOKMAKERS = {
    "winline": Bookmaker(
        id="winline",
        name="Winline",
        slug="winline",
        url_live="https://winline.ru/live",
        url_prematch="https://winline.ru/prematch",
        priority=2
    ),
    "olimp": Bookmaker(
        id="olimp",
        name="Olimp",
        slug="olimp",
        url_live="https://www.olimp.bet/live",
        url_prematch="https://www.olimp.bet",
        priority=1
    ),
    "pari": Bookmaker(
        id="pari",
        name="Pari",
        slug="pari",
        url_live="https://www.pari.ru/live",
        url_prematch="https://www.pari.ru/prematch",
        priority=1
    ),
    "marathon": Bookmaker(
        id="marathon",
        name="Marathon",
        slug="marathon",
        url_live="https://www.marathonbet.ru/live",
        url_prematch="https://www.marathonbet.ru",
        priority=2
    ),
    "betboom": Bookmaker(
        id="betboom",
        name="BetBoom",
        slug="betboom",
        url_live="https://betboom.ru/live",
        url_prematch="https://betboom.ru/prematch",
        priority=1
    ),
    "fonbet": Bookmaker(
        id="fonbet",
        name="Fonbet",
        slug="fonbet",
        url_live="https://www.fonbet.ru/live",
        url_prematch="https://www.fonbet.ru/prematch",
        priority=2
    ),
    "1xbet": Bookmaker(
        id="1xbet",
        name="1xBet",
        slug="1xbet",
        url_live="https://1xbet.kz/live",
        url_prematch="https://1xbet.kz",
        priority=1
    ),
    "leon": Bookmaker(
        id="leon",
        name="Leon",
        slug="leon",
        url_live="https://leon.bet/live",
        url_prematch="https://leon.bet",
        priority=1
    ),
    "betcity": Bookmaker(
        id="betcity",
        name="Betcity",
        slug="betcity",
        url_live="https://betcity.ru/live",
        url_prematch="https://betcity.ru",
        priority=1
    ),
    "olimpbet": Bookmaker(
        id="olimpbet",
        name="Olimpbet",
        slug="olimpbet",
        url_live="https://olimp.bet/live",
        url_prematch="https://olimp.bet",
        priority=1
    ),
    "pinup": Bookmaker(
        id="pinup",
        name="Pin-up",
        slug="pinup",
        url_live="https://pinupgames.com/live",
        url_prematch="https://pinupgames.com",
        priority=1
    ),
    "zenit": Bookmaker(
        id="zenit",
        name="Zenit",
        slug="zenit",
        url_live="https://zenitbet.com/live",
        url_prematch="https://zenitbet.com",
        priority=2
    ),
}
