import asyncio, sys, json
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from playwright.async_api import async_playwright

STEALTH_JS = """
Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
window.chrome = {runtime: {}};
Object.defineProperty(navigator, 'languages', {get: () => ['ru-RU', 'ru', 'en']});
Object.defineProperty(navigator, 'plugins', {get: () => [1, 2, 3, 4, 5]});
"""

async def s():
    pw = await async_playwright().start()
    browser = await pw.chromium.launch(
        headless=True,
        args=[
            '--disable-blink-features=AutomationControlled',
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--window-size=1920,1080',
        ]
    )
    ctx = await browser.new_context(
        user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
        viewport={'width': 1920, 'height': 1080},
        locale='ru-RU',
        timezone_id='Europe/Moscow',
    )
    page = await ctx.new_page()
    await page.add_init_script(STEALTH_JS)

    api_data = {}
    async def handle_resp(resp):
        if 'events/list' in resp.url and resp.status == 200:
            try:
                body = await resp.text()
                api_data['events_list'] = json.loads(body)
            except Exception:
                pass
    page.on('response', handle_resp)

    print('Navigating to pari.ru/live/football ...')
    try:
        await page.goto('https://pari.ru/live/football', wait_until='domcontentloaded', timeout=30000)
        print(f'Page loaded: {page.url}')
        await asyncio.sleep(5)
        if 'events_list' in api_data:
            events = api_data['events_list'].get('events', [])
            print(f'API events captured: {len(events)}')
        else:
            print('No API data captured')
            title = await page.title()
            print(f'Page title: {title}')
            content = await page.content()
            print(f'Content length: {len(content)}')
            print(f'Content snippet: {content[:500]}')
    except Exception as e:
        print(f'Navigation error: {type(e).__name__}: {e}')
    await browser.close()
    await pw.stop()

asyncio.run(s())
