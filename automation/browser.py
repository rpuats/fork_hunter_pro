# automation/browser.py
import asyncio
from typing import Dict, Optional, List
from dataclasses import dataclass
import logging

logger = logging.getLogger(__name__)


@dataclass
class BookmakerConfig:
    name: str
    url: str
    login_selectors: Dict[str, str]
    event_search_selector: str
    odds_selectors: Dict[str, str]
    stake_input_selector: str
    confirm_button_selector: str


class BrowserController:
    """
    Browser automation for automatic betting
    Uses Playwright for browser control
    """
    
    BROOKMAKER_CONFIGS = {
        'winline': BookmakerConfig(
            name='Winline',
            url='https://winline.ru/live',
            login_selectors={'username': '#login', 'password': '#password', 'submit': '.btn-login'},
            event_search_selector='.search-input',
            odds_selectors={'1': '[data-market="1"]', '2': '[data-market="2"]', 'X': '[data-market="X"]'},
            stake_input_selector='.stake-input',
            confirm_button_selector='.btn-confirm'
        ),
        'olimp': BookmakerConfig(
            name='Olimp',
            url='https://www.olimp.bet/live',
            login_selectors={'username': 'input[name="login"]', 'password': 'input[name="password"]', 'submit': '.btn-enter'},
            event_search_selector='.search-line',
            odds_selectors={'1': '.k1', '2': '.k2', 'X': '.kx'},
            stake_input_selector='input.stake',
            confirm_button_selector='button[type="submit"]'
        ),
        'pari': BookmakerConfig(
            name='Pari',
            url='https://www.pari.ru/live',
            login_selectors={'username': '#UserLogin_username', 'password': '#UserLogin_password', 'submit': '.btn-success'},
            event_search_selector='.live-search input',
            odds_selectors={'1': '.outcome-item:first-child', '2': '.outcome-item:last-child'},
            stake_input_selector='.bet-amount input',
            confirm_button_selector='.bet-submit'
        ),
    }
    
    def __init__(self, headless: bool = True):
        self.headless = headless
        self.browser = None
        self.context = None
        self.playwright = None
        self.is_initialized = False
        self.current_page = None
    
    async def init(self):
        try:
            from playwright.async_api import async_playwright
            
            self.playwright = await async_playwright().start()
            self.browser = await self.playwright.chromium.launch(
                headless=self.headless,
                args=[
                    '--disable-blink-features=AutomationControlled',
                    '--disable-infobars',
                    '--no-sandbox',
                    '--disable-setuid-sandbox'
                ]
            )
            self.context = await self.browser.new_context(
                viewport={'width': 1920, 'height': 1080},
                user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                locale='ru-RU'
            )
            self.is_initialized = True
            logger.info("Browser controller initialized")
            
        except ImportError:
            logger.error("Playwright not installed. Run: pip install playwright && playwright install chromium")
            raise
        except Exception as e:
            logger.error(f"Browser init error: {e}")
            raise
    
    async def close(self):
        if self.browser:
            await self.browser.close()
        if self.playwright:
            await self.playwright.stop()
        self.is_initialized = False
        logger.info("Browser controller closed")
    
    async def login(self, bookmaker: str, username: str, password: str) -> bool:
        if not self.is_initialized:
            await self.init()
        
        config = self.BROOKMAKER_CONFIGS.get(bookmaker)
        if not config:
            logger.error(f"Unknown bookmaker: {bookmaker}")
            return False
        
        try:
            page = await self.context.new_page()
            await page.goto(config.url, wait_until='networkidle', timeout=30000)
            
            await page.fill(config.login_selectors['username'], username)
            await page.fill(config.login_selectors['password'], password)
            await page.click(config.login_selectors['submit'])
            
            await page.wait_for_load_state('networkidle', timeout=10000)
            
            self.current_page = page
            logger.info(f"Logged in to {bookmaker}")
            return True
            
        except Exception as e:
            logger.error(f"Login error: {e}")
            return False
    
    async def search_event(self, event_name: str) -> bool:
        if not self.current_page:
            logger.error("No active page")
            return False
        
        try:
            search_input = await self.current_page.query_selector('.search-input, .search-line, input[type="search"]')
            if search_input:
                await search_input.fill(event_name)
                await asyncio.sleep(1)
                return True
            return False
        except Exception as e:
            logger.error(f"Search error: {e}")
            return False
    
    async def place_bet(
        self,
        bookmaker: str,
        market: str,
        odds: float,
        stake: float,
        selection: str
    ) -> Dict:
        if not self.is_initialized:
            await self.init()
        
        config = self.BROOKMAKER_CONFIGS.get(bookmaker)
        if not config:
            return {'success': False, 'error': 'Unknown bookmaker'}
        
        try:
            if not self.current_page:
                self.current_page = await self.context.new_page()
                await self.current_page.goto(config.url, wait_until='networkidle', timeout=30000)
            
            await self.current_page.goto(config.url, wait_until='networkidle', timeout=30000)
            
            odds_selector = config.odds_selectors.get(market, config.odds_selectors.get('1'))
            odds_element = await self.current_page.query_selector(odds_selector)
            
            if odds_element:
                await odds_element.click()
                await asyncio.sleep(0.5)
            
            stake_input = await self.current_page.query_selector(config.stake_input_selector)
            if stake_input:
                await stake_input.fill(str(stake))
                await asyncio.sleep(0.3)
            
            confirm_btn = await self.current_page.query_selector(config.confirm_button_selector)
            if confirm_btn:
                await confirm_btn.click()
                await asyncio.sleep(1)
                
                return {
                    'success': True,
                    'bookmaker': bookmaker,
                    'market': market,
                    'odds': odds,
                    'stake': stake,
                    'selection': selection
                }
            
            return {'success': False, 'error': 'Could not place bet'}
            
        except Exception as e:
            logger.error(f"Bet placement error: {e}")
            return {'success': False, 'error': str(e)}
    
    async def place_surebet(self, surebet: Dict, total_stake: float = 10000) -> List[Dict]:
        results = []
        
        for leg in surebet.get('legs', []):
            stake = leg.get('calculated_stake', total_stake / len(surebet.get('legs', [1])))
            
            result = await self.place_bet(
                bookmaker=leg['bookmaker'],
                market=leg['market'],
                odds=leg['odds'],
                stake=stake,
                selection=leg['selection']
            )
            
            results.append(result)
            await asyncio.sleep(2)
        
        return results
    
    async def screenshot(self, path: str = 'screenshot.png'):
        if self.current_page:
            await self.current_page.screenshot(path=path)
    
    async def execute_script(self, script: str):
        if self.current_page:
            return await self.current_page.evaluate(script)
