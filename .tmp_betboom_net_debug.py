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
                ct = response.headers.get('content-type','')
                url = response.url
                if response.status == 200 and ('json' in ct or 'javascript' in ct or 'html' in ct):
                    text = await response.text()
                    if len(text) > 50:
                        hits.append({
                            'url': url,
                            'status': response.status,
                            'ct': ct,
                            'len': len(text),
                            'preview': text[:300]
                        })
            except Exception:
                pass

        page.on('response', on_response)
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        await browser.close()
    uniq = []
    seen = set()
    for hit in hits:
        if hit['url'] in seen:
            continue
        seen.add(hit['url'])
        uniq.append(hit)
    sys.stdout.buffer.write(json.dumps(uniq[:80], ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
