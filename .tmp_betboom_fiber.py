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
          const node = document.getElementById('sportApp');
          const key = node ? Object.keys(node).find(k => k.startsWith('__reactFiber$')) : null;
          const fiber = key ? node[key] : null;
          function summarize(f, depth=0){
            if(!f || depth > 6) return null;
            const type = typeof f.type === 'string' ? f.type : (f.type?.name || f.elementType?.name || f.type?.displayName || f.elementType?.displayName || null);
            return {
              depth,
              type,
              tag: f.tag,
              pendingPropsKeys: f.pendingProps && typeof f.pendingProps === 'object' ? Object.keys(f.pendingProps).slice(0,20) : null,
              memoizedPropsKeys: f.memoizedProps && typeof f.memoizedProps === 'object' ? Object.keys(f.memoizedProps).slice(0,20) : null,
              child: summarize(f.child, depth+1),
              sibling: null
            };
          }
          return summarize(fiber);
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
