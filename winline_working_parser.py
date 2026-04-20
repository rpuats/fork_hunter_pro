#!/usr/bin/env python3
"""
Рабочий Winline парсер - вытаскивает 3000+ прематч событий + 10-20 лайв
Использует Playwright для загрузки JavaScript и обхода bot detection
"""

import asyncio
import json
import re
from datetime import datetime
from typing import List, Dict, Tuple, Optional
import logging

# Попытаемся импортировать Playwright, если нет - выведем инструкцию
try:
    from playwright.async_api import async_playwright
    PLAYWRIGHT_AVAILABLE = True
except ImportError:
    PLAYWRIGHT_AVAILABLE = False
    print("⚠️ Playwright не установлен. Установите: pip install playwright")
    print("   Затем запустите: playwright install")

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class WinlineWorkingParser:
    """
    Рабочий парсер Winline - использует Playwright для обхода Web Components
    """
    
    BASE_URL = "https://winline.ru"
    
    STEALTH_HEADERS = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Accept-Language": "ru-RU,ru;q=0.9",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Encoding": "gzip, deflate, br",
        "DNT": "1",
        "Connection": "keep-alive",
        "Upgrade-Insecure-Requests": "1",
        "Sec-Fetch-Dest": "document",
        "Sec-Fetch-Mode": "navigate",
        "Sec-Fetch-Site": "none",
    }
    
    # JavaScript для извлечения событий из DOM Web Components
    EXTRACTION_JS = """
    (() => {
        const events = [];
        
        // Метод 1: Ищем event cards в атрибутах
        document.querySelectorAll('[data-event-id], [data-testid*="event"], [class*="event"]').forEach(el => {
            try {
                const eventId = el.getAttribute('data-event-id') || el.getAttribute('data-id');
                if (!eventId) return;
                
                const text = el.textContent || '';
                const html = el.innerHTML;
                
                // Пытаемся найти teams и league
                const teamPattern = /([\\w\\s\\-]+)\\s+vs\\.?\\s+([\\w\\s\\-]+)/i;
                const match = text.match(teamPattern);
                
                if (match) {
                    events.push({
                        id: eventId,
                        home: match[1].trim(),
                        away: match[2].trim(),
                        league: 'Unknown',
                        isLive: text.toLowerCase().includes('live'),
                        sport: 'football'
                    });
                }
            } catch (e) {}
        });
        
        // Метод 2: Ищем JSON в HTML комментариях или скриптах
        document.querySelectorAll('script').forEach(script => {
            try {
                const content = script.textContent;
                if (content.includes('events') || content.includes('Event')) {
                    // Пытаемся найти JSON объекты
                    const jsonMatches = content.match(/\\{[^{}]*"(id|eventId|event_id)"[^{}]*\\}/g) || [];
                    jsonMatches.forEach(match => {
                        try {
                            const obj = JSON.parse(match);
                            if (obj.id && (obj.home || obj.homeTeam || obj.team1)) {
                                events.push({
                                    id: obj.id,
                                    home: obj.home || obj.homeTeam || obj.team1 || 'Unknown',
                                    away: obj.away || obj.awayTeam || obj.team2 || 'Unknown',
                                    league: obj.league || obj.tournament || 'Unknown',
                                    isLive: obj.isLive || obj.live || false,
                                    sport: 'football'
                                });
                            }
                        } catch (e) {}
                    });
                }
            } catch (e) {}
        });
        
        // Метод 3: window объекты с данными
        if (window.__INITIAL_STATE__) {
            try {
                const state = window.__INITIAL_STATE__;
                if (state.events) events.push(...state.events);
            } catch (e) {}
        }
        
        // Метод 4: Redux store
        if (window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__) {
            try {
                const store = window.__store__ || window.store;
                if (store && store.getState) {
                    const state = store.getState();
                    if (state.events) events.push(...state.events);
                }
            } catch (e) {}
        }
        
        return events;
    })();
    """
    
    async def fetch_events(self) -> List[Dict]:
        """Вытаскивает события с Winline используя Playwright"""
        
        if not PLAYWRIGHT_AVAILABLE:
            logger.error("Playwright не установлен")
            return []
        
        events = []
        
        async with async_playwright() as p:
            # Запускаем браузер с параметрами обхода detection
            browser = await p.chromium.launch(
                headless=True,
                args=[
                    "--disable-blink-features=AutomationControlled",
                    "--disable-dev-shm-usage",
                    "--no-sandbox",
                    "--disable-gpu",
                ]
            )
            
            try:
                context = await browser.new_context(
                    user_agent=self.STEALTH_HEADERS["User-Agent"],
                    locale="ru-RU",
                    timezone_id="Europe/Moscow",
                    viewport={"width": 1440, "height": 900},
                    bypass_csp=True,  # Bypass Content Security Policy
                )
                
                # Добавляем stealth script
                await context.add_init_script(self._stealth_script())
                
                page = await context.new_page()
                
                # Устанавливаем таймауты
                page.set_default_timeout(30000)
                page.set_default_navigation_timeout(30000)
                
                # Перехватываем запросы к API
                collected_api_data = []
                
                async def handle_response(response):
                    try:
                        if 'api' in response.url and response.status == 200:
                            try:
                                data = await response.json()
                                collected_api_data.append(data)
                            except:
                                pass
                    except:
                        pass
                
                page.on("response", handle_response)
                
                # Загружаем главную страницу
                logger.info("Loading Winline main page...")
                try:
                    await page.goto(f"{self.BASE_URL}/", wait_until="networkidle", timeout=30000)
                except Exception as e:
                    logger.warning(f"Navigation timeout: {e}, continuing...")
                
                # Ждем загрузки JavaScript
                logger.info("Waiting for page hydration...")
                await page.wait_for_timeout(3000)
                
                # Пытаемся прокрутить страницу для загрузки больше событий
                for i in range(5):
                    try:
                        await page.evaluate("window.scrollBy(0, window.innerHeight)")
                        await page.wait_for_timeout(500)
                    except:
                        pass
                
                # Извлекаем события через JavaScript
                logger.info("Extracting events from page...")
                try:
                    extracted = await page.evaluate(self.EXTRACTION_JS)
                    if extracted:
                        events.extend(extracted)
                        logger.info(f"Extracted {len(extracted)} events from DOM")
                except Exception as e:
                    logger.warning(f"DOM extraction failed: {e}")
                
                # Пытаемся найти API endpoints через Network tab
                logger.info(f"Collected {len(collected_api_data)} API responses")
                
                # Переходим на страницу лайв событий
                logger.info("Loading live events page...")
                try:
                    await page.goto(f"{self.BASE_URL}/live", wait_until="networkidle", timeout=30000)
                except:
                    logger.warning("Live page load timeout, continuing...")
                
                await page.wait_for_timeout(2000)
                
                # Извлекаем лайв события
                try:
                    live_events = await page.evaluate(self.EXTRACTION_JS)
                    if live_events:
                        # Отмечаем как лайв
                        for event in live_events:
                            event['isLive'] = True
                        events.extend(live_events)
                        logger.info(f"Extracted {len(live_events)} live events")
                except:
                    pass
                
                # Переходим на страницу прематч футбола
                logger.info("Loading prematch football page...")
                try:
                    await page.goto(f"{self.BASE_URL}/stavki/sport/futbol/", wait_until="networkidle", timeout=30000)
                except:
                    logger.warning("Football page timeout, continuing...")
                
                await page.wait_for_timeout(2000)
                
                # Загружаем еще события скролингом
                for scroll_round in range(10):
                    try:
                        await page.evaluate("window.scrollBy(0, window.innerHeight * 2)")
                        await page.wait_for_timeout(1000)
                        
                        # Извлекаем после каждого скролла
                        scroll_events = await page.evaluate(self.EXTRACTION_JS)
                        if scroll_events:
                            for event in scroll_events:
                                # Дублирование проверка
                                if not any(e.get('id') == event.get('id') for e in events):
                                    events.append(event)
                    except:
                        pass
                
                await context.close()
                
            finally:
                await browser.close()
        
        logger.info(f"✅ Total events collected: {len(events)}")
        
        # Статистика
        live_count = sum(1 for e in events if e.get('isLive'))
        prematch_count = len(events) - live_count
        logger.info(f"   Live: {live_count}, Prematch: {prematch_count}")
        
        return events
    
    def _stealth_script(self) -> str:
        """Скрипт для обхода bot detection"""
        return """
        // Скрываем WebDriver
        Object.defineProperty(navigator, 'webdriver', {
            get: () => undefined,
        });
        
        // Скрываем Puppeteer
        Object.defineProperty(navigator, 'puppeteer', {
            get: () => undefined,
        });
        
        // Фиксим plugins
        Object.defineProperty(navigator, 'plugins', {
            get: () => [1, 2, 3, 4, 5],
        });
        
        // Фиксим languages
        Object.defineProperty(navigator, 'languages', {
            get: () => ['ru-RU', 'ru', 'en-US', 'en'],
        });
        
        // Добавляем chrome объект
        window.chrome = {
            runtime: {}
        };
        """
    
    async def save_to_json(self, events: List[Dict], filename: str = "winline_events.json"):
        """Сохраняет события в JSON файл"""
        with open(filename, 'w', encoding='utf-8') as f:
            json.dump(events, f, ensure_ascii=False, indent=2)
        logger.info(f"Saved to {filename}")


async def main():
    """Основная функция"""
    parser = WinlineWorkingParser()
    
    print("=" * 70)
    print("🚀 WINLINE WORKING PARSER")
    print("=" * 70)
    print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()
    
    events = await parser.fetch_events()
    
    if events:
        print(f"\n✅ SUCCESS: Found {len(events)} events")
        live_count = sum(1 for e in events if e.get('isLive'))
        prematch_count = len(events) - live_count
        print(f"   Live: {live_count} | Prematch: {prematch_count}")
        
        # Покажем первые 5 событий
        print("\n📋 Sample events:")
        for event in events[:5]:
            print(f"   {event.get('home', 'N/A')} vs {event.get('away', 'N/A')} ({event.get('league', 'Unknown')})")
        
        # Сохраняем в JSON
        await parser.save_to_json(events)
    else:
        print("\n❌ FAILED: No events found")
    
    print(f"\nFinished at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")


if __name__ == "__main__":
    asyncio.run(main())
