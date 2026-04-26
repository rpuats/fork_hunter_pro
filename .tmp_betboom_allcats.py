import asyncio, json, sys
from playwright.async_api import async_playwright

URLS = ['https://betboom.ru/sport/live', 'https://betboom.ru/sport']

async def extract(page, url):
    await page.goto(url, wait_until='domcontentloaded', timeout=30000)
    await page.wait_for_timeout(30000)
    return await page.evaluate("""() => {
      const text = (document.body && document.body.innerText) || '';
      const lines = text.split('\n').map(x => x.trim()).filter(Boolean);
      const out = [];
      for (let i = 0; i < lines.length - 1; i++) {
        const label = lines[i];
        const count = lines[i + 1];
        if (/^\d+$/.test(count) && label.length > 1 && label.length < 60) {
          out.push({label, count: parseInt(count, 10)});
        }
      }
      return out;
    }""")

async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        data = {}
        for url in URLS:
          data[url] = await extract(page, url)
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
