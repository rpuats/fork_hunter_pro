#!/usr/bin/env python3
"""
Полный анализ сетевых запросов Winline
Сохраняем ВСЕ запросы и ответы для анализа
"""

import asyncio
import json
from playwright.async_api import async_playwright
from datetime import datetime

async def main():
    all_requests = []
    all_responses = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=["--disable-blink-features=AutomationControlled"]
        )
        context = await browser.new_context(
            user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        )
        page = await context.new_page()
        
        # Перехватываем ВСЕ запросы
        async def on_request(request):
            req_data = {
                'url': request.url,
                'method': request.method,
                'headers': dict(request.headers),
            }
            all_requests.append(req_data)
            print(f"📤 REQ: {request.method} {request.url[:100]}")
        
        # Перехватываем ВСЕ ответы
        async def on_response(response):
            try:
                url = response.url
                status = response.status
                
                # Проверяем что это не изображение/стиль
                content_type = response.headers.get('content-type', '')
                is_data = any(x in content_type for x in ['json', 'text', 'javascript', 'application']) or 'api' in url
                
                body = ""
                try:
                    body = await response.text()
                except:
                    body = f"<binary data {len(await response.body())} bytes>"
                
                resp_data = {
                    'url': url,
                    'status': status,
                    'content_type': content_type,
                    'size': len(body),
                    'has_data': len(body) > 50 and is_data,
                }
                
                if is_data and len(body) > 50:
                    resp_data['preview'] = body[:500]
                
                all_responses.append(resp_data)
                
                if is_data and len(body) > 100:
                    print(f"📥 RESP: {status} {url[:80]}")
                    if len(body) > 10000:
                        print(f"        ⚠️ LARGE: {len(body)} bytes")
                    
            except Exception as e:
                print(f"   Error processing response: {e}")
        
        page.on("request", on_request)
        page.on("response", on_response)
        
        print("=" * 70)
        print("🚀 LOADING WINLINE.RU")
        print("=" * 70)
        
        try:
            await page.goto("https://winline.ru/", timeout=60000, wait_until="domcontentloaded")
        except Exception as e:
            print(f"⚠️ Navigation error: {e}")
        
        print("\n⏳ Waiting 3 seconds for lazy-load events...")
        await asyncio.sleep(3)
        
        print("\n" + "=" * 70)
        print("📊 ANALYSIS")
        print("=" * 70)
        print(f"Total requests: {len(all_requests)}")
        print(f"Total responses: {len(all_responses)}")
        
        # Ищем ответы с событиями
        print("\n🔍 Responses with potential event data:")
        for resp in all_responses:
            if resp['has_data']:
                preview = resp.get('preview', '')
                if any(word in preview.lower() for word in ['event', 'match', 'sport', 'team', 'league']):
                    print(f"\n✅ {resp['url'][:100]}")
                    print(f"   Status: {resp['status']}, Size: {resp['size']} bytes")
                    print(f"   Preview: {preview[:200]}")
        
        # Сохраняем полный лог
        output = {
            'timestamp': datetime.now().isoformat(),
            'requests_count': len(all_requests),
            'responses_count': len(all_responses),
            'all_requests': all_requests,
            'all_responses': all_responses,
        }
        
        with open('winline_network_dump.json', 'w', encoding='utf-8') as f:
            json.dump(output, f, indent=2, ensure_ascii=False)
        
        print(f"\n💾 Full dump saved to winline_network_dump.json")
        
        await browser.close()

asyncio.run(main())
