import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
ROUTES = [
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/recommendations/matches/get', {}),
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_sport_id', {'sport_id': 1}),
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_sport_id', {'sportId': 1}),
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_category_id', {'category_id': 1}),
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_category_id', {'categoryId': 1}),
    ('https://siteapi.betboom.ru/api/site_api/v1/sporthub/market_tooltip/tooltip/get_by_sport_id', {'sport_id': 1}),
]
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        out=[]
        for route, body in ROUTES:
            result = await page.evaluate("""async ({route, body}) => {
                try {
                    const resp = await fetch(route, {
                        method: 'POST',
                        credentials: 'include',
                        headers: {
                            'content-type': 'application/json;charset=UTF-8',
                            'accept': 'application/json, text/plain, */*',
                            'x-platform': 'web'
                        },
                        body: JSON.stringify(body),
                    });
                    const text = await resp.text();
                    return {ok:true,status:resp.status,text:text.slice(0,500)};
                } catch (e) {
                    return {ok:false,error:String(e)};
                }
            }""", {'route': route, 'body': body})
            out.append({'route': route, 'body': body, 'result': result})
        await browser.close()
        sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
