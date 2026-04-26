import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport/live'
CATS=['Футбол','Теннис','Баскетбол','Хоккей']
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(32000)
        out=[]
        for cat in CATS:
            clicked=False
            try:
                await page.get_by_text(cat, exact=True).first.click(timeout=5000)
                clicked=True
            except Exception as e:
                out.append({'cat':cat,'clicked':False,'error':str(e)})
                continue
            await page.wait_for_timeout(6000)
            text=await page.evaluate("(document.body && document.body.innerText) || ''")
            out.append({'cat':cat,'clicked':clicked,'sample':text[:1500]})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
