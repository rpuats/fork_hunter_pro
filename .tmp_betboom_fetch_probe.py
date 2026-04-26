import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
TESTS = [
  ['https://betboom.ru/api/games/get_game_kinds', {}],
  ['https://siteapi.betboom.ru/api/site_api/v1/sporthub/recommendations/matches/get', {}],
  ['https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_sport_id', {'sport_id': 1}],
  ['https://siteapi.betboom.ru/api/site_api/v1/sporthub/match_result_statistics/tournaments/get_by_sport_id', {'sportId': 1}],
  ['https://siteapi.betboom.ru/api/site_api/v1/sporthub/market_tooltip/tooltip/get_by_sport_id', {'sport_id': 1}],
]
async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(12000)
        results = []
        for url, body in TESTS:
            data = await page.evaluate("""async ({url, body}) => {
                try {
                    const resp = await fetch(url, {
                        method: 'POST',
                        credentials: 'include',
                        headers: {
                            'content-type': 'application/json;charset=UTF-8',
                            'x-platform': 'web',
                            'accept': 'application/json, text/plain, */*'
                        },
                        body: JSON.stringify(body),
                    });
                    const text = await resp.text();
                    return { ok: true, status: resp.status, text: text.slice(0, 400) };
                } catch (e) {
                    return { ok: false, error: String(e) };
                }
            }""", {'url': url, 'body': body})
            results.append({'url': url, 'body': body, 'result': data})
        cookies = await context.cookies()
        await browser.close()
        sys.stdout.buffer.write(json.dumps({'results': results, 'cookies': [c['name'] for c in cookies]}, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
