# models/event.py
from datetime import datetime
from enum import Enum
from typing import Optional
from pydantic import BaseModel, Field


class EventStatus(str, Enum):
    UPCOMING = "upcoming"
    LIVE = "live"
    FINISHED = "finished"


class SportType(str, Enum):
    FOOTBALL = "football"
    HOCKEY = "hockey"
    BASKETBALL = "basketball"
    TENNIS = "tennis"
    VOLLEYBALL = "volleyball"
    OTHER = "other"


class MarketType(str, Enum):
    MONEYLINE = "1x2"
    HANDICAP = "handicap"
    TOTAL = "total"
    BOTH_SCORE = "both_score"
    DOUBLE_CHANCE = "double_chance"


class Event(BaseModel):
    id: str
    bookmaker: str
    sport: SportType
    league: str
    home_team: str
    away_team: str
    start_time: datetime
    status: EventStatus = EventStatus.UPCOMING
    
    market: MarketType
    home_odds: float
    draw_odds: Optional[float] = None
    away_odds: float
    
    home_line: Optional[float] = None
    away_line: Optional[float] = None
    total_line: Optional[float] = None
    
    source_url: str
    created_at: datetime = Field(default_factory=datetime.utcnow)
    
    @property
    def name(self) -> str:
        return f"{self.home_team} vs {self.away_team}"
    
    @property
    def is_live(self) -> bool:
        return self.status == EventStatus.LIVE
