#!/usr/bin/env python3
"""
Winline API Reverse Engineering

Анализирует и тестирует реальные API endpoints для вытягивания событий.
"""

import asyncio
import aiohttp
import json
import logging
import sys
from typing import Optional, Dict, List

logging.basicConfig(
    level=logging.INFO,
    format='[%(asctime)s] %(levelname)s: %(message)s'
)
logger = logging.getLogger(__name__)


class WinlineAPIAnalyzer:
    """Reverse engineers Winline API"""
    
    def __init__(self):
        self.base_url = 'https://winline.ru'
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            'Accept': 'application/json, text/plain, */*',
            'Referer': 'https://winline.ru/stavki/sport/futbol',
            'X-Requested-With': 'XMLHttpRequest',
        }
        self.results = {
            'api_endpoints_found': [],
            'event_data_extracted': [],
            'api_structure': {},
        }
    
    async def test_api_endpoints(self, session: aiohttp.ClientSession):
        """Тестирует найденные API endpoints"""
        logger.info("\n[1] Testing discovered API endpoints...\n")
        
        # Endpoints найденные из сетевого анализа
        endpoints_to_test = [
            # XDS API (Events)
            '/api/xds/v2/event/{event_id}/1',  # Даны с живого анализа
            '/api/xds/v2/sport/205/1',  # Футбол
            '/api/xds/v2/sports',  # Все спорты
            
            # CLS API (Classification)
            '/api/cls/menu/sport/205/country-xy/8-22',  # Menu
            '/api/cls/event/1/{event_id}',  # Event
            '/api/cls/sports',  # Sports list
            
            # Alternative endpoints
            '/api/v2/events',
            '/api/v2/sports',
            '/api/v2/menu',
            '/api/v1/sports/205/events',
            '/api/v1/sports/205/matches',
            
            # WebSocket API
            '/api/v2/websocket',
        ]
        
        # Конкретные event IDs от анализа
        test_event_ids = [
            15613139, 15613204, 15613123, 15611162, 15611165,
        ]
        
        results = []
        
        for endpoint_template in endpoints_to_test:
            # Если в шаблоне есть {event_id}, тестируем с реальными ID-ами
            if '{event_id}' in endpoint_template:
                for event_id in test_event_ids[:2]:  # Первые 2 для проверки
                    endpoint = endpoint_template.format(event_id=event_id)
                    result = await self._test_endpoint(session, endpoint)
                    if result['status'] == 200:
                        logger.info(f"✓ WORKING: {endpoint}")
                        results.append(result)
            else:
                result = await self._test_endpoint(session, endpoint_template)
                if result['status'] == 200:
                    logger.info(f"✓ WORKING: {endpoint_template}")
                    results.append(result)
        
        return results
    
    async def _test_endpoint(self, session: aiohttp.ClientSession, endpoint: str) -> Dict:
        """Тестирует один endpoint"""
        url = self.base_url + endpoint
        
        try:
            async with session.get(url, timeout=10, headers=self.headers) as resp:
                if resp.status == 200:
                    try:
                        data = await resp.json()
                        return {
                            'endpoint': endpoint,
                            'url': url,
                            'status': 200,
                            'content_type': resp.content_type,
                            'data_sample': str(data)[:200],
                            'has_events': 'event' in json.dumps(data).lower() or 'match' in json.dumps(data).lower(),
                        }
                    except:
                        return {
                            'endpoint': endpoint,
                            'status': 200,
                            'content_type': resp.content_type,
                            'error': 'Not JSON',
                        }
                else:
                    return {
                        'endpoint': endpoint,
                        'status': resp.status,
                        'error': f'HTTP {resp.status}',
                    }
        except asyncio.TimeoutError:
            return {
                'endpoint': endpoint,
                'status': 0,
                'error': 'Timeout',
            }
        except Exception as e:
            return {
                'endpoint': endpoint,
                'status': 0,
                'error': str(e)[:50],
            }
    
    async def analyze_api_structure(self, session: aiohttp.ClientSession):
        """Анализирует структуру API"""
        logger.info("\n[2] Analyzing API structure...\n")
        
        # 1. Получаем список спортов
        logger.info("Fetching sports list...")
        sports_endpoints = [
            '/api/cls/sports',
            '/api/xds/v2/sports',
            '/api/v2/sports',
        ]
        
        sports_data = None
        for endpoint in sports_endpoints:
            try:
                url = self.base_url + endpoint
                async with session.get(url, timeout=10, headers=self.headers) as resp:
                    if resp.status == 200:
                        sports_data = await resp.json()
                        logger.info(f"✓ Got sports from {endpoint}")
                        logger.info(f"  Sample: {str(sports_data)[:100]}")
                        break
            except:
                pass
        
        # 2. Получаем меню футбола (sport_id = 205)
        logger.info("\nFetching football menu...")
        menu_endpoints = [
            '/api/cls/menu/sport/205/country-xy/8-22',
            '/api/xds/v2/menu/205',
            '/api/v2/menu/sport/205',
        ]
        
        menu_data = None
        for endpoint in menu_endpoints:
            try:
                url = self.base_url + endpoint
                async with session.get(url, timeout=10, headers=self.headers) as resp:
                    if resp.status == 200:
                        menu_data = await resp.json()
                        logger.info(f"✓ Got menu from {endpoint}")
                        
                        # Ищем события в ответе
                        menu_str = json.dumps(menu_data)
                        event_count = menu_str.count('"id"')
                        logger.info(f"  Contains ~{event_count} items")
                        logger.info(f"  Sample: {menu_str[:150]}")
                        break
            except Exception as e:
                logger.debug(f"  Failed: {e}")
        
        return {
            'sports': sports_data,
            'menu': menu_data,
        }
    
    async def extract_real_events(self, session: aiohttp.ClientSession):
        """Вытягивает реальные события"""
        logger.info("\n[3] Extracting real events...\n")
        
        events = []
        
        # Пробуем получить события из меню
        menu_endpoint = '/api/cls/menu/sport/205/country-xy/8-22'
        try:
            url = self.base_url + menu_endpoint
            async with session.get(url, timeout=10, headers=self.headers) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    
                    # Парсим структуру ответа
                    logger.info(f"Response keys: {list(data.keys())[:5] if isinstance(data, dict) else 'Not a dict'}")
                    
                    # Ищем события
                    if isinstance(data, dict):
                        # Рекурсивный поиск событий
                        events = self._extract_events_recursive(data)
                        logger.info(f"Found {len(events)} events in menu")
                    elif isinstance(data, list):
                        events = data
                        logger.info(f"Got list of {len(events)} items")
        except Exception as e:
            logger.error(f"Failed to extract from menu: {e}")
        
        # Выводим примеры
        if events:
            logger.info("\nFirst 3 events:")
            for event in events[:3]:
                logger.info(f"  {json.dumps(event, ensure_ascii=False)[:100]}")
        
        return events
    
    def _extract_events_recursive(self, obj, depth=0):
        """Рекурсивно ищет события"""
        events = []
        
        if depth > 10:  # Защита от глубокой рекурсии
            return events
        
        if isinstance(obj, dict):
            # Проверяем если это событие
            if 'id' in obj and ('name' in obj or 'title' in obj or 'league' in obj):
                events.append(obj)
            
            # Ищем дальше
            for value in obj.values():
                events.extend(self._extract_events_recursive(value, depth + 1))
        
        elif isinstance(obj, list):
            for item in obj:
                events.extend(self._extract_events_recursive(item, depth + 1))
        
        return events


async def main():
    logger.info("=" * 60)
    logger.info("WINLINE API REVERSE ENGINEERING")
    logger.info("=" * 60)
    
    analyzer = WinlineAPIAnalyzer()
    
    async with aiohttp.ClientSession() as session:
        # 1. Тестируем endpoints
        working_endpoints = await analyzer.test_api_endpoints(session)
        analyzer.results['api_endpoints_found'] = working_endpoints
        
        # 2. Анализируем структуру
        api_structure = await analyzer.analyze_api_structure(session)
        analyzer.results['api_structure'] = api_structure
        
        # 3. Вытягиваем события
        events = await analyzer.extract_real_events(session)
        analyzer.results['event_data_extracted'] = events
    
    # Сохраняем результаты
    with open('winline_api_reverse_engineering.json', 'w', encoding='utf-8') as f:
        json.dump(analyzer.results, f, indent=2, ensure_ascii=False, default=str)
    
    logger.info("\n" + "=" * 60)
    logger.info("ANALYSIS COMPLETE")
    logger.info(f"Results: {len(working_endpoints)} endpoints, {len(events)} events")
    logger.info(f"Saved to: winline_api_reverse_engineering.json")
    logger.info("=" * 60)


if __name__ == '__main__':
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())
    
    asyncio.run(main())
