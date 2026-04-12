"""API Hunter v3 - FINAL VERSION - finds ALL API endpoints with events"""
import asyncio
import json
import sys
import os
import time
import re

async def main():
    from playwright.async_api import async_playwright
    
    bk_name = sys.argv[1] if len(sys.argv) > 1 else "winline"
    
    BK_URLS = {
        "winline": [
            "https://winline.ru/football",
            "https://winline.ru/live/football",
            "https://winline.ru/basketball",
        ],
        "zenit": [
            "https://zenit.win/line/football",
            "https://zenit.win/live/football",
        ],
        "betcity": [
            "https://betcity.ru/ru/line/football",
        ],
        "baltbet": [
            "https://baltbet.ru/line",
        ],
    }
    
    urls = BK_URLS.get(bk_name, BK_URLS["winline"])
    print(f"API Hunter v3 - {bk_name}")
    print(f"Pages to scan: {len(urls)}")
    
    all_apis = []
    all_events = []
    seen_keys = set()
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=['--no-sandbox', '--disable-dev-shm-usage']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        
        for url_idx, start_url in enumerate(urls):
            print(f"\n[{url_idx+1}/{len(urls)}] Loading: {start_url}")
            
            page = await context.new_page()
            
            # Intercept ALL responses
            async def on_response(response):
                try:
                    url = response.url
                    status = response.status
                    
                    # Only successful responses
                    if status != 200:
                        return
                    
                    ct = response.headers.get('content-type', '').lower()
                    
                    # Only JSON
                    if 'json' not in ct:
                        return
                    
                    # Skip analytics/tracking
                    if any(skip in url.lower() for skip in [
                        'yandex', 'google', 'analytics', 'metric', 'telemetry',
                        'cdn.', 'static.', 'assets/', 'icons', 'promo',
                        'loyalty', 'bonus', 'settings/desktop'
                    ]):
                        return
                    
                    body = await response.json()
                    size = len(json.dumps(body, ensure_ascii=False))
                    
                    # Check for events data
                    has_events = False
                    events_data = None
                    event_count = 0
                    
                    # Strategy 1: Check common event keys
                    if isinstance(body, dict):
                        for key in ['e', 'events', 'data', 'items', 'matches', 'lines', 't', 'm', 'b']:
                            if key in body and isinstance(body[key], list) and len(body[key]) > 5:
                                has_events = True
                                events_data = body[key]
                                event_count = len(body[key])
                                break
                    
                    # Strategy 2: Check if it's an array
                    if not has_events and isinstance(body, list) and len(body) > 10:
                        has_events = True
                        events_data = body
                        event_count = len(body)
                    
                    # Strategy 3: Look for odds-like values
                    if not has_events and size > 5000:
                        text = json.dumps(body)
                        odds = re.findall(r'"(?:odds|coef|k|o|odd)"\s*:\s*([\d.]+)', text)
                        if len(odds) > 15:
                            has_events = True
                    
                    if has_events:
                        key = url.split('?')[0]
                        if key not in seen_keys:
                            seen_keys.add(key)
                            
                            api_info = {
                                'url': url,
                                'size': size,
                                'event_count': event_count,
                                'sample_keys': list(body.keys())[:10] if isinstance(body, dict) else f'Array[{len(body)}]',
                            }
                            all_apis.append(api_info)
                            print(f"  [API FOUND] {url[:90]}")
                            print(f"    Size: {size:,}b, Events: {event_count}")
                            print(f"    Keys: {api_info['sample_keys']}")
                            
                            # Save events if we found them
                            if events_data and len(events_data) > 0:
                                all_events.extend(events_data[:100])  # Save sample
                                
                except Exception as e:
                    pass
            
            page.on('response', on_response)
            
            # Load page
            try:
                await page.goto(start_url, wait_until='networkidle', timeout=20000)
                await page.wait_for_timeout(3000)
                
                # Scroll to trigger lazy loading
                for scroll_pos in [0.33, 0.5, 0.75, 1.0]:
                    await page.evaluate(f'window.scrollTo(0, document.body.scrollHeight * {scroll_pos})')
                    await page.wait_for_timeout(1500)
                
                # Click on sport tabs if they exist
                await page.evaluate('''
                    () => {
                        const tabs = document.querySelectorAll('a, button, [role="tab"], [class*="tab"], [class*="sport"]');
                        for (let i = 0; i < Math.min(5, tabs.length); i++) {
                            try { tabs[i].click(); } catch(e) {}
                        }
                    }
                ''')
                await page.wait_for_timeout(3000)
                
                # Scroll again
                await page.evaluate('window.scrollTo(0, document.body.scrollHeight)')
                await page.wait_for_timeout(2000)
                
            except Exception as e:
                print(f"  Error loading page: {str(e)[:100]}")
            
            await page.close()
        
        await browser.close()
    
    # Results
    print(f"\n{'='*60}")
    print(f"RESULTS for {bk_name}")
    print(f"{'='*60}")
    print(f"APIs found: {len(all_apis)}")
    print(f"Events sampled: {len(all_events)}")
    
    if all_apis:
        print(f"\nAPI Endpoints:")
        for i, api in enumerate(all_apis):
            print(f"  {i+1}. {api['url'][:90]}")
            print(f"     Size: {api['size']:,}b, Events: {api['event_count']}")
            print(f"     Keys: {api['sample_keys']}")
    
    # Save results
    output = {
        'bk': bk_name,
        'apis': all_apis,
        'events_sample': all_events[:50],
        'api_count': len(all_apis),
        'event_count': len(all_events),
        'timestamp': time.time()
    }
    
    output_file = f'{bk_name}_api_v3.json'
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, indent=2, default=str)
    
    print(f"\nSaved to {output_file}")

if __name__ == "__main__":
    asyncio.run(main())
