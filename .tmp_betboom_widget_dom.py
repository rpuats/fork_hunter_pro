import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        data = await page.evaluate("""() => {
          const nodes = Array.from(document.querySelectorAll('*'))
            .filter(n => {
              const id = n.id || '';
              const cls = typeof n.className === 'string' ? n.className : '';
              return /sport|widget|iframe|matchpromo|sportbook/i.test(id + ' ' + cls);
            })
            .slice(0, 100)
            .map(n => ({ tag: n.tagName, id: n.id || '', cls: typeof n.className === 'string' ? n.className : '', text: (n.textContent || '').slice(0,120) }));
          const globals = Object.keys(window).filter(k => /sport|widget|betboom|matchpromo/i.test(k)).slice(0, 100);
          return {
            href: location.href,
            title: document.title,
            bodyLen: ((document.body && document.body.innerText) || '').length,
            nodes,
            globals,
            scripts: Array.from(document.scripts).map(s => s.src).filter(Boolean).filter(s => /sport|widget|betboom|matchpromo/i.test(s)).slice(0, 50),
          };
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
