import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        data = await page.evaluate("""() => {
          const keys = Object.keys(window).filter(k => /effector|store|sport|route|widget|coupon/i.test(k)).slice(0,200);
          const sportApp = document.getElementById('sportApp');
          const reactKey = sportApp ? Object.keys(sportApp).find(k => k.startsWith('__reactFiber$') || k.startsWith('__reactProps$')) : null;
          return {
            keys,
            sportAppReactKey: reactKey,
            sportAppReactValueType: reactKey && sportApp ? typeof sportApp[reactKey] : null,
            hasNextData: !!document.getElementById('__NEXT_DATA__'),
            nextDataLen: (document.getElementById('__NEXT_DATA__')?.textContent || '').length,
          };
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
