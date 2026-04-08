# scanner/parsers/mock_parser.py
"""
Mock parser for testing the system without real bookmaker APIs
Generates realistic events with slight odds variations to create surebets
"""
import random
import time
from typing import List, Dict
from scanner.parsers.base import BaseParser


TEAMS = [
    ('Реал Мадрид', 'Барселона'),
    ('Манчестер Сити', 'Ливерпуль'),
    ('Бавария', 'Боруссия Дортмунд'),
    ('ПСЖ', 'Марсель'),
    ('Ювентус', 'Интер'),
    ('Спартак', 'ЦСКА'),
    ('Зенит', 'Локомотив'),
    ('Арсенал', 'Челси'),
    ('Атлетико', 'Севилья'),
    ('Наполи', 'Милан'),
    ('Тоттенхэм', 'Манчестер Юнайтед'),
    ('Рома', 'Лацио'),
    ('Динамо Москва', 'Краснодар'),
    ('Ростов', 'Урал'),
    ('Бетис', 'Вильярреал'),
    ('ЦСКА', 'СКА'),
    ('Динамо Москва', 'Спартак'),
    ('Ак Барс', 'Металлург'),
    ('Авангард', 'Салават Юлаев'),
    ('ЦСКА', 'Реал'),
]

SPORTS = ['football', 'hockey', 'basketball']


class MockParser(BaseParser):
    """Mock parser that generates realistic events for testing"""
    name = "Mock"
    slug = "mock"
    base_url = "https://mock.ghost-imperium.local"
    
    async def get_events(self) -> List[Dict]:
        """Generate mock events with realistic odds variations"""
        events = []
        count = random.randint(30, 50)
        
        for i in range(count):
            home, away = random.choice(TEAMS)
            sport = random.choice(SPORTS)
            
            base_odds = random.uniform(1.5, 3.5)
            home_odds = round(base_odds + random.uniform(-0.3, 0.3), 2)
            away_odds = round(base_odds + random.uniform(-0.3, 0.3), 2)
            draw_odds = round(random.uniform(2.8, 4.2), 2) if sport == 'football' else None
            
            events.append({
                'id': f"mock_{self.slug}_{i}_{int(time.time())}",
                'bookmaker': self.slug,
                'sport': sport,
                'home_team': home,
                'away_team': away,
                'league': f"Test League {random.randint(1, 5)}",
                'home_odds': max(1.01, home_odds),
                'draw_odds': draw_odds if draw_odds and draw_odds > 1.0 else None,
                'away_odds': max(1.01, away_odds),
                'is_live': random.choice([True, True, True, False]),
                'market': '1x2',
                'source_url': f"{self.base_url}/live",
                'scraped_at': time.time()
            })
        
        return events


class MockWinlineParser(MockParser):
    name = "Mock Winline"
    slug = "winline"
    base_url = "https://winline.ru"


class MockFonbetParser(MockParser):
    name = "Mock Fonbet"
    slug = "fonbet"
    base_url = "https://fonbet.ru"


class MockPariParser(MockParser):
    name = "Mock Pari"
    slug = "pari"
    base_url = "https://pari.ru"


class MockOlimpParser(MockParser):
    name = "Mock Olimp"
    slug = "olimp"
    base_url = "https://olimp.bet"


class MockBetBoomParser(MockParser):
    name = "Mock BetBoom"
    slug = "betboom"
    base_url = "https://betboom.ru"


class Mock1xStavkaParser(MockParser):
    name = "Mock 1xStavka"
    slug = "1xstavka"
    base_url = "https://1xstavka.ru"


class MockLeonParser(MockParser):
    name = "Mock Leon"
    slug = "leon"
    base_url = "https://leon.ru"


class MockMarathonParser(MockParser):
    name = "Mock Marathon"
    slug = "marathon"
    base_url = "https://marathonbet.com"


class MockBetcityParser(MockParser):
    name = "Mock Betcity"
    slug = "betcity"
    base_url = "https://betcity.ru"


class MockPinupParser(MockParser):
    name = "Mock Pin-up"
    slug = "pinup"
    base_url = "https://pin-up.ru"


class MockZenitParser(MockParser):
    name = "Mock Zenit"
    slug = "zenit"
    base_url = "https://zenit.bet"


class MockOlimpbetParser(MockParser):
    name = "Mock Olimpbet"
    slug = "olimpbet"
    base_url = "https://olimpbet.kz"


MOCK_PARSERS = [
    MockWinlineParser,
    MockFonbetParser,
    MockPariParser,
    MockOlimpParser,
    MockBetBoomParser,
    Mock1xStavkaParser,
    MockLeonParser,
    MockMarathonParser,
    MockBetcityParser,
    MockPinupParser,
    MockZenitParser,
    MockOlimpbetParser,
]
