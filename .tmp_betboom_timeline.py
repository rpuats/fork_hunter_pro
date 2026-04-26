import asyncio, json, sys
from playwright.async_api import async_playwright

URL = 'https://betboom.ru/sport/football'
CHECKS = [2000, 5000, 10000, 15000, 25000, 35000]

async def main():
    out = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width': 1920, 'height': 1080}, locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page = await context.new_page()
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        elapsed = 0
        for target in CHECKS:
            await page.wait_for_timeout(target - elapsed)
            elapsed = target
            data = await page.evaluate("""() => ({ href: location.href, bodyLen: ((document.body && document.body.innerText) || '').length, bbNm: document.querySelectorAll('.bb-Nm').length, bbRm: document.querySelectorAll('.bb-Rm').length, iframeCount: document.querySelectorAll('iframe').length, scriptCount: document.scripts.length, htmlLen: document.documentElement.outerHTML.length })""")
            data['t_ms'] = elapsed
            out.append(data)
        await browser.close()
    sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
