# scrapers/pari_scraper.py - С ИНДИВИДУАЛЬНЫМ ФИЛЬТРОМ КАК У ДРУГИХ БК
import asyncio
import re
import logging
from typing import List, Dict
from playwright.async_api import async_playwright
from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class PariScraper(BaseScraper):
    def __init__(self):
        super().__init__()
        self.name = "Pari"
        
        # ОСЛАБЛЕННЫЙ фильтр для Pari (для вилок)
        self.exclude_patterns = [
            r'киберспорт', r'лотерея', r'игры', r'24/7', r'secret', 
            r'медиа', r'приложения', r'результаты', r'статистика',
            r'корзина', r'история', r'бонусный клуб', r'кешбэк',
            r'генератор экспресса', r'размер коэффициента',
            r'сумма возм.выигрыша', r'только топ-события',
            r'добавить исход', r'обновить список', r'популярные события',
            r'добавить в корзину', r'подробнее', r'ежемесячно', r'деньгами'
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

                await page.goto("https://www.pari.ru/live/football", wait_until="domcontentloaded", timeout=90000)
                await asyncio.sleep(12)

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
                            r'киберспорт', r'лотерея', r'игры', r'24/7', r'secret', 
                            r'медиа', r'приложения', r'результаты', r'статистика',
                            r'корзина', r'история', r'бонусный клуб', r'кешбэк',
                            r'генератор экспресса', r'размер коэффициента',
                            r'сумма возм.выигрыша', r'только топ-события',
                            r'добавить исход', r'обновить список', r'популярные события',
                            r'добавить в корзину', r'подробнее', r'ежемесячно', r'деньгами'
                        ]
                        
                        for pattern in obvious_junk:
                            if re.search(pattern, text_lower):
                                skip_event = True
                                break
                        
                        if skip_event:
                            continue
                        
                        clean = re.sub(r'\s+\d+:\d+|\s+\d+\s*—\s*\d+', '', block)
                        clean = re.sub(r'\s+', ' ', clean).strip()

                        if '—' in clean:
                            parts = [p.strip() for p in clean.split('—', 1)]
                        elif ':' in clean:
                            parts = [p.strip() for p in clean.split(':', 1)]
                        elif '-' in clean:
                            parts = [p.strip() for p in clean.split('-', 1)]
                        elif 'vs' in clean:
                            parts = [p.strip() for p in clean.split('vs', 1)]
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
                                    'bookmaker': 'Pari'
                                })

                            if any(x in block for x in ['2.5', '2,5']) and len(odds) >= 4:
                                events.append({
                                    'name': name,
                                    'normalized_name': normalize_event_name(name),
                                    'market_type': 'total',
                                    'total_value': 2.5,
                                    'over': odds[2],
                                    'under': odds[3],
                                    'bookmaker': 'Pari'
                                })
                    except:
                        continue

                await browser.close()
        except Exception as e:
            logger.error(f"[Pari] Error: {e}")

        logger.info(f"[Pari] Получено {len(events)} событий")
        return events
