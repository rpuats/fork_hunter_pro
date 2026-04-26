import asyncio, json, sys
from playwright.async_api import async_playwright

URL = 'https://betboom.ru/sport/football'

async def main():
    hits = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width': 1920, 'height': 1080}, locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page = await context.new_page()

        async def on_response(response):
            try:
                if '/api/games/' in response.url:
                    req = response.request
                    hits.append({
                        'url': response.url,
                        'status': response.status,
                        'method': req.method,
                        'headers': req.headers,
                        'post_data': req.post_data,
                    })
            except Exception:
                pass

        page.on('response', on_response)
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        await browser.close()
    sys.stdout.buffer.write(json.dumps(hits, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
