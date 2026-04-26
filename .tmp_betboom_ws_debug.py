import asyncio, json, sys
from playwright.async_api import async_playwright

URL = 'https://betboom.ru/sport/football'

async def main():
    seen = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width': 1920, 'height': 1080}, locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page = await context.new_page()

        def on_websocket(ws):
            item = {'url': ws.url, 'frames': []}
            seen.append(item)
            ws.on('framesent', lambda payload: item['frames'].append({'dir':'out','payload': payload[:500] if isinstance(payload, str) else str(payload)[:500]}))
            ws.on('framereceived', lambda payload: item['frames'].append({'dir':'in','payload': payload[:500] if isinstance(payload, str) else str(payload)[:500]}))

        page.on('websocket', on_websocket)
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(15000)
        await browser.close()
    sys.stdout.buffer.write(json.dumps(seen, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
