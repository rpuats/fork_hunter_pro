import asyncio, json, sys, os
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
async def main():
    reqs=[]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page.on('request', lambda req: reqs.append({'url': req.url, 'method': req.method, 'resource': req.resource_type, 'post': req.post_data}))
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(30000)
        for label in ['1н','Футбол','Россия','Англия. Премьер-лига']:
            try:
                await page.evaluate("""(label) => {
                  const norm = (v) => String(v || '').replace(/\u00a0/g,' ').replace(/\s+/g,' ').trim();
                  const nodes = Array.from(document.querySelectorAll('button,a,div,span'));
                  const target = nodes.find(n => norm(n.textContent || '') === label && n.getBoundingClientRect().width > 0 && n.getBoundingClientRect().height > 0);
                  if (target) target.click();
                }""", label)
            except Exception:
                pass
            await page.wait_for_timeout(6000)
        await browser.close()
    filtered=[]
    for r in reqs:
        u=r['url']
        if any(x in u for x in ['siteapi','sporthub','/api/games/','tree_ws','market_betstats','bets_history']):
            filtered.append(r)
    sys.stdout.buffer.write(json.dumps(filtered, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
