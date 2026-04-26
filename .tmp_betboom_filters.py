import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
FILTERS=['1н','1д','Все']
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(30000)
        out=[]
        for flt in FILTERS:
            try:
                await page.get_by_text(flt, exact=True).first.click(timeout=5000)
                await page.wait_for_timeout(7000)
                text=await page.evaluate("(document.body && document.body.innerText) || ''")
                out.append({'filter':flt,'sample':text[:1800]})
            except Exception as e:
                out.append({'filter':flt,'error':str(e)})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
