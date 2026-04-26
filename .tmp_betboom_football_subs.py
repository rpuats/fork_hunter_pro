import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from playwright.async_api import async_playwright
from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser
URL='https://betboom.ru/sport'
SUBS=['Россия','Англия','Испания','Италия','Франция','Бразилия']
async def main():
    p=BetBoomPlaywrightParser()
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await p._accept_cookie_if_present(page)
        await p._wait_for_compact_markers(page)
        await page.get_by_text('1н', exact=True).first.click(timeout=5000)
        await asyncio.sleep(4)
        await page.get_by_text('Футбол', exact=True).first.click(timeout=5000)
        await asyncio.sleep(4)
        out=[]
        for sub in SUBS:
            try:
                await page.get_by_text(sub, exact=True).first.click(timeout=5000)
                await asyncio.sleep(5)
                seen=set()
                for _ in range(8):
                    events=await p._extract_from_text(page, URL, 'Футбол')
                    for e in events:
                        seen.add((e.get('home_team'), e.get('away_team')))
                    await page.mouse.wheel(0, 2200)
                    await asyncio.sleep(1.5)
                out.append({'sub':sub,'unique':len(seen)})
            except Exception as e:
                out.append({'sub':sub,'error':str(e)})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
