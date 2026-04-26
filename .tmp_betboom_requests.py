import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
async def main():
    requests = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page.on('request', lambda req: requests.append({'url': req.url, 'method': req.method, 'resource': req.resource_type}))
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        await browser.close()
    filtered = [r for r in requests if 'sporthub' in r['url'] or 'siteapi' in r['url'] or '/api/games/' in r['url'] or 'widget.js' in r['url']]
    sys.stdout.buffer.write(json.dumps(filtered, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
