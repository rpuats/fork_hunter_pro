import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from playwright.async_api import async_playwright
from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser

URL = 'https://betboom.ru/sport'
TARGETS = ['Теннис','Настольный теннис','Футбол','Бейсбол']

async def main():
    p = BetBoomPlaywrightParser()
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page = await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await p._accept_cookie_if_present(page)
        await p._wait_for_compact_markers(page)
        try:
            await page.get_by_text('Все', exact=True).first.click(timeout=5000)
            await asyncio.sleep(4)
        except Exception:
            pass
        out=[]
        for cat in TARGETS:
            try:
                await page.get_by_text(cat, exact=True).first.click(timeout=5000)
            except Exception as e:
                out.append({'category': cat, 'error': str(e)})
                continue
            await asyncio.sleep(4)
            seen=set()
            steps=[]
            for step in range(16):
                events = await p._extract_from_text(page, URL, cat)
                for e in events:
                    seen.add((e.get('home_team'), e.get('away_team')))
                steps.append({'step': step, 'now': len(events), 'unique': len(seen)})
                await page.mouse.wheel(0, 2200)
                await asyncio.sleep(1.5)
            out.append({'category': cat, 'steps': steps})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
