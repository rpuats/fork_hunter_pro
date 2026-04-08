# automation/auto_better.py
"""
Auto-Bet Executor - Automated bet placement
"""
import asyncio
import logging
from typing import Dict, List, Optional
from dataclasses import dataclass
from enum import Enum

logger = logging.getLogger(__name__)


class BetStatus(Enum):
    PENDING = "pending"
    CONFIRMED = "confirmed"
    REJECTED = "rejected"
    CANCELLED = "cancelled"
    ERROR = "error"


class BetMode(Enum):
    MANUAL = "manual"  # User confirms each bet
    SEMI_AUTO = "semi_auto"  # Auto if profit > X%
    FULL_AUTO = "full_auto"  # Everything automated


@dataclass
class BetRequest:
    """Bet placement request"""
    surebet_id: str
    bookmaker: str
    selection: str
    odds: float
    stake: float
    event_name: str
    mode: BetMode = BetMode.MANUAL
    max_retry: int = 3
    confirm_required: bool = True


@dataclass
class BetResult:
    """Result of bet placement"""
    request: BetRequest
    status: BetStatus
    message: str
    bet_id: Optional[str] = None
    actual_odds: Optional[float] = None
    actual_stake: Optional[float] = None
    timestamp: float = 0


class AutoBetter:
    """
    Auto-bet executor with Ghost Mode integration.
    Supports Playwright browser automation and direct API.
    """
    
    def __init__(self):
        self.mode = BetMode.MANUAL
        self.confirm_required = True
        self.min_profit_auto = 3.0  # Auto-bet if profit > 3%
        self.pending_bets: List[BetRequest] = []
        self.completed_bets: List[BetResult] = []
        self.browser = None
        self._pending_confirmations: Dict[str, BetRequest] = {}
    
    def set_mode(self, mode: BetMode):
        """Set bet mode"""
        self.mode = mode
        self.confirm_required = mode == BetMode.MANUAL
    
    async def place_bet(self, request: BetRequest) -> BetResult:
        """Place a bet"""
        logger.info(f"Placing bet: {request.bookmaker} - {request.selection} @ {request.odds}")
        
        # Check Ghost Mode heat
        from core.ghost_mode import ghost_mode
        if ghost_mode.should_slow_down(request.bookmaker):
            return BetResult(
                request=request,
                status=BetStatus.ERROR,
                message=f"Account {request.bookmaker} is HOT. Slow down required."
            )
        
        # Apply stake rounding (Ghost Mode)
        rounded_stake = ghost_mode.round_stake(request.stake, request.bookmaker)
        request.stake = rounded_stake
        
        # Add delay (Ghost Mode)
        delay = ghost_mode.get_random_delay(5.0)
        await asyncio.sleep(delay)
        
        # Check if confirmation required
        if request.confirm_required and request.mode == BetMode.MANUAL:
            self._pending_confirmations[request.surebet_id] = request
            return BetResult(
                request=request,
                status=BetStatus.PENDING,
                message="Awaiting confirmation"
            )
        
        # Try to place bet
        result = await self._execute_bet(request)
        
        # Record result in Ghost Mode
        if result.status == BetStatus.CONFIRMED:
            ghost_mode.record_bet_result(request.bookmaker, True, request.stake)
        elif result.status == BetStatus.REJECTED:
            ghost_mode.record_bet_result(request.bookmaker, False, 0)
        
        self.completed_bets.append(result)
        return result
    
    async def _execute_bet(self, request: BetRequest) -> BetResult:
        """Execute bet via browser or API"""
        try:
            # For now, simulate bet placement
            # In production, use Playwright or bookmaker API
            logger.info(f"Executing bet: {request.bookmaker}")
            
            await asyncio.sleep(0.5)  # Simulate network delay
            
            return BetResult(
                request=request,
                status=BetStatus.CONFIRMED,
                message="Bet confirmed",
                bet_id=f"BET_{request.surebet_id[:8]}",
                actual_odds=request.odds,
                actual_stake=request.stake
            )
            
        except Exception as e:
            return BetResult(
                request=request,
                status=BetStatus.ERROR,
                message=str(e)
            )
    
    async def place_surebet_pair(self, surebet: Dict) -> List[BetResult]:
        """Place bets for both sides of a surebet"""
        results = []
        
        legs = surebet.get('legs', [])
        
        # First bet - bookmaker with faster odds change
        legs_sorted = sorted(legs, key=lambda x: x.get('odds', 2.0), reverse=True)
        
        for i, leg in enumerate(legs_sorted):
            request = BetRequest(
                surebet_id=surebet.get('id', ''),
                bookmaker=leg.get('bookmaker', ''),
                selection=leg.get('selection', ''),
                odds=leg.get('odds', 0),
                stake=leg.get('calculated_stake', 1000),
                event_name=surebet.get('event_name', ''),
                mode=BetMode.MANUAL if self.confirm_required else BetMode.SEMI_AUTO,
                confirm_required=self.confirm_required
            )
            
            # Stagger bets to avoid detection
            if i > 0:
                await asyncio.sleep(3)  # 3 sec between bets
            
            result = await self.place_bet(request)
            results.append(result)
            
            if result.status == BetStatus.ERROR:
                break  # Stop if first bet failed
        
        return results
    
    def confirm_bet(self, surebet_id: str) -> bool:
        """Confirm a pending bet"""
        if surebet_id in self._pending_confirmations:
            request = self._pending_confirmations.pop(surebet_id)
            asyncio.create_task(self.place_bet(request))
            return True
        return False
    
    def cancel_bet(self, surebet_id: str) -> bool:
        """Cancel a pending bet"""
        if surebet_id in self._pending_confirmations:
            request = self._pending_confirmations.pop(surebet_id)
            self.completed_bets.append(BetResult(
                request=request,
                status=BetStatus.CANCELLED,
                message="Cancelled by user"
            ))
            return True
        return False
    
    def get_pending(self) -> List[BetRequest]:
        """Get pending bet confirmations"""
        return list(self._pending_confirmations.values())
    
    def get_stats(self) -> Dict:
        """Get betting statistics"""
        total = len(self.completed_bets)
        confirmed = sum(1 for r in self.completed_bets if r.status == BetStatus.CONFIRMED)
        rejected = sum(1 for r in self.completed_bets if r.status == BetStatus.REJECTED)
        
        return {
            'mode': self.mode.value,
            'total_bets': total,
            'confirmed': confirmed,
            'rejected': rejected,
            'pending': len(self._pending_confirmations),
            'success_rate': round(confirmed / total * 100, 1) if total > 0 else 0,
            'recent_results': [
                {'status': r.status.value, 'bookmaker': r.request.bookmaker}
                for r in self.completed_bets[-10:]
            ]
        }


# Global instance
auto_better = AutoBetter()
