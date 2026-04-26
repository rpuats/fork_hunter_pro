"""
Анализируем WebSocket протокол через Playwright
"""
import asyncio
import json
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=False)  # Видимый браузер
        page = await browser.new_page()
        
        captured_messages = {
            'sent': [],
            'received': []
        }
        
        # Перехватываем WebSocket
        async def handle_websocket(ws):
            print(f"WebSocket: {ws.url}")
            
            async def on_frame_receive(payload):
                print(f"  << Received ({len(str(payload))} bytes)")
                if isinstance(payload, str):
                    try:
                        data = json.loads(payload)
                        print(f"     JSON keys: {list(data.keys())[:10] if isinstance(data, dict) else 'list'}")
                        captured_messages['received'].append(data)
                    except:
                        print(f"     Not JSON: {payload[:100]}")
                        captured_messages['received'].append(str(payload)[:200])
            
            async def on_frame_send(payload):
                print(f"  >> Sent ({len(str(payload))} bytes)")
                if isinstance(payload, str):
                    try:
                        data = json.loads(payload)
                        print(f"     JSON: {data}")
                        captured_messages['sent'].append(data)
                    except:
                        print(f"     {payload[:100]}")
                        captured_messages['sent'].append(str(payload)[:200])
            
            ws.on('framereceived', on_frame_receive)
            ws.on('framesent', on_frame_send)
        
        page.on('websocket', handle_websocket)
        
        print("Loading winline.ru...")
        await page.goto('https://winline.ru/stavki/sport/futbol/', wait_until='load', timeout=60000)
        
        print("\nWaiting 20 seconds for WebSocket traffic...")
        await asyncio.sleep(20)
        
        print(f"\n=== WebSocket Analysis ===")
        print(f"Messages sent: {len(captured_messages['sent'])}")
        print(f"Messages received: {len(captured_messages['received'])}")
        
        if captured_messages['sent']:
            print(f"\nFirst sent message:")
            print(json.dumps(captured_messages['sent'][0], indent=2, ensure_ascii=False)[:500])
        
        if captured_messages['received']:
            print(f"\nFirst received message:")
            msg = captured_messages['received'][0]
            if isinstance(msg, dict):
                print(json.dumps(msg, indent=2, ensure_ascii=False)[:500])
            else:
                print(str(msg)[:500])
        
        input("Press Enter to close browser...")
        await browser.close()

asyncio.run(main())
