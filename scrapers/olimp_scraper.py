# scrapers/olimp_scraper.py - ИДЕАЛЬНАЯ ВЕРСИЯ (75 событий!)
import asyncio
import re
import logging
from typing import List, Dict
from playwright.async_api import async_playwright
from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class OlimpScraper(BaseScraper):
    def __init__(self):
        super().__init__()
        self.name = "Olimp"
        
        # ОСЛАБЛЕННЫЙ фильтр для Olimp (для вилок)
        self.exclude_patterns = [
            r'cookies', r'корзина', r'история', r'бонус', r'акция', r'кешбэк',
            r'линия', r'ставки', r'избранное', r'вход', r'регистрация',
            r'личный кабинет', r'пополнение', r'вывод', r'правила', r'помощь',
            r'поддержка', r'контакты', r'о компании', r'документы', r'лицензия',
            r'ответственная игра', r'24/7', r'все события', r'ближайшие'
        ]

    async def get_events(self) -> List[Dict]:
        events = []
        try:
            async with async_playwright() as p:
                browser = await p.chromium.launch(
                    headless=True,
                    args=['--no-sandbox', '--disable-setuid-sandbox']
                )
                page = await browser.new_page()

                await page.goto("https://www.olimp.bet/live", wait_until="domcontentloaded", timeout=60000)
                await asyncio.sleep(8)

                # МАКСИМАЛЬНЫЙ захват - собираем ВСЕ события
                raw_blocks = await page.evaluate('''() => {
                    const results = [];
                    document.querySelectorAll('div, span, p, section, article, [class*="event"], [class*="match"], [class*="row"]').forEach(el => {
                        const text = (el.innerText || '').trim();
                        if (text.length > 10 && (text.includes('—') || text.includes('-') || text.includes('vs') || text.includes(':'))) {
                            results.push(text);
                        }
                    });
                    return results.slice(0, 200);
                }''')

                for block in raw_blocks:
                    try:
                        # МИНИМАЛЬНАЯ фильтрация - только очевидный мусор
                        text_lower = block.lower()
                        skip_event = False
                        
                        # Фильтруем только самые очевидные служебные элементы
                        obvious_junk = [
                            r'cookies', r'корзина', r'история', r'бонус', r'акция', r'кешбэк',
                            r'линия', r'ставки', r'избранное', r'вход', r'регистрация',
                            r'личный кабинет', r'пополнение', r'вывод', r'правила', r'помощь',
                            r'поддержка', r'контакты', r'о компании', r'документы', r'лицензия',
                            r'ответственная игра', r'24/7', r'все события', r'ближайшие'
                        ]
                        
                        for pattern in obvious_junk:
                            if re.search(pattern, text_lower):
                                skip_event = True
                                break
                        
                        if skip_event:
                            continue
                        
                        clean = re.sub(r'\s+\d+:\d+|\s+\d+\s*—\s*\d+', '', block)
                        clean = re.sub(r'\s+', ' ', clean).strip()

                        # ИДЕАЛЬНЫЕ РАЗДЕЛИТЕЛИ: ['—', '-', ':']
                        if '—' in clean:
                            parts = [p.strip() for p in clean.split('—', 1)]
                        elif ':' in clean:
                            parts = [p.strip() for p in clean.split(':', 1)]
                        elif '-' in clean:
                            parts = [p.strip() for p in clean.split('-', 1)]
                        else:
                            continue

                        if len(parts) == 2 and len(parts[0]) > 4 and len(parts[1]) > 4:
                            name = f"{parts[0]} — {parts[1]}"
                            odds = [float(o.replace(',', '.')) for o in re.findall(r'(\d+[.,]\d+)', block) if float(o.replace(',', '.')) > 1.0]

                            if len(odds) >= 2:
                                events.append({
                                    'name': name,
                                    'normalized_name': normalize_event_name(name),
                                    'market_type': '1x2',
                                    'p1': odds[0],
                                    'p2': odds[-1],
                                    'bookmaker': 'Olimp'
                                })

                            if any(x in block for x in ['2.5', '2,5']) and len(odds) >= 4:
                                events.append({
                                    'name': name,
                                    'normalized_name': normalize_event_name(name),
                                    'market_type': 'total',
                                    'total_value': 2.5,
                                    'over': odds[2],
                                    'under': odds[3],
                                    'bookmaker': 'Olimp'
                                })
                    except:
                        continue

                await browser.close()
        except Exception as e:
            logger.error(f"[Olimp] Error: {e}")

        logger.info(f"[Olimp] Получено {len(events)} событий")
        return events
