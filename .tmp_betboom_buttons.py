import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page=await context.new_page()
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(5000)
        data=await page.evaluate("""() => ({ buttons: Array.from(document.querySelectorAll('button')).map(b => ({text:(b.innerText||'').trim(), cls:b.className})).filter(x => x.text).slice(0,30), links: Array.from(document.querySelectorAll('a')).map(a => ({text:(a.innerText||'').trim(), href:a.href})).filter(x => x.text).slice(0,20) })""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
