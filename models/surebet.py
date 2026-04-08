# models/surebet.py
from datetime import datetime
from typing import List, Optional
from pydantic import BaseModel, Field


class SurebetLeg(BaseModel):
    bookmaker: str
    event_id: str
    event_name: str
    market: str
    selection: str
    odds: float
    stake_percent: float
    calculated_stake: Optional[float] = None


class Surebet(BaseModel):
    id: str
    event_name: str
    sport: str
    is_live: bool
    
    legs: List[SurebetLeg]
    
    profit_percent: float
    total_stake: float
    estimated_profit: float
    
    bookmakers: List[str]
    market_type: str
    
    found_at: datetime = Field(default_factory=datetime.utcnow)
    expires_at: Optional[datetime] = None
    
    @property
    def num_legs(self) -> int:
        return len(self.legs)
    
    @property
    def is_three_way(self) -> bool:
        return self.num_legs >= 3
