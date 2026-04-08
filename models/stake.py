# models/stake.py
from datetime import datetime
from enum import Enum
from typing import Optional
from pydantic import BaseModel, Field


class StakeStatus(str, Enum):
    PENDING = "pending"
    CONFIRMED = "confirmed"
    WON = "won"
    LOST = "lost"
    CANCELLED = "cancelled"
    ERROR = "error"


class Stake(BaseModel):
    id: str
    surebet_id: str
    
    bookmaker: str
    event_name: str
    selection: str
    odds: float
    
    stake_amount: float
    potential_win: float
    
    status: StakeStatus = StakeStatus.PENDING
    
    placed_at: Optional[datetime] = None
    resolved_at: Optional[datetime] = None
    result: Optional[str] = None
    
    notes: Optional[str] = None
