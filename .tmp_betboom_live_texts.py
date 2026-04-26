import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/live'
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(32000)
        data=await page.evaluate("""() => {
          const visible = (node) => !!(node && (node.offsetWidth || node.offsetHeight || node.getClientRects().length));
          return Array.from(document.querySelectorAll('.bb-Nm')).slice(0,20).map((container) => {
            const parent = container.parentElement;
            const grandparent = parent?.parentElement;
            const great = grandparent?.parentElement;
            return {
              text: ((great || grandparent || parent || container)?.textContent || '').trim(),
              odds: Array.from(container.querySelectorAll('.bb-Rm')).map(n => (n.textContent||'').trim()),
              parent_classes: parent?.className || '',
              grandparent_classes: grandparent?.className || '',
              visible: visible(container),
            };
          });
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
