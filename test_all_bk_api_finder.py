"""
Universal API Finder for all BK
Opens each bookmaker in browser, captures ALL JSON API responses.
"""
import asyncio
import json
import sys
import os
sys.stdout.reconfigure(encoding='utf-8')
from playwright.async_api import async_playwright

BOOKMAKERS = [
    {"name": "Winline", "url": "https://winline.ru/live/football"},
    {"name": "Pari", "url": "https://pari.ru/live/football"},
    {"name": "Zenit", "url": "https://zenit.ru/live/football"},
    {"name": "Marathon", "url": "https://marathonbet.com/ru/live/Футбол"},
    {"name": "Betcity", "url": "https://betcity.ru/live/football"},
    {"name": "Baltbet", "url": "https://baltbet.com/live/football"},
    {"name": "Bettery", "url": "https://bettery.ru/live/football"},
    {"name": "BetBoom", "url": "https://betboom.ru/live/football"},
    {"name": "Fonbet", "url": "https://fonbet.ru/live/football"},
]

OUTPUT_DIR = "network_capture"
os.makedirs(OUTPUT_DIR, exist_ok=True)

async def capture_bk(bk):
    name = bk["name"]
    url = bk["url"]
    print(f"\n{'='*60}")
    print(f"CAPTURING: {name}")
    print(f"URL: {url}")
    print(f"{'='*60}")
    
    api_responses = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        context = await browser.new_context(
            viewport={'width': 1920, 'height': 1080},
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            locale='ru-RU',
        )
        page = await context.new_page()
        
        async def on_response(response):
            url_resp = response.url
            if response.status == 200:
                content_type = response.headers.get('content-type', '')
                if 'json' in content_type or 'api' in url_resp.lower() or 'feed' in url_resp.lower():
                    try:
                        body = await response.text()
                        if len(body) > 200:
                            data = json.loads(body)
                            keys_info = list(data.keys())[:8] if isinstance(data, dict) else f'list[{len(data)}]'
                            api_responses.append({
                                'url': url_resp,
                                'keys': keys_info,
                                'size': len(body)
                            })
                            print(f"  [API] {url_resp[:90]} -> {keys_info} ({len(body)}b)")
                    except:
                        pass
        
        page.on('response', on_response)
        
        try:
            print(f"  Opening {url}...")
            await page.goto(url, wait_until='domcontentloaded', timeout=20000)
            print(f"  Waiting 10s for API calls...")
            for i in range(10):
                await asyncio.sleep(1)
                if i % 3 == 0:
                    await page.mouse.move(200 + i*50, 300 + i*30)
        except Exception as e:
            print(f"  Error: {e}")
        
        await browser.close()
    
    # Save results
    safe_name = name.lower().replace(' ', '_')
    output_file = os.path.join(OUTPUT_DIR, f"{safe_name}_api.json")
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(api_responses, f, ensure_ascii=False, indent=2)
    
    print(f"\n  Captured {len(api_responses)} API endpoints")
    print(f"  Saved to: {output_file}")
    return api_responses

async def main():
    print("="*60)
    print("UNIVERSAL BK API FINDER")
    print("="*60)
    print(f"Will scan {len(BOOKMAKERS)} bookmakers...")
    
    all_results = {}
    for bk in BOOKMAKERS:
        try:
            results = await capture_bk(bk)
            all_results[bk["name"]] = len(results)
            await asyncio.sleep(2)  # pause between BKs
        except Exception as e:
            print(f"  Failed: {e}")
            all_results[bk["name"]] = 0
    
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    for name, count in all_results.items():
        status = "✅" if count > 0 else "❌"
        print(f"  {status} {name}: {count} API endpoints")

asyncio.run(main())
