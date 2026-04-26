# scanner/parsers/betboom_playwright.py
"""
BetBoom Parser - Text extraction from main page
"""
import asyncio
import time
import json
import sys
import re
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)

GENERIC_TEAM_TOKENS = {
    'линия', 'live', 'акции', 'популярное', 'спорт', 'киберспорт', 'футбол', 'баскетбол',
    'теннис', 'волейбол', 'хоккей', 'ставки', 'сегодня', 'завтра', 'ещё'
}
GENERIC_CONTEXT_TOKENS = {
    'футбол', 'баскетбол', 'теннис', 'волейбол', 'хоккей', 'товарищеские', 'матчи', 'топ',
    'сборные', 'бразилия', 'серия', 'россия', 'лига', 'саудовская', 'аравия', 'премьер-лига'
}
GENERIC_SUBCATEGORY_TOKENS = {
    'лайв', 'live', 'все', '1ч', '3ч', '6ч', '12ч', '1д', '2д', '3д', '1н',
    'исход', 'тотал', 'фора', 'вход', 'регистрация', 'избранное', 'информация',
    'контакты', 'частые вопросы', 'персональные данные', 'товарный знак', 'документация',
    'школа ставок', 'ставки прямо сейчас', 'партнёры', 'спорт', 'кибер', 'акции', 'клубы'
}


class BetBoomPlaywrightParser:
    name = "BetBoom (Playwright)"
    slug = "betboom"
    urls = [
        "https://betboom.ru/sport/live",
        "https://betboom.ru/sport",
    ]
    direct_prematch_urls = [
        ("https://betboom.ru/sport/football", "Футбол"),
        ("https://betboom.ru/sport/tennis", "Теннис"),
        ("https://betboom.ru/sport/table-tennis", "Настольный теннис"),
        ("https://betboom.ru/sport/baseball", "Бейсбол"),
    ]
    live_categories = [
        'Теннис', 'Настольный теннис', 'Футбол', 'Бейсбол', 'Баскетбол', 'Хоккей',
        'Волейбол', 'Футзал', 'Гандбол', 'Крикет', 'Киберфутбол', 'Киберспорт', 'Кибербаскетбол',
        'Киберхоккей', 'Боулинг'
    ]
    live_priority_categories = ['Настольный теннис', 'Теннис', 'Киберспорт', 'Футбол', 'Бейсбол']
    live_filters = ['Все', '3ч', '12ч']
    prematch_categories = [
        'Теннис', 'Футбол', 'Бейсбол', 'Баскетбол', 'Хоккей', 'Футзал', 'Гандбол'
    ]
    prematch_priority_categories = ['Теннис', 'Футбол', 'Бейсбол']
    prematch_filters = ['Все', '1н', '1д']
    prematch_category_filters = {
        'Теннис': ['1н', 'Все'],
        'Футбол': ['1н', 'Все'],
        'Бейсбол': ['1н', 'Все'],
    }
    live_passes = 4
    prematch_passes = 4
    
    async def get_events(self) -> List[Dict]:
        merged = []
        seen = set()
        for url, passes in (("https://betboom.ru/sport/live", self.live_passes), ("https://betboom.ru/sport", self.prematch_passes)):
            for _ in range(passes):
                try:
                    events = await self._fetch_url(url)
                    for event in events:
                        key = (event.get('home_team'), event.get('away_team'), event.get('sport'), event.get('is_live'))
                        if key in seen:
                            continue
                        seen.add(key)
                        merged.append(event)
                except Exception as e:
                    logger.warning(f"BetBoom failed for {url}: {e}")

        for url, hint in self.direct_prematch_urls:
            try:
                events = await self._fetch_direct_prematch_url(url, hint)
                for event in events:
                    key = (event.get('home_team'), event.get('away_team'), event.get('sport'), event.get('is_live'))
                    if key in seen:
                        continue
                    seen.add(key)
                    merged.append(event)
            except Exception as e:
                logger.warning(f"BetBoom direct prematch failed for {url}: {e}")
        return merged
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled', '--no-sandbox', '--disable-dev-shm-usage']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            window.chrome = {runtime: {}};
        """)
        
        page = await context.new_page()
        
        # Intercept API calls
        api_data = []
        async def handle_response(response):
            url = response.url
            ct = response.headers.get('content-type', '')
            if response.status == 200 and 'json' in ct:
                try:
                    data = await response.json()
                    if isinstance(data, dict) and len(str(data)) > 100:
                        api_data.append({'url': url, 'data': data})
                except:
                    pass
        
        page.on('response', handle_response)
        
        try:
            await self._goto_with_retry(page, url)
            await self._accept_cookie_if_present(page)
            await self._wait_for_compact_markers(page)
            
            events = []

            if url.endswith('/sport/live'):
                events = await self._extract_live_categories(page, url)
            elif url.endswith('/sport'):
                events = await self._extract_prematch_categories(page, url)
            else:
                # Try to extract from iframes
                for frame in page.frames:
                    if frame != page.main_frame and 'betboom' in frame.url:
                        try:
                            frame_events = await self._extract_from_frame(frame, url)
                            events.extend(frame_events)
                        except:
                            pass

                if not events:
                    events = await self._extract_from_compact_cards(page, url)

                if not events:
                    events = await self._extract_from_text(page, url)
            
            logger.info(f"BetBoom ({url}): {len(events)} events")
        except Exception as e:
            logger.warning(f"BetBoom error: {e}")
            events = []
        finally:
            await browser.close()
        
        return events

    async def _fetch_direct_prematch_url(self, url: str, sport_hint: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled', '--no-sandbox', '--disable-dev-shm-usage']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            window.chrome = {runtime: {}};
        """)

        page = await context.new_page()
        try:
            await self._goto_with_retry(page, url)
            await self._accept_cookie_if_present(page)
            await self._wait_for_compact_markers(page)
            seen = set()
            events = []
            for _ in range(20):
                batch = await self._extract_from_text(page, url, sport_hint)
                for event in batch:
                    key = (event.get('home_team'), event.get('away_team'))
                    if key in seen:
                        continue
                    seen.add(key)
                    events.append(event)
                try:
                    await page.mouse.wheel(0, 2400)
                except Exception:
                    pass
                await asyncio.sleep(1.5)
            return events
        finally:
            await browser.close()

    async def _goto_with_retry(self, page, url: str) -> None:
        last_error = None
        for _ in range(3):
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=30000)
                return
            except Exception as e:
                last_error = e
                await asyncio.sleep(2)
        raise last_error

    async def _extract_live_categories(self, page, url: str) -> List[Dict]:
        all_events = []
        seen = set()

        for filter_name in self.live_filters:
            await self._click_visible_text(page, filter_name)
            await asyncio.sleep(3)

            for category in self.live_categories + self.live_priority_categories:
                try:
                    clicked = await self._click_visible_text(page, category)
                    if not clicked:
                        continue
                    await asyncio.sleep(4)
                    rounds = 14 if category in self.live_priority_categories else 10
                    for _ in range(rounds):
                        try:
                            await page.mouse.wheel(0, 1600)
                        except Exception:
                            pass
                        await asyncio.sleep(2)
                        category_events = await self._extract_from_text(page, url, category)
                        for event in category_events:
                            key = (event.get('home_team'), event.get('away_team'))
                            if key in seen:
                                continue
                            seen.add(key)
                            all_events.append(event)
                except Exception:
                    continue

        return all_events

    async def _extract_prematch_categories(self, page, url: str) -> List[Dict]:
        all_events = []
        seen = set()

        for category in self.prematch_categories + self.prematch_priority_categories:
            category_filters = self.prematch_category_filters.get(category, self.prematch_filters)
            for filter_name in category_filters:
                clicked_filter = await self._click_visible_text(page, filter_name)
                if not clicked_filter:
                    continue
                await asyncio.sleep(4)

                clicked_category = await self._click_visible_text(page, category)
                if not clicked_category:
                    continue
                await asyncio.sleep(4)
                sublabels = await self._extract_subcategory_labels(page, category)

                # First collect from category root view.
                root_rounds = 14 if category in self.prematch_priority_categories else 10
                for _ in range(root_rounds):
                    try:
                        await page.mouse.wheel(0, 2200)
                    except Exception:
                        pass
                    await asyncio.sleep(2)
                    category_events = await self._extract_from_text(page, url, category)
                    for event in category_events:
                        key = (event.get('home_team'), event.get('away_team'))
                        if key in seen:
                            continue
                        seen.add(key)
                        all_events.append(event)

                # Then drill into the most promising subcategories/leagues.
                label_limit = 30 if category in self.prematch_priority_categories else 20
                for label in sublabels[:label_limit]:
                    clicked_label = await self._click_visible_text(page, label, prefix=True)
                    if not clicked_label:
                        continue
                    await asyncio.sleep(3)
                    rounds = 10 if category in self.prematch_priority_categories else 8
                    for _ in range(rounds):
                        try:
                            await page.mouse.wheel(0, 1800)
                        except Exception:
                            pass
                        await asyncio.sleep(2)
                        label_events = await self._extract_from_text(page, url, category)
                        for event in label_events:
                            key = (event.get('home_team'), event.get('away_team'))
                            if key in seen:
                                continue
                            seen.add(key)
                            all_events.append(event)

        return all_events

    async def _accept_cookie_if_present(self, page) -> None:
        try:
            btn = page.get_by_role('button', name='Окей')
            await btn.click(timeout=2500)
            await asyncio.sleep(1)
        except Exception:
            pass

    async def _wait_for_compact_markers(self, page) -> None:
        for _ in range(7):
            try:
                counts = await page.evaluate("""() => ({
                    bbNm: document.querySelectorAll('.bb-Nm').length,
                    bbRm: document.querySelectorAll('.bb-Rm').length,
                    bbKG: document.querySelectorAll('.bb-KG').length,
                    bodyLen: ((document.body && document.body.innerText) || '').length,
                })""")
                if counts.get('bbNm', 0) or counts.get('bbRm', 0) or counts.get('bbKG', 0):
                    return
                if counts.get('bodyLen', 0) > 2000:
                    return
            except Exception:
                pass

            try:
                await page.mouse.wheel(0, 1200)
            except Exception:
                pass
            await asyncio.sleep(4)

    async def _extract_from_compact_cards(self, page, url: str) -> List[Dict]:
        """Extract events from compact betboom cards using known bb-* classes."""
        try:
            raw_events = await page.evaluate("""
                () => {
                    const results = [];
                    const visible = (node) => !!(node && (node.offsetWidth || node.offsetHeight || node.getClientRects().length));
                    const norm = (value) => String(value || '').replace(/\u00a0/g, ' ').replace(/\s+/g, ' ').trim();
                    const containers = Array.from(document.querySelectorAll('.bb-Nm'));

                    containers.forEach((container) => {
                        try {
                            if (!visible(container)) return;
                            const parent = container.parentElement;
                            const grandparent = parent?.parentElement;
                            const greatGrandparent = grandparent?.parentElement;
                            const teamText = norm((greatGrandparent || grandparent || parent || container)?.textContent || '');
                            const odds = Array.from(container.querySelectorAll('.bb-Rm'))
                                .map((node) => norm(node.textContent))
                                .filter((txt) => /^\d+[\.,]\d+$/.test(txt))
                                .map((txt) => parseFloat(txt.replace(',', '.')))
                                .filter((val) => Number.isFinite(val) && val >= 1.01 && val <= 100)
                                .slice(0, 3);

                            if (odds.length < 2 || teamText.length < 20) return;

                            const teams = teamText.split(/\n|\r/).map(norm).filter((t) => t.length > 2 && t.length < 40);
                            results.push({
                                home: teams[0] || '',
                                away: teams[1] || teams[teams.length - 1] || '',
                                odds,
                                team_text: teamText,
                                parent_classes: parent?.className || '',
                                grandparent_classes: grandparent?.className || ''
                            });
                        } catch (_) {}
                    });

                    return results.slice(0, 120);
                }
            """)

            return self._normalize(raw_events, url)
        except Exception:
            return []
    
    async def _extract_from_frame(self, frame, url: str) -> List[Dict]:
        """Extract events from iframe."""
        try:
            raw_events = await frame.evaluate("""
                () => {
                    const events = [];
                    const containers = document.querySelectorAll('[class*="event"], [class*="match"], [class*="coupon"], [class*="card"]');
                    
                    containers.forEach(el => {
                        const text = el.textContent || '';
                        if (!text || text.length < 20) return;
                        
                        const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                        const teams = [];
                        const odds = [];
                        
                        for (const line of lines) {
                            const val = parseFloat(line.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                odds.push(val);
                            } else if (line.length > 2 && line.length < 40) {
                                teams.push(line);
                            }
                            if (teams.length >= 2 && odds.length >= 1) break;
                        }
                        
                        if (teams.length >= 2 && odds.length >= 1) {
                            events.push({
                                home: teams[0],
                                away: teams[1],
                                odds: odds.slice(0, 4)
                            });
                        }
                    });
                    
                    return events;
                }
            """)
            
            return self._normalize(raw_events, url)
        except:
            return []
    
    async def _extract_from_text(self, page, url: str, sport_hint: str | None = None) -> List[Dict]:
        """Extract events from page text."""
        try:
            text = await page.evaluate("(document.body && document.body.innerText) || ''")
            lines = [line.strip() for line in text.split('\n') if line.strip()]
            raw_events = self._extract_structured_live_events(lines, sport_hint) if 'live' in url else self._extract_structured_prematch_events(lines, sport_hint)
            if not raw_events:
                raw_events = self._extract_loose_text_events(lines, sport_hint)
            
            return self._normalize(raw_events, url)
        except:
            return []

    def _extract_structured_live_events(self, lines: List[str], sport_hint: str | None) -> List[Dict]:
        events = []
        current_league = None
        started = False

        for i, line in enumerate(lines):
            if line == 'Лайв':
                started = True
                continue
            if not started:
                continue

            if self._looks_like_league(line):
                current_league = line
                continue

            if line != 'П1':
                continue

            home = self._find_previous_team(lines, i - 1, set())
            away = self._find_previous_team(lines, i - 1, {home} if home else set())
            if not home or not away:
                continue

            odds = []
            for needle in ('П1', 'X', 'П2'):
                idx = self._find_next(lines, needle, i, 8)
                if idx is None or idx + 1 >= len(lines):
                    continue
                try:
                    odds.append(float(lines[idx + 1].replace(',', '.')))
                except Exception:
                    continue

            if len(odds) >= 2:
                events.append({
                    'home': home,
                    'away': away,
                    'odds': odds[:3],
                    'league': current_league or 'Live',
                    'sport': self._normalize_sport_hint(sport_hint),
                })

        return events

    def _extract_loose_text_events(self, lines: List[str], sport_hint: str | None) -> List[Dict]:
        events = []
        i = 0
        while i < len(lines) - 2:
            line = lines[i]
            if self._looks_like_team_name(line):
                maybe_next = lines[i + 1]
                if self._looks_like_team_name(maybe_next):
                    odds = []
                    for j in range(i + 2, min(len(lines), i + 12)):
                        try:
                            value = float(lines[j].replace(',', '.'))
                            if 1.01 <= value <= 100:
                                odds.append(value)
                        except Exception:
                            pass
                    if len(odds) >= 2:
                        events.append({
                            'home': line,
                            'away': maybe_next,
                            'odds': odds[:3],
                            'league': sport_hint or ('Live' if 'live' in lines[:20] else 'Pre-match'),
                            'sport': self._normalize_sport_hint(sport_hint),
                        })
                        i += 6
                        continue
            i += 1
        return events

    def _extract_structured_prematch_events(self, lines: List[str], sport_hint: str | None) -> List[Dict]:
        events = []
        current_league = None
        start_idx = 0
        if sport_hint:
            for idx, line in enumerate(lines):
                if line == sport_hint:
                    start_idx = idx
        for i in range(start_idx, len(lines)):
            line = lines[i]

            if self._looks_like_league(line):
                current_league = line
                continue

            if line != 'П1':
                continue

            home = self._find_previous_team(lines, i - 1, set())
            away = self._find_previous_team(lines, i - 1, {home} if home else set())
            if not home or not away:
                continue

            odds = []
            for needle in ('П1', 'X', 'П2'):
                idx = self._find_next(lines, needle, i, 8)
                if idx is None or idx + 1 >= len(lines):
                    continue
                try:
                    odds.append(float(lines[idx + 1].replace(',', '.')))
                except Exception:
                    continue

            if len(odds) >= 2:
                events.append({
                    'home': home,
                    'away': away,
                    'odds': odds[:3],
                    'league': current_league or (sport_hint or 'Pre-match'),
                    'sport': self._normalize_sport_hint(sport_hint),
                })

        return events
    
    def _normalize(self, raw_events: list, url: str) -> List[Dict]:
        result = []
        seen = set()
        
        for e in raw_events:
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            team_text = e.get('team_text', '').strip()

            if not self._looks_like_team_name(home) or not self._looks_like_team_name(away):
                extracted = self._extract_teams_from_blob(team_text)
                if extracted:
                    home, away = extracted
            
            if not home or not away or len(home) < 2 or len(away) < 2:
                continue

            if not self._looks_like_team_name(home) or not self._looks_like_team_name(away):
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 1:
                continue
            
            result.append({
                'id': f"betboom_{hash(key) % 1000000}",
                'bookmaker': 'betboom',
                'sport': e.get('sport', 'football'),
                'home_team': home,
                'away_team': away,
                'league': e.get('league') or ('Live' if 'live' in url else 'Pre-match'),
                'home_odds': odds[0],
                'draw_odds': odds[1] if len(odds) > 2 else None,
                'away_odds': odds[2] if len(odds) > 2 else (odds[1] if len(odds) > 1 else 0),
                'is_live': 'live' in url,
                'market': '1x2',
                'source_url': url,
                'scraped_at': time.time()
            })
        
        return result

    def _looks_like_team_name(self, value: str) -> bool:
        value = (value or '').strip()
        if len(value) < 2 or len(value) > 40:
            return False
        lowered = value.lower()
        if lowered in GENERIC_TEAM_TOKENS:
            return False
        if lowered in {'не начался', 'событие не началось', 'перерыв'}:
            return False
        if value.startswith('+ '):
            return False
        if re.fullmatch(r'\d+-[йя]\s+(сет|карта|период|тайм)', lowered):
            return False
        if re.fullmatch(r'\d+(?:-\d+){1,3}', value):
            return False
        if re.fullmatch(r'\d+', value):
            return False
        if 'мин' in lowered or 'перерыв' in lowered:
            return False
        if value.isdigit():
            return False
        return True

    def _looks_like_league(self, value: str) -> bool:
        value = (value or '').strip()
        if len(value) < 5 or len(value) > 80:
            return False
        lowered = value.lower()
        return any(token in lowered for token in ['лига', 'серия', 'дивизион', 'примера', 'премьер', 'кубок'])

    def _find_previous_team(self, lines: List[str], start: int, excluded: set):
        for idx in range(start, max(-1, start - 10), -1):
            candidate = lines[idx].strip()
            if candidate in excluded:
                continue
            if self._looks_like_team_name(candidate):
                return candidate
        return None

    def _find_next(self, lines: List[str], needle: str, start: int, window: int):
        end = min(len(lines), start + window)
        for idx in range(start, end):
            if lines[idx] == needle:
                return idx
        return None

    async def _extract_subcategory_labels(self, page, category: str) -> List[str]:
        try:
            labels = await page.evaluate(
                """(category) => {
                    const text = (document.body && document.body.innerText) || '';
                    const lines = text.split('\n').map(x => x.trim()).filter(Boolean);
                    const start = lines.indexOf(category);
                    if (start < 0) return [];
                    const end = lines.indexOf('LIVE', start + 1);
                    const slice = lines.slice(start + 1, end > 0 ? end : Math.min(lines.length, start + 120));
                    const out = [];
                    for (let i = 0; i < slice.length - 1; i++) {
                        const label = slice[i];
                        const count = slice[i + 1];
                        if (/^\d+$/.test(count) && label.length > 2 && label.length < 60) {
                            out.push({ label, count: parseInt(count, 10) });
                        }
                    }
                    out.sort((a, b) => b.count - a.count);
                    return out.map(x => x.label);
                }""",
                category,
            )
            result = []
            for label in labels:
                lowered = label.lower()
                if lowered in GENERIC_SUBCATEGORY_TOKENS:
                    continue
                if label == category:
                    continue
                result.append(label)
            return result
        except Exception:
            return []

    async def _click_visible_text(self, page, target_text: str, prefix: bool = False) -> bool:
        try:
            return await page.evaluate(
                """({targetText, prefix}) => {
                    const normalize = (value) => String(value || '').replace(/\u00a0/g, ' ').replace(/\s+/g, ' ').trim();
                    const nodes = Array.from(document.querySelectorAll('button, a, div, span'));
                    const target = nodes.find((node) => {
                        const text = normalize(node.textContent || '');
                        if (prefix) {
                            if (!(text === targetText || text.startsWith(targetText + ' '))) return false;
                        } else if (text !== targetText) {
                            return false;
                        }
                        const rect = node.getBoundingClientRect();
                        return rect.width > 0 && rect.height > 0;
                    });
                    if (!target) return false;
                    target.click();
                    return true;
                }""",
                {'targetText': target_text, 'prefix': prefix},
            )
        except Exception:
            return False

    def _extract_teams_from_blob(self, text: str):
        text = (text or '').strip()
        if len(text) < 10:
            return None

        # Cut off everything after time/status/markets.
        text = re.split(r'(Сегодня|Завтра|\d{1,2}:\d{2}|\d+\s+апреля|П1|П2|\bX\b|Ещё)', text, maxsplit=1)[0]
        text = re.sub(r'\d+-\d+-\d+', ' ', text)
        text = re.sub(r'\s+', ' ', text).strip()

        # Split camel-like Cyrillic/Latin boundaries inside merged text.
        text = re.sub(r'(?<=[a-zа-я])(?=[A-ZА-Я])', ' ', text)
        text = re.sub(r'(?<=[A-ZА-Я])(?=[A-ZА-Я][a-zа-я])', ' ', text)

        parts = [p.strip() for p in re.split(r'\s{2,}|\n|\r', text) if p.strip()]
        token_text = ' '.join(parts)
        token_text = re.sub(r'Футбол\.?', ' ', token_text)
        token_text = re.sub(r'Баскетбол\.?', ' ', token_text)
        token_text = re.sub(r'Теннис\.?', ' ', token_text)
        token_text = re.sub(r'Волейбол\.?', ' ', token_text)
        token_text = re.sub(r'Хоккей\.?', ' ', token_text)
        token_text = re.sub(r'\s+', ' ', token_text).strip()

        tokens = re.findall(r'[A-ZА-Я][A-Za-zА-Яа-я\-]{1,}', token_text)
        tokens = [t.strip() for t in tokens if t.strip()]
        while tokens and (tokens[0].lower() in GENERIC_CONTEXT_TOKENS or len(tokens[0]) == 1):
            tokens.pop(0)
        while tokens and (tokens[-1].lower() in GENERIC_TEAM_TOKENS or len(tokens[-1]) == 1):
            tokens.pop()

        if len(tokens) >= 4:
            home = ' '.join(tokens[:-2])
            away = ' '.join(tokens[-2:])
        elif len(tokens) == 3:
            home = tokens[0]
            away = ' '.join(tokens[1:])
        elif len(tokens) == 2:
            home, away = tokens
        else:
            return None

        if self._looks_like_team_name(home) and self._looks_like_team_name(away):
            return home, away

        return None

    def _normalize_sport_hint(self, value: str | None) -> str:
        lowered = (value or '').strip().lower()
        if 'настольный теннис' in lowered:
            return 'table_tennis'
        if 'теннис' in lowered:
            return 'tennis'
        if 'баскетбол' in lowered:
            return 'basketball'
        if 'хоккей' in lowered:
            return 'hockey'
        if 'волейбол' in lowered:
            return 'volleyball'
        if 'футзал' in lowered:
            return 'futsal'
        if 'бейсбол' in lowered:
            return 'baseball'
        if 'гандбол' in lowered:
            return 'handball'
        if 'кибер' in lowered:
            return 'esports'
        return 'football'


async def _main():
    parser = BetBoomPlaywrightParser()
    events = await parser.get_events()
    payload = json.dumps(events, ensure_ascii=False)
    sys.stdout.buffer.write(payload.encode('utf-8'))


if __name__ == '__main__':
    asyncio.run(_main())
