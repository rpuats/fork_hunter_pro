"""
Проверяем - заблокирован ли headless браузер
"""
import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        # Сначала - обычный headless
        print("=" * 70)
        print("TEST 1: Normal headless browser")
        print("=" * 70)
        
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        page = await browser.new_page()
        
        url = "https://winline.ru/stavki/sport/futbol/"
        
        await page.goto(url, wait_until='domcontentloaded', timeout=30000)
        
        # Check if navigation succeeded
        title = await page.title()
        url_after = page.url
        
        print(f"Title: {title}")
        print(f"URL: {url_after}")
        print(f"Expected 'футбол', got: {'футбол' in title}")
        
        # Check if page loaded properly
        body_text = await page.evaluate("() => document.body.innerText.length")
        print(f"Body text length: {body_text}")
        
        # Check for error messages
        errors = await page.evaluate("""
        () => {
            const text = document.body.innerText;
            return {
                has_error: text.includes('ошибка') || text.includes('error') || text.includes('bot'),
                has_maintenance: text.includes('обслуживание') || text.includes('maintenance'),
                has_access_denied: text.includes('доступ') || text.includes('denied'),
                text_preview: text.slice(0, 500)
            };
        }
        """)
        
        print(f"Has error messages: {errors['has_error']}")
        print(f"Has maintenance message: {errors['has_maintenance']}")
        print(f"Has access denied: {errors['has_access_denied']}")
        print(f"Content preview: {errors['text_preview'][:300]}")
        
        await browser.close()
        
        # Теперь - с видимым браузером
        print("\n" + "=" * 70)
        print("TEST 2: Visible browser (headless=False)")
        print("=" * 70)
        
        browser = await p.chromium.launch(headless=False, args=['--no-sandbox'])
        page = await browser.new_page()
        
        print("Opening browser (will show window)...")
        
        await page.goto(url, wait_until='domcontentloaded', timeout=30000)
        
        title = await page.title()
        print(f"Title: {title}")
        
        # Don't wait too long
        await asyncio.sleep(2)
        
        body_text = await page.evaluate("() => document.body.innerText.length")
        print(f"Body text length: {body_text}")
        
        text_preview = await page.evaluate("() => document.body.innerText.slice(0, 500)")
        print(f"Content: {text_preview[:300]}")
        
        await browser.close()

asyncio.run(main())
