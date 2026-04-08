# scanner/auto_better.py
import asyncio
from typing import Dict, Optional, List
from dataclasses import dataclass
from datetime import datetime
import logging

logger = logging.getLogger(__name__)


@dataclass
class BookmakerCredentials:
    username: str
    password: str
    two_factor: Optional[str] = None


@dataclass
class BetRequest:
    bookmaker: str
    event_name: str
    market: str
    odds: float
    stake: float
    selection: str


@dataclass
class BetResult:
    success: bool
    bookmaker: str
    bet_id: Optional[str]
    error: Optional[str]
    timestamp: datetime


class AutoBetter:
    def __init__(self, browser_controller=None):
        self.browser = browser_controller
        self.credentials: Dict[str, BookmakerCredentials] = {}
        self.is_running = False
        
    def add_credentials(self, bookmaker: str, username: str, password: str, two_factor: str = None):
        self.credentials[bookmaker] = BookmakerCredentials(
            username=username,
            password=password,
            two_factor=two_factor
        )
    
    async def place_bet(self, request: BetRequest) -> BetResult:
        if request.bookmaker not in self.credentials:
            return BetResult(
                success=False,
                bookmaker=request.bookmaker,
                bet_id=None,
                error="No credentials configured",
                timestamp=datetime.utcnow()
            )
        
        try:
            if self.browser:
                return await self._place_bet_browser(request)
            else:
                return await self._place_bet_api(request)
                
        except Exception as e:
            logger.error(f"Bet placement error: {e}")
            return BetResult(
                success=False,
                bookmaker=request.bookmaker,
                bet_id=None,
                error=str(e),
                timestamp=datetime.utcnow()
            )
    
    async def _place_bet_browser(self, request: BetRequest) -> BetResult:
        creds = self.credentials[request.bookmaker]
        
        logger.info(f"Opening {request.bookmaker}...")
        await self.browser.open_page(self._get_bookmaker_url(request.bookmaker))
        
        if not await self.browser.is_logged_in():
            logger.info(f"Logging in to {request.bookmaker}...")
            await self.browser.login(creds.username, creds.password)
            
            if creds.two_factor:
                await self.browser.enter_2fa(creds.two_factor)
        
        await self.browser.wait_for_load()
        
        logger.info(f"Searching for: {request.event_name}")
        await self.browser.search_event(request.event_name)
        
        logger.info(f"Clicking odds: {request.odds}")
        await self.browser.click_odds(request.market, request.selection)
        
        logger.info(f"Entering stake: {request.stake}")
        await self.browser.enter_stake(request.stake)
        
        logger.info("Confirming bet...")
        result = await self.browser.confirm_bet()
        
        return BetResult(
            success=result,
            bookmaker=request.bookmaker,
            bet_id=str(hash(datetime.utcnow().isoformat())) if result else None,
            error=None if result else "Bet not confirmed",
            timestamp=datetime.utcnow()
        )
    
    async def _place_bet_api(self, request: BetRequest) -> BetResult:
        logger.info(f"API bet to {request.bookmaker}: {request.event_name}")
        
        return BetResult(
            success=False,
            bookmaker=request.bookmaker,
            bet_id=None,
            error="API not implemented - use browser mode",
            timestamp=datetime.utcnow()
        )
    
    def _get_bookmaker_url(self, bookmaker: str) -> str:
        urls = {
            "winline": "https://winline.ru/live",
            "olimp": "https://www.olimp.bet/live",
            "pari": "https://www.pari.ru/live",
            "fonbet": "https://www.fonbet.ru/live",
            "marathon": "https://www.marathonbet.ru/live",
            "betboom": "https://betboom.ru/live",
            "1xbet": "https://1xbet.kz/live",
            "leon": "https://leon.bet/live",
        }
        return urls.get(bookmaker, "")
    
    async def place_surebet(self, surebet: Dict, total_stake: float = 10000) -> List[BetResult]:
        results = []
        
        for leg in surebet.get('legs', []):
            request = BetRequest(
                bookmaker=leg['bookmaker'],
                event_name=leg['event_name'],
                market=leg['market'],
                odds=leg['odds'],
                stake=leg.get('calculated_stake', total_stake / len(surebet.get('legs', [1]))),
                selection=leg['selection']
            )
            
            result = await self.place_bet(request)
            results.append(result)
            
            if not result.success:
                logger.warning(f"Bet failed: {result.error}")
        
        return results
