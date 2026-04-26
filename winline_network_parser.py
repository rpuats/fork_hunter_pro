#!/usr/bin/env python3
"""
Winline парсер через перехват сетевых запросов
Вместо того чтобы парсить DOM, перехватываем реальные API ответы
"""

import asyncio
import json
import re
from datetime import datetime
from typing import List, Dict, Optional
import logging

try:
    from playwright.async_api import async_playwright, Route
    PLAYWRIGHT_AVAILABLE = True
except ImportError:
    PLAYWRIGHT_AVAILABLE = False
    print("⚠️ Playwright не установлен")

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class WinlineNetworkParser:
    """Парсер через перехват сетевых запросов"""
    
    BASE_URL = "https://winline.ru"
    
    STEALTH_HEADERS = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.8",
        "DNT": "1",
    }
    
    def __init__(self):
        self.events = []
        self.api_responses = []
        self.intercepted_urls = set()
        
    async def handle_route(self, route: Route):
        """Перехватываем каждый запрос"""
        url = route.request.url
        
        # Продолжаем загрузку
        await route.continue_()
        
    async def handle_response(self, response):
        """Обрабатываем каждый ответ"""
        url = response.url
        
        # Ищем API эндпоинты с событиями
        if any(api in url for api in [
            '/api/',
            'events',
            'matches',
            'sports',
            'competitions',
            'fixtures',
            'getEvents',
            'getMatches',
        ]):
            self.intercepted_urls.add(url)
            try:
                body = await response.text()
                if len(body) > 100:  # Игнорируем пустые ответы
                    logger.info(f"📡 Intercepted: {url[:80]}...")
                    logger.info(f"   Response size: {len(body)} bytes")
                    
                    # Пытаемся распарсить как JSON
                    try:
                        data = json.loads(body)
                        self.api_responses.append({
                            'url': url,
                            'data': data,
                            'size': len(body)
                        })
                        
                        # Ищем события в ответе
                        self._extract_from_api_response(data, url)
                    except json.JSONDecodeError:
                        pass
            except Exception as e:
                pass
    
    def _extract_from_api_response(self, data, url: str):
        """Извлекаем события из API ответа"""
        
        # Рекурсивно ищем структуры которые похожи на события
        def find_events(obj, depth=0):
            if depth > 10:  # Не копаем слишком глубоко
                return
            
            if isinstance(obj, dict):
                # Проверяем ключи которые указывают на события
                for key in obj.keys():
                    if any(word in key.lower() for word in ['event', 'match', 'game', 'sport', 'competition', 'fixture']):
                        value = obj[key]
                        if isinstance(value, list):
                            for item in value:
                                if isinstance(item, dict):
                                    event = self._try_parse_event(item)
                                    if event and event not in self.events:
                                        self.events.append(event)
                                        logger.info(f"   ✅ Found event: {event.get('home', 'N/A')} vs {event.get('away', 'N/A')}")
                        else:
                            find_events(value, depth + 1)
                    
                # Также рекурсивно ищем во всех значениях
                for value in obj.values():
                    if isinstance(value, (dict, list)):
                        find_events(value, depth + 1)
                        
            elif isinstance(obj, list):
                for item in obj:
                    find_events(item, depth + 1)
        
        find_events(data)
    
    def _try_parse_event(self, obj: dict) -> Optional[dict]:
        """Пытаемся распарсить объект как событие"""
        if not isinstance(obj, dict):
            return None
        
        # Проверяем что есть основные поля события
        has_teams = any(field in obj for field in ['home', 'away', 'home_team', 'away_team', 'team1', 'team2'])
        has_sport = any(field in obj for field in ['sport', 'league', 'competition', 'category'])
        has_id = any(field in obj for field in ['id', 'event_id', 'match_id', 'pk'])
        
        if not (has_teams or has_id):
            return None
        
        # Пытаемся извлечь данные
        event = {}
        
        # ID
        for field in ['id', 'event_id', 'match_id', 'pk']:
            if field in obj:
                event['id'] = str(obj[field])
                break
        
        # Команды
        for field in ['home', 'home_team', 'team1']:
            if field in obj:
                event['home'] = str(obj[field])
                break
        
        for field in ['away', 'away_team', 'team2']:
            if field in obj:
                event['away'] = str(obj[field])
                break
        
        # Спорт/лига
        for field in ['sport', 'league', 'competition', 'category']:
            if field in obj:
                event['league'] = str(obj[field])
                break
        
        # Live статус
        for field in ['is_live', 'live', 'status']:
            if field in obj:
                event['is_live'] = obj[field] in [True, 'live', 1]
                break
        
        # Если есть хотя бы ID и команды - это событие
        if 'id' in event and ('home' in event or 'away' in event or 'league' in event):
            return event
        
        return None
    
    async def parse(self) -> List[dict]:
        """Запускаем парсер"""
        if not PLAYWRIGHT_AVAILABLE:
            logger.error("❌ Playwright not available")
            return []
        
        logger.info("🚀 Starting Winline network sniffer parser")
        logger.info(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print()
        
        try:
            async with async_playwright() as p:
                # Запускаем браузер с минимальной защитой
                browser = await p.chromium.launch(
                    headless=True,
                    args=[
                        "--disable-blink-features=AutomationControlled",
                        "--disable-dev-shm-usage",
                        "--no-sandbox",
                        "--disable-gpu",
                    ]
                )
                
                context = await browser.new_context(
                    user_agent=self.STEALTH_HEADERS["User-Agent"],
                    locale="ru-RU",
                    timezone_id="Europe/Moscow",
                )
                
                # Добавляем stealth скрипт
                stealth_js = """
                Object.defineProperty(navigator, 'webdriver', {
                    get: () => undefined,
                });
                """
                await context.add_init_script(stealth_js)
                
                page = await context.new_page()
                
                # Перехватываем ответы
                page.on("response", self.handle_response)
                
                logger.info("📡 Loading Winline main page...")
                try:
                    await page.goto(f"{self.BASE_URL}/", timeout=60000, wait_until="domcontentloaded")
                except:
                    logger.warning("Main page timeout, continuing...")
                
                # Даем время на загрузку событий
                logger.info("⏳ Waiting for API responses...")
                await asyncio.sleep(5)
                
                # Загружаем live страницу
                logger.info("📡 Loading live page...")
                try:
                    await page.goto(f"{self.BASE_URL}/live", timeout=60000, wait_until="domcontentloaded")
                except:
                    pass
                await asyncio.sleep(3)
                
                # Загружаем футбол
                logger.info("📡 Loading football page...")
                try:
                    await page.goto(f"{self.BASE_URL}/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
                except:
                    pass
                await asyncio.sleep(3)
                
                # Закрываем браузер
                await browser.close()
        
        except Exception as e:
            logger.error(f"❌ Error: {e}")
        
        logger.info('')
        logger.info(f"📊 RESULTS:")
        logger.info(f"   🔍 Intercepted URLs: {len(self.intercepted_urls)}")
        logger.info(f"   📡 API responses: {len(self.api_responses)}")
        logger.info(f"   ✅ Events found: {len(self.events)}")
        
        # Сохраняем результаты
        result = {
            "bookmaker": "Winline",
            "events": self.events,
            "count": len(self.events),
            "generated_at": datetime.now().isoformat(),
            "intercepted_urls": list(self.intercepted_urls),
            "api_responses_count": len(self.api_responses),
        }
        
        with open("winline_events.json", "w", encoding="utf-8") as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
        
        logger.info(f"💾 Saved to winline_events.json")
        logger.info(f"Finished at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        
        return self.events


async def main():
    parser = WinlineNetworkParser()
    events = await parser.parse()
    
    if events:
        print()
        print(f"✅ SUCCESS: Found {len(events)} events")
        print()
        print("Sample events:")
        for i, event in enumerate(events[:5]):
            home = event.get('home', 'N/A')
            away = event.get('away', 'N/A')
            league = event.get('league', 'N/A')
            print(f"  {i+1}. {home} vs {away} ({league})")
    else:
        print()
        print("❌ FAILED: No events found")
        print()
        print("Debug info:")
        print(f"  Intercepted {len(parser.intercepted_urls)} unique URLs")
        print(f"  Parsed {len(parser.api_responses)} API responses")


if __name__ == "__main__":
    asyncio.run(main())

