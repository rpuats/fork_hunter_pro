import asyncio, json, sys
from playwright.async_api import async_playwright
URL='https://betboom.ru/sport'
async def main():
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(headless=True,args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context=await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',viewport={'width':1920,'height':1080},locale='ru-RU')
        page=await context.new_page()
        await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(30000)
        await page.evaluate("""() => {
            const norm=(v)=>String(v||'').replace(/\u00a0/g,' ').replace(/\s+/g,' ').trim();
            const click=(label)=>{ const nodes=[...document.querySelectorAll('button,a,div,span')]; const t=nodes.find(n=>norm(n.textContent||'')===label && n.getBoundingClientRect().width>0 && n.getBoundingClientRect().height>0); if(t){t.click(); return true;} return false; };
            click('1н'); click('Футбол');
        }""")
        await page.wait_for_timeout(6000)
        data=await page.evaluate("""() => {
          const nodes=[...document.querySelectorAll('*')].map((n,idx)=>({idx,tag:n.tagName,id:n.id||'',cls:typeof n.className==='string'?n.className:'',client:n.clientHeight||0,scroll:n.scrollHeight||0,text:((n.innerText||'').trim()).slice(0,120)})).filter(x=>x.scroll>x.client+100 && x.client>0).sort((a,b)=>(b.scroll-b.client)-(a.scroll-a.client)).slice(0,40);
          return nodes;
        }""")
        await browser.close()
        sys.stdout.buffer.write(json.dumps(data, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
