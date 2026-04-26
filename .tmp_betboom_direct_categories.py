import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from playwright.async_api import async_playwright
from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser
URLS = [
    ('https://betboom.ru/sport/tennis', 'Теннис'),
    ('https://betboom.ru/sport/table-tennis', 'Настольный теннис'),
    ('https://betboom.ru/sport/football', 'Футбол'),
    ('https://betboom.ru/sport/baseball', 'Бейсбол'),
]
async def main():
    p=BetBoomPlaywrightParser()
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        out=[]
        for url, hint in URLS:
            try:
                await p._goto_with_retry(page, url)
                await p._accept_cookie_if_present(page)
                await p._wait_for_compact_markers(page)
                seen=set()
                for _ in range(20):
                    ev=await p._extract_from_text(page, url, hint)
                    for e in ev:
                        seen.add((e.get('home_team'), e.get('away_team')))
                    await page.mouse.wheel(0, 2400)
                    await asyncio.sleep(1.5)
                out.append({'url':url,'hint':hint,'unique':len(seen)})
            except Exception as e:
                out.append({'url':url,'hint':hint,'error':str(e)})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
