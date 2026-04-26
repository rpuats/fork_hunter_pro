import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
async def main():
    failed = []
    console = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page.on('requestfailed', lambda req: failed.append({'url': req.url, 'method': req.method, 'resource': req.resource_type, 'failure': req.failure}))
        page.on('console', lambda msg: console.append({'type': msg.type, 'text': msg.text}))
        page.on('pageerror', lambda exc: console.append({'type':'pageerror','text':str(exc)}))
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(15000)
        data = await page.evaluate("""() => ({ hasRegister: !!globalThis.SportbookWidgetRegister, scripts: Array.from(document.scripts).map(s => ({src:s.src, type:s.type || ''})).filter(s => s.src.includes('sporthub') || s.src.includes('_next/static/chunks/pages/sport')).slice(0,20) })""")
        await browser.close()
    sys.stdout.buffer.write(json.dumps({'failed': failed[:50], 'console': console[:100], 'data': data}, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
