import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(30000)
        await page.get_by_text('1н', exact=True).first.click(timeout=5000)
        await page.wait_for_timeout(5000)
        await page.get_by_text('Футбол', exact=True).first.click(timeout=5000)
        await page.wait_for_timeout(5000)
        data=await page.evaluate("() => { const txt=((document.body&&document.body.innerText)||''); const lines=txt.split(/\\n/).map(x=>x.trim()).filter(Boolean); const idx=lines.indexOf('Футбол'); return idx>=0 ? lines.slice(idx, idx+220) : lines.slice(0,220); }")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
