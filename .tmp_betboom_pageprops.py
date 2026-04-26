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
          const next = JSON.parse(document.getElementById('__NEXT_DATA__').textContent);
          return {
            pageProps: next.props.pageProps,
            runtimeConfig: {
              SPORTBOOK_API_URL: next.runtimeConfig.SPORTBOOK_API_URL,
              SPORTBOOK_FEED_WS_URL: next.runtimeConfig.SPORTBOOK_FEED_WS_URL,
              SPORTBOOK_MARKET_BET_STATS_WS_URL: next.runtimeConfig.SPORTBOOK_MARKET_BET_STATS_WS_URL,
              SPORTBOOK_BETS_HISTORY_WS_URL: next.runtimeConfig.SPORTBOOK_BETS_HISTORY_WS_URL,
            }
          };
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
