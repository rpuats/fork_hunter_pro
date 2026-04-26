import asyncio, json, sys
from playwright.async_api import async_playwright

URL = 'https://betboom.ru/sport/football'

async def main():
    out = []
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width': 1920, 'height': 1080}, locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page = await context.new_page()
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        frames = page.frames
        for idx, frame in enumerate(frames):
            try:
                data = await frame.evaluate("""() => ({ href: location.href, title: document.title, bodyLen: ((document.body && document.body.innerText) || '').length, bbNm: document.querySelectorAll('.bb-Nm').length, bbRm: document.querySelectorAll('.bb-Rm').length, bbKG: document.querySelectorAll('.bb-KG').length, bodySample: ((document.body && document.body.innerText) || '').slice(0,300) })""")
            except Exception as e:
                data = {'error': str(e)}
            out.append({'frame_index': idx, 'frame_url': frame.url, 'data': data})
        await browser.close()
    sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
