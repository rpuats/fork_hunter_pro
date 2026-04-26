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
          if (!fiber) return { found: false };
          const hooks = [];
          let hook = fiber.memoizedState;
          let i = 0;
          const summarize = (v) => {
            if (v === null || v === undefined) return v;
            if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') return v;
            if (Array.isArray(v)) return { kind: 'array', len: v.length };
            if (typeof v === 'object') {
              if (v.current !== undefined) return { kind: 'ref', currentNull: v.current === null, currentType: typeof v.current };
              const out = { kind: 'object' };
              for (const k of Object.keys(v).slice(0,10)) {
                const item = v[k];
                if (item === null || item === undefined || typeof item === 'string' || typeof item === 'number' || typeof item === 'boolean') out[k] = item;
                else if (Array.isArray(item)) out[k] = { kind: 'array', len: item.length };
                else out[k] = { kind: typeof item };
              }
              return out;
            }
            return { kind: typeof v };
          };
          while (hook && i < 20) {
            hooks.push({ index: i, memoizedState: summarize(hook.memoizedState), baseState: summarize(hook.baseState), hasQueue: !!hook.queue });
            hook = hook.next;
            i += 1;
          }
          return { found: true, hooks };
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
