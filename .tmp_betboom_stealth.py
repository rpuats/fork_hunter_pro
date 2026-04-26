import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

URL = 'https://betboom.ru/sport/football'

async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await create_stealth_context(browser, generate_stealth_config())
        page = await context.new_page()
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(15000)
        data = await page.evaluate("""() => ({ href: location.href, bodyLen: ((document.body && document.body.innerText) || '').length, bbNm: document.querySelectorAll('.bb-Nm').length, bbRm: document.querySelectorAll('.bb-Rm').length, bbKG: document.querySelectorAll('.bb-KG').length, iframeCount: document.querySelectorAll('iframe').length, sample: ((document.body && document.body.innerText) || '').slice(0,600) })""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
