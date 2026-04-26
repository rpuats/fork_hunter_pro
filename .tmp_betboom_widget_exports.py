import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'
WIDGET = 'https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js'
async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(4000)
        data = await page.evaluate("""async (widgetUrl) => {
            try {
                const mod = await import(widgetUrl);
                const keys = Object.keys(mod).sort();
                const typed = {};
                for (const k of keys.slice(0, 80)) {
                    typed[k] = typeof mod[k];
                }
                return { ok: true, keys, typed };
            } catch (e) {
                return { ok: false, error: String(e) };
            }
        }""", WIDGET)
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
