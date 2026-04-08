"""
OlimpBet API Finder - opens browser, captures ALL network requests
"""
import asyncio
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=False, args=['--no-sandbox'])
        context = await browser.new_context(
            viewport={'width': 1920, 'height': 1080},
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            locale='ru-RU',
        )
        page = await context.new_page()
        
        api_responses = []
        
        async def on_response(response):
            url = response.url
            if response.status == 200 and ('api' in url.lower() or 'feed' in url.lower() or 'event' in url.lower() or 'json' in url.lower()):
                content_type = response.headers.get('content-type', '')
                if 'json' in content_type:
                    try:
                        body = await response.text()
                        if len(body) > 500:
                            data = json.loads(body)
                            api_responses.append({'url': url, 'keys': list(data.keys())[:10] if isinstance(data, dict) else f'list[{len(data)}]'})
                            print(f"[API] {url[:100]} keys={list(data.keys())[:8] if isinstance(data, dict) else 'list'}")
                    except:
                        pass
        
        page.on('response', on_response)
        
        print("Opening OlimpBet...")
        await page.goto('https://www.olimp.bet/live/football', wait_until='domcontentloaded', timeout=30000)
        print("Waiting 15s for API calls...")
        for i in range(15):
            await asyncio.sleep(1)
            if i % 5 == 0:
                await page.mouse.move(200 + i*30, 300 + i*20)
        
        print(f"\nCaptured {len(api_responses)} API responses:")
        for r in api_responses:
            print(f"  {r['url'][:100]} -> {r['keys']}")
        
        await browser.close()

asyncio.run(main())
