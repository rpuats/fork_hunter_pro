"""
Простой способ: запустить ВИДИМЫЙ браузер и вручную посмотреть WebSocket
"""
import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        # ВИДИМЫЙ браузер
        browser = await p.chromium.launch(headless=False, args=[
            "--disable-blink-features=AutomationControlled",
            "--no-sandbox"
        ])
        
        context = await browser.new_context()
        page = await context.new_page()
        
        print("=" * 70)
        print("WINLINE WEBSOCKET ANALYSIS")
        print("=" * 70)
        print("\nOpening browser with DevTools...")
        print("1. Open DevTools (F12)")
        print("2. Go to Network tab")
        print("3. Filter for 'WS' (WebSocket)")
        print("4. Click on 'data_ng' connection")
        print("5. Go to 'Messages' tab")
        print("6. You'll see binary frames being sent/received")
        print("\nI'll load the page and keep browser open for you to inspect.")
        print("=" * 70)
        
        print("\nGoing to https://winline.ru...")
        await page.goto("https://winline.ru/stavki/sport/futbol/", wait_until="domcontentloaded", timeout=60000)
        
        print("✓ Page loaded. WebSocket should be connected now.")
        print("\nPAGE CONTENT CHECK:")
        
        # Check what's in the page after load
        content_check = await page.evaluate("""
        () => {
            return {
                title: document.title,
                h1_count: document.querySelectorAll('h1').length,
                events_mentioned: document.body.innerText.includes('событ') || document.body.innerText.includes('Event'),
                has_live: document.body.innerText.includes('Live') || document.body.innerText.includes('LIVE'),
                body_length: document.body.innerText.length,
                all_text: document.body.innerText.slice(0, 500)
            }
        }
        """)
        
        print(f"Title: {content_check['title']}")
        print(f"Events/Live words found: {content_check['events_mentioned']}")
        print(f"'Live' keyword found: {content_check['has_live']}")
        print(f"Body text (first 500 chars):\n{content_check['all_text']}")
        
        print("\n" + "=" * 70)
        print("Browser is still open - inspect WebSocket in DevTools!")
        print("=" * 70)
        
        # Wait 30 seconds then close
        print("\nBrowser will close in 30 seconds...")
        print("Or press Ctrl+C to close immediately")
        
        try:
            await asyncio.sleep(30)
        except KeyboardInterrupt:
            pass
        
        await browser.close()
        print("Browser closed.")

if __name__ == '__main__':
    asyncio.run(main())
