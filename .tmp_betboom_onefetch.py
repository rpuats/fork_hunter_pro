import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
ROUTE='https://siteapi.betboom.ru/api/site_api/v1/sporthub/recommendations/matches/get'
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(8000)
        cookies = await context.cookies(['https://betboom.ru','https://siteapi.betboom.ru'])
        result = await page.evaluate("""async (route) => {
            try {
                const ctrl = new AbortController();
                setTimeout(() => ctrl.abort('timeout'), 12000);
                const resp = await fetch(route, {
                    method: 'POST',
                    credentials: 'include',
                    headers: {
                        'content-type': 'application/json;charset=UTF-8',
                        'accept': 'application/json, text/plain, */*',
                        'x-platform': 'web'
                    },
                    body: '{}',
                    signal: ctrl.signal,
                });
                const text = await resp.text();
                return {ok:true,status:resp.status,text:text.slice(0,500)};
            } catch (e) {
                return {ok:false,error:String(e)};
            }
        }""", ROUTE)
        await browser.close()
        sys.stdout.buffer.write(json.dumps({'cookies':[c['name'] for c in cookies], 'result': result}, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
