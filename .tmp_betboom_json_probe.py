import asyncio, json, sys, os
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
async def main():
    api=[]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        async def handle_response(response):
            try:
                ct=response.headers.get('content-type','')
                if response.status==200 and 'json' in ct:
                    data=await response.json()
                    api.append({'url':response.url,'keys':list(data.keys())[:15] if isinstance(data,dict) else None,'kind':type(data).__name__,'preview':str(data)[:500]})
            except Exception:
                pass
        page.on('response', handle_response)
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(30000)
        # robust click path similar to parser
        for label in ['1н','Теннис','Футбол','Бейсбол']:
            try:
                await page.evaluate("""(targetText) => {
                    const normalize = (value) => String(value || '').replace(/\u00a0/g,' ').replace(/\s+/g,' ').trim();
                    const nodes = Array.from(document.querySelectorAll('button, a, div, span'));
                    const target = nodes.find((node) => {
                        const text = normalize(node.textContent || '');
                        if (text !== targetText) return false;
                        const rect = node.getBoundingClientRect();
                        return rect.width > 0 && rect.height > 0;
                    });
                    if (!target) return false;
                    target.click();
                    return true;
                }""", label)
            except Exception:
                pass
            await page.wait_for_timeout(6000)
        await browser.close()
    uniq=[]; seen=set()
    for item in api:
        if item['url'] in seen: continue
        seen.add(item['url']); uniq.append(item)
    sys.stdout.buffer.write(json.dumps(uniq[:80], ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
