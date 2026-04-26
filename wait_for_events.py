"""
Ждём когда загрузятся реальные события и перехватываем запрос
"""
import asyncio
import json
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        page = await browser.new_page()
        
        # Перехватываем ALL ответы включая WebSocket frames
        intercepted_data = {
            'xhrs': [],
            'fetches': [],
            'all_requests': []
        }
        
        async def on_response(response):
            url = response.url
            try:
                text = await response.text()
                
                # Ищем события в любом ответе
                if any(x in text.lower() for x in ['football', 'матч', 'событие', 'event', 'sport_id', 'team', 'odds', 'coefficient', '1x2']):
                    if len(text) > 500:  # Только значительные ответы
                        intercepted_data['xhrs'].append({
                            'url': url,
                            'size': len(text),
                            'type': response.headers.get('content-type'),
                            'contains_events': any(x in text.lower() for x in ['football', 'football', 'матч', 'team'])
                        })
                        print(f"[POTENTIAL EVENT DATA] {url[:90]}")
                        print(f"  Size: {len(text)}, Type: {response.headers.get('content-type')}")
                        
                        # Если JSON - покажи первые ключи
                        try:
                            data = json.loads(text[:10000])
                            if isinstance(data, dict):
                                print(f"  Keys: {list(data.keys())[:15]}")
                            elif isinstance(data, list) and len(data) > 0:
                                print(f"  Array[{len(data)}], first item type: {type(data[0]).__name__}")
                        except:
                            print(f"  Not JSON")
                        
                        # Покажи первый 500 символов
                        print(f"  Preview: {text[:500]}")
                        print()
            except:
                pass
        
        page.on('response', on_response)
        
        print("Loading https://winline.ru/stavki/sport/futbol/...")
        print("Waiting for event data to load...\n")
        
        await page.goto("https://winline.ru/stavki/sport/futbol/", wait_until='domcontentloaded', timeout=60000)
        
        # Ждём 10 секунд и смотрим что загрузилось
        print("\nWaiting 10 seconds for JavaScript to load events...")
        for i in range(10):
            await asyncio.sleep(1)
            body_length = await page.evaluate("() => document.body.innerText.length")
            print(f"  {i+1}s - Body text: {body_length} chars")
        
        # Попытаемся найти события через JavaScript
        print("\nSearching for loaded events...")
        
        events_found = await page.evaluate("""
        () => {
            const findings = {
                window_vars_with_data: [],
                text_with_odds: [],
                elements_with_team_names: 0
            };
            
            // Проверяем window variables
            for (let key in window) {
                try {
                    const val = window[key];
                    const valStr = JSON.stringify(val);
                    if (valStr && (
                        valStr.includes('football') || 
                        valStr.includes('sport') ||
                        valStr.includes('event') ||
                        valStr.includes('1x2')
                    ) && valStr.length > 200) {
                        findings.window_vars_with_data.push(key);
                    }
                } catch (e) {}
            }
            
            // Ищем текст с коэффициентами
            const text = document.body.innerText;
            const lines = text.split('\\n');
            for (let line of lines) {
                if (/\\d\\.\\d+/.test(line) && line.length > 20 && line.length < 200) {
                    if (findings.text_with_odds.length < 10) {
                        findings.text_with_odds.push(line.trim());
                    }
                }
            }
            
            return findings;
        }
        """)
        
        print(f"Window variables with data: {events_found['window_vars_with_data']}")
        if events_found['text_with_odds']:
            print(f"Lines with odds patterns:")
            for line in events_found['text_with_odds']:
                print(f"  - {line}")
        
        print(f"\nTotal requests/responses captured: {len(intercepted_data['xhrs'])}")
        
        await browser.close()

asyncio.run(main())
