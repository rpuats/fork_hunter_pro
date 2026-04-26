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
          let fiber = key ? node[key] : null;
          const chain = [];
          let depth = 0;
          while (fiber && depth < 12) {
            const type = typeof fiber.type === 'string' ? fiber.type : (fiber.type?.name || fiber.elementType?.name || fiber.type?.displayName || fiber.elementType?.displayName || null);
            chain.push({
              depth,
              type,
              tag: fiber.tag,
              pendingPropsKeys: fiber.pendingProps && typeof fiber.pendingProps === 'object' ? Object.keys(fiber.pendingProps).slice(0,20) : null,
              memoizedPropsKeys: fiber.memoizedProps && typeof fiber.memoizedProps === 'object' ? Object.keys(fiber.memoizedProps).slice(0,20) : null,
              stateNodeType: fiber.stateNode ? (fiber.stateNode.nodeType ? 'dom' : typeof fiber.stateNode) : null
            });
            fiber = fiber.return;
            depth += 1;
          }
          return chain;
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
