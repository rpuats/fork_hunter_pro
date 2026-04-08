# scanner/parsers/zenit_playwright.py
"""
Zenit Playwright Parser - Real data extraction from SPA
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

logger = logging.getLogger(__name__)


class ZenitPlaywrightParser:
    name = "Zenit (Playwright)"
    slug = "zenit"
    urls = [
        "https://zenit.win/live/football",
        "https://zenit.win/line/football",
        "https://zenit.win/live/cyber-sport",
        "https://zenit.win/live/basketball",
        "https://zenit.win/line/basketball",
        "https://zenit.win/live/hockey",
        "https://zenit.win/line/hockey",
        "https://zenit.win/live/tennis",
        "https://zenit.win/line/tennis",
        "https://zenit.win/live/volleyball",
        "https://zenit.win/line/volleyball",
        "https://zenit.win/live/baseball",
        "https://zenit.win/line/baseball",
        "https://zenit.win/live/handball",
        "https://zenit.win/line/handball",
        "https://zenit.win/live/rugby",
        "https://zenit.win/line/rugby",
        "https://zenit.win/live/table-tennis",
        "https://zenit.win/line/table-tennis",
        "https://zenit.win/live/badminton",
        "https://zenit.win/line/badminton",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        seen = set()
        for url_idx, url in enumerate(self.urls):
            try:
                events = await self._fetch_url(url)
                for e in events:
                    key = e.get('home_team','') + '|' + e.get('away_team','')
                    if key not in seen:
                        seen.add(key)
                        all_events.append(e)
                if len(all_events) > 0 and url_idx >= 3:
                    logger.info(f"Zenit: early break after {url_idx + 1} URLs, {len(all_events)} events")
                    break
            except Exception as e:
                logger.warning(f"Zenit failed for {url}: {e}")
                continue
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled']
        )
        config = generate_stealth_config()
        context = await create_stealth_context(browser, config)
        
        page = await context.new_page()
        await page.goto(url, wait_until='domcontentloaded', timeout=15000)
        await asyncio.sleep(2)
        
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
        await asyncio.sleep(0.5)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(0.5)
        
        events = await self._extract_events(page, url)
        logger.info(f"Zenit ({url}): Extracted {len(events)} events")
        
        await browser.close()
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        raw_events = await page.evaluate("""
            () => {
                const events = [];
                
                const containers = document.querySelectorAll(
                    '[class*="event"], [class*="match"], [class*="game"], .sport-event, .event-item, .match-item, .game-item'
                );
                
                containers.forEach((el, idx) => {
                    try {
                        const text = el.textContent || '';
                        if (!text || text.length < 10) return;
                        
                        const odds = [];
                        el.querySelectorAll('[class*="coef"], [class*="rate"], [class*="kef"], .coef, .rate, .kef, .price').forEach(n => {
                            const val = parseFloat(n.textContent.trim().replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                odds.push(val);
                            }
                        });
                        
                        const lines = text.split(/\\n/).filter(l => l.trim());
                        let teams = [];
                        
                        for (const line of lines) {
                            const clean = line.trim();
                            if (clean.length > 2 && clean.length < 50 &&
                                !clean.match(/^\\d+[.,]\\d+$/) &&
                                !clean.match(/^\\d{1,2}:\\d{2}/) &&
                                !clean.match(/^LIVE$/i) &&
                                clean.length > 2) {
                                if (teams.length === 0 || teams[teams.length - 1] !== clean) {
                                    teams.push(clean);
                                }
                            }
                            if (teams.length >= 3) break;
                        }
                        
                        const home = teams[0] || '';
                        const away = teams.length > 1 ? teams[teams.length - 1] : (teams[1] || '');
                        
                        // Extract totals
                        const totals_over = {};
                        const totals_under = {};
                        
                        // Look for total/точал elements within event
                        el.querySelectorAll('[class*="total"], [class*="sum"], [class*="tb"], [class*="tm"]').forEach(t => {
                            const tText = t.textContent || '';
                            const lineMatch = tText.match(/(\d+[.,]\d)/);
                            const oddsMatch = tText.match(/(\d+[.,]\d{1,2})/g);
                            if (lineMatch && oddsMatch) {
                                const line = parseFloat(lineMatch[1].replace(',', '.'));
                                for (const om of oddsMatch) {
                                    const val = parseFloat(om.replace(',', '.'));
                                    if (val >= 1.01 && val <= 50 && Math.abs(val - line) > 0.5) {
                                        if (tText.toLowerCase().includes('больше') || tText.toLowerCase().includes('over') || tText.toLowerCase().includes('б') || /бо/i.test(tText)) {
                                            totals_over[line] = val;
                                        } else if (tText.toLowerCase().includes('меньше') || tText.toLowerCase().includes('under') || tText.toLowerCase().includes('м') || /ме/i.test(tText)) {
                                            totals_under[line] = val;
                                        }
                                    }
                                }
                            }
                        });
                        
                        // Also scan lines for "Тотал" patterns
                        for (let li = 0; li < lines.length; li++) {
                            const l = lines[li].trim();
                            if (/тотал|total|тб|тм/i.test(l)) {
                                const lineMatch = l.match(/(\d+[.,]\d)/);
                                if (lineMatch) {
                                    const line = parseFloat(lineMatch[1].replace(',', '.'));
                                    // Look for odds near this line
                                    const nextLines = lines.slice(li, li + 3).join(' ');
                                    const oddsNearby = nextLines.match(/(\d+[.,]\d{1,2})/g) || [];
                                    for (const om of oddsNearby) {
                                        const val = parseFloat(om.replace(',', '.'));
                                        if (val >= 1.01 && val <= 50 && Math.abs(val - line) > 0.5) {
                                            if (/бо|over|больше/i.test(nextLines)) {
                                                totals_over[line] = val;
                                            } else if (/ме|under|меньше/i.test(nextLines)) {
                                                totals_under[line] = val;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if (home && away && odds.length >= 2) {
                            events.push({
                                home,
                                away,
                                odds: odds.slice(0, 3),
                                totals_over,
                                totals_under
                            });
                        }
                    } catch(e) {}
                });
                
                if (events.length === 0) {
                    const lines = document.body.innerText.split(/\\n/);
                    for (let i = 0; i < lines.length; i++) {
                        const line = lines[i].trim();
                        if (!line) continue;
                        
                        const odds = line.match(/\\d+[.,]\\d{1,3}/g) || [];
                        const validOdds = odds.map(o => parseFloat(o.replace(',', '.'))).filter(v => v >= 1.05 && v <= 30);
                        
                        if (validOdds.length >= 2) {
                            const prevLines = lines.slice(Math.max(0, i-5), i).join(' ');
                            const teams = prevLines.match(/[A-ZА-ЯЁ][a-zа-яё]{2,}(?:\\s+[A-ZА-ЯЁ][a-zа-яё]{2,})*/g) || [];
                            
                            if (teams.length >= 2) {
                                events.push({
                                    home: teams[0],
                                    away: teams[teams.length - 1],
                                    odds: validOdds.slice(0, 3)
                                });
                            }
                        }
                    }
                }
                
                return events;
            }
        """)
        
        result = []
        seen = set()
        for i, e in enumerate(raw_events):
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            if not home or not away:
                continue
            if len(home) < 2 or len(away) < 2:
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 2:
                continue
            
            is_3way = len(odds) >= 3
            
            totals_over = e.get('totals_over', {}) or {}
            totals_under = e.get('totals_under', {}) or {}
            
            event = {
                'id': f"zenit_{i}_{hash(key) % 1000000}",
                'bookmaker': 'zenit',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if 'live' in url else 'Pre-match',
                'home_odds': odds[0],
                'draw_odds': odds[1] if is_3way else None,
                'away_odds': odds[2] if is_3way else odds[1],
                'is_live': 'live' in url,
                'market': '1x2',
                'total_over': totals_over,
                'total_under': totals_under,
                'source_url': url,
                'scraped_at': time.time()
            }
            
            if event['home_odds'] >= 1.01:
                result.append(event)
        
        return result


async def test():
    logging.basicConfig(level=logging.INFO)
    parser = ZenitPlaywrightParser()
    events = await parser.get_events()
    print(f'Found {len(events)} events from Zenit')
    for e in events[:5]:
        print(f"  {e['home_team']} vs {e['away_team']}: {e['home_odds']} - {e['draw_odds']} - {e['away_odds']}")

if __name__ == '__main__':
    asyncio.run(test())
