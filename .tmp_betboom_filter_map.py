import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from playwright.async_api import async_playwright
from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser
URL='https://betboom.ru/sport'
TESTS=[('Все','Теннис'),('1н','Теннис'),('Все','Футбол'),('1н','Футбол')]
async def main():
    p=BetBoomPlaywrightParser()
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await p._goto_with_retry(page, URL)
        await p._accept_cookie_if_present(page)
        await p._wait_for_compact_markers(page)
        out=[]
        for flt,cat in TESTS:
            await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
            await p._accept_cookie_if_present(page)
            await p._wait_for_compact_markers(page)
            await p._click_visible_text(page, flt)
            await asyncio.sleep(4)
            await p._click_visible_text(page, cat)
            await asyncio.sleep(4)
            seen=set()
            for _ in range(10):
                ev=await p._extract_from_text(page, URL, cat)
                for e in ev:
                    seen.add((e.get('home_team'), e.get('away_team')))
                await page.mouse.wheel(0,2200)
                await asyncio.sleep(1.5)
            out.append({'filter':flt,'category':cat,'unique':len(seen)})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
