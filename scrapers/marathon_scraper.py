# scrapers/marathon_scraper.py - Marathon scraper для вилок
import asyncio
import re
import logging
from typing import List, Dict
from playwright.async_api import async_playwright
from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class MarathonScraper(BaseScraper):
    def __init__(self):
        super().__init__()
        self.name = "Marathon"
        
        # ОСЛАБЛЕННЫЙ фильтр для Marathon (для вилок)
        self.exclude_patterns = [
            r'избранное', r'ближайшие', r'корзина', r'история', r'бонус',
            r'акция', r'кешбэк', r'генератор экспресса', r'размер коэффициента',
            r'сумма возм.выигрыша', r'только топ-события', r'добавить исход',
            r'обновить список', r'популярные события', r'добавить в корзину',
            r'подробнее', r'ежемесячно', r'деньгами', r'личный кабинет',
            r'пополнение', r'вывод', r'правила', r'помощь', r'поддержка'
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

                await page.goto("https://www.marathonbet.ru/live/football", wait_until="domcontentloaded", timeout=90000)
                await asyncio.sleep(12)

                # ЗАХВАТ для Marathon
                raw_blocks = await page.evaluate('''() => {
                    const results = [];
                    
                    // Ищем ТОВАРЩЕСКИЕ МАТЧИ СБОРНЫЕ (как у других БК)
                    document.querySelectorAll('div, span, p, section, article').forEach(el => {
                        const text = (el.innerText || '').trim();
                        
                        // Ищем только товарищеские матчи сборных
                        if (text.length > 20 && text.length < 150 &&
                            (text.includes('—') || text.includes('-') || text.includes('vs')) &&
                            (text.includes('сборн') || text.includes('товарищ') || text.includes('internation'))) {
                            results.push(text);
                        }
                    });
                    
                    // Если мало, добавляем все матчи
                    if (results.length < 20) {
                        document.querySelectorAll('div, span, p, section, article, [class*="event"], [class*="match"], [class*="row"]').forEach(el => {
                            const text = (el.innerText || '').trim();
                            if (text.length > 30 && (text.includes('—') || text.includes('vs') || text.includes('-'))) {
                                results.push(text);
                            }
                        });
                    }
                    
                    return results.slice(0, 100);
                }''')

                for block in raw_blocks:
                    try:
                        # Проверяем по exclude_patterns
                        skip_event = False
                        for pattern in self.exclude_patterns:
                            if re.search(pattern, block, re.IGNORECASE):
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
                                    'bookmaker': 'Marathon'
                                })

                            if any(x in block for x in ['2.5', '2,5']) and len(odds) >= 4:
                                events.append({
                                    'name': name,
                                    'normalized_name': normalize_event_name(name),
                                    'market_type': 'total',
                                    'total_value': 2.5,
                                    'over': odds[2],
                                    'under': odds[3],
                                    'bookmaker': 'Marathon'
                                })
                    except:
                        continue

                await browser.close()
        except Exception as e:
            logger.error(f"[Marathon] Error: {e}")

        logger.info(f"[Marathon] Получено {len(events)} событий")
        return events
