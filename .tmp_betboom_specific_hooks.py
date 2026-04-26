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
          while (fiber) {
            const type = typeof fiber.type === 'string' ? fiber.type : (fiber.type?.name || fiber.elementType?.name || fiber.type?.displayName || fiber.elementType?.displayName || null);
            if (type === 'eO') break;
            fiber = fiber.return;
          }
          if (!fiber) return { found:false };
          let hook = fiber.memoizedState;
          let i = 0;
          const picks = {};
          while (hook && i < 20) {
            if (i === 4 || i === 11 || i === 18) {
              const memo = hook.memoizedState;
              picks[i] = Array.isArray(memo) ? memo.map(v => {
                if (v === null || v === undefined || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') return v;
                if (Array.isArray(v)) return { kind:'array', len:v.length };
                if (typeof v === 'object') return { kind:'object', keys:Object.keys(v).slice(0,20), bools:Object.fromEntries(Object.entries(v).filter(([k,val])=>typeof val==='boolean').slice(0,10)) };
                return { kind: typeof v };
              }) : memo;
            }
            hook = hook.next;
            i += 1;
          }
          return picks;
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
