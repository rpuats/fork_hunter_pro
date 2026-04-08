# core/orchestrator.py
import asyncio
import logging
from typing import List, Dict
from datetime import datetime

from config import CONFIG
from core.event_normalizer import normalize_event_name, are_same_event
from core.surebet_calculator import SurebetCalculator
from freebet.freebet_handler import FreebetHandler
# from real_events_filter import filter_real_events  # ВРЕМЕННО ОТКЛЮЧАЕМ
from scrapers.winline_scraper import WinlineScraper
from scrapers.olimp_scraper import OlimpScraper
from scrapers.pari_scraper import PariScraper
from scrapers.marathon_scraper import MarathonScraper
from scrapers.betboom_scraper import BetBoomScraper

logger = logging.getLogger(__name__)

class ScraperOrchestrator:
    def __init__(self):
        self.scrapers = [
            WinlineScraper(),
            OlimpScraper(),
            PariScraper(),
            MarathonScraper(),
            BetBoomScraper()
        ]
        self.surebet_calculator = SurebetCalculator(min_profit=0.5)  # сильно снизили, чтобы увидеть вилки
        self.freebet_handler = FreebetHandler()

    async def get_all_events(self) -> List[Dict]:
        all_events = []
        
        for scraper in self.scrapers:
            try:
                events = await scraper.get_events()
                logger.info(f"[{scraper.name}] Получено {len(events)} событий")
                
                # ВРЕМЕННО БЕЗ ФИЛЬТРА ДЛЯ ТЕСТА ВИЛОК
                # real_events = filter_real_events(events)
                # logger.info(f"[{scraper.name}] Отфильтровано {len(real_events)} реальных событий")
                
                all_events.extend(events)
            except Exception as e:
                logger.error(f"[{scraper.name}] Ошибка: {e}")
        
        print(f"Собрано событий всего: {len(all_events)}")
        return all_events

    def calculate_surebets(self, events: List[Dict]) -> List[Dict]:
        return self.surebet_calculator.find_surebets(events)

    def print_surebets(self, surebets: List[Dict]):
        if not surebets:
            print("Вилок не найдено в этом цикле.")
            return

        print(f"\n{'='*100}")
        print(f"НАЙДЕНО ВИЛОК: {len(surebets)}")
        print(f"{'='*100}\n")

        for i, sb in enumerate(surebets, 1):
            print(f"{i:2d}. {sb.get('event_name', 'Unknown')[:90]}")
            print(f"    Рынок: {sb['market_type']} | Прибыль: +{sb['profit_percent']:.2f}%")
            print(f"    {sb['bookmaker1']} @{sb['odd1']}  vs  {sb['bookmaker2']} @{sb['odd2']}")
            print(f"    Ставки: {sb['stake1_percent']}% / {sb['stake2_percent']}%")
            print("-" * 90)

    def check_freebet_opportunities(self, surebets: List[Dict]):
        if not surebets:
            return
        print("\n🎟️ Анализ фрибетов...")
        recommendations = self.freebet_handler.get_recommendations(surebets)
        for rec in recommendations:
            print(f"   ✅ {rec['message']}")

    async def close(self):
        for scraper in self.scrapers:
            if hasattr(scraper, 'close'):
                await scraper.close()
