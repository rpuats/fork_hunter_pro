"""
Проверяем какие селекторы работают на текущей Winline
"""
import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        page = await browser.new_page()
        
        url = "https://winline.ru/stavki/sport/futbol/"
        print(f"Loading {url}...")
        
        try:
            await page.goto(url, wait_until='load', timeout=60000)
        except:
            print("Navigation timeout, but continuing...")
        
        await asyncio.sleep(3)
        
        # Ищем какие элементы на странице содержат события
        print("\nSearching for event elements...")
        
        selectors_to_try = [
            'ww-feature-event-mini-card-dsk',
            'app-event-card',
            '.event-card',
            '[class*="event"]',
            '[data-event-id]',
            '[id*="event"]',
            '.match',
            '.match-card',
            '[class*="match"]',
            'event-card',
            '.game',
            '[class*="game"]',
            '.fixture',
            '[class*="fixture"]',
            'div[data-testid]',
        ]
        
        for selector in selectors_to_try:
            try:
                count = await page.query_selector_all(selector)
                if count:
                    print(f"  {selector}: {len(count)} elements")
                    # Show first element's HTML
                    if len(count) > 0:
                        html = await page.locator(selector).first.inner_html()
                        print(f"    First element: {html[:200]}...")
            except:
                pass
        
        # Проверим текстовый контент
        print("\nChecking page text content...")
        text = await page.evaluate("() => document.body.innerText")
        
        # Ищем названия команд
        print(f"Page text (first 1000 chars):\n{text[:1000]}")
        
        # Ищем числа (вероятно коэффициенты)
        import re
        numbers = re.findall(r'\d+\.\d+', text[:5000])
        if numbers:
            print(f"\nFound potential odds: {numbers[:20]}")
        
        # Попробуем получить события через JavaScript
        print("\nTrying JavaScript extraction...")
        
        result = await page.evaluate("""
        () => {
            const info = {
                doc_title: document.title,
                body_text_length: document.body.innerText.length,
                all_divs: document.querySelectorAll('div').length,
                all_buttons: document.querySelectorAll('button').length,
                all_spans: document.querySelectorAll('span').length,
                web_components: Array.from(document.querySelectorAll('*'))
                    .filter(e => e.tagName.includes('-'))
                    .map(e => e.tagName)
                    .slice(0, 20)
            };
            
            // Ищем текст с командами
            const text = document.body.innerText;
            const has_teams = text.includes('Real') || text.includes('Madrid') || 
                             text.includes('Barcelona') || text.includes('Manchester');
            
            return {
                ...info,
                has_team_names: has_teams,
                first_500_chars: text.slice(0, 500)
            };
        }
        """)
        
        print(f"\nDocument structure:")
        print(f"  Title: {result['doc_title']}")
        print(f"  Body text length: {result['body_text_length']}")
        print(f"  Web components found: {result['web_components']}")
        print(f"  Has team names: {result['has_team_names']}")
        print(f"  First 500 chars: {result['first_500_chars']}")
        
        await browser.close()

asyncio.run(main())
