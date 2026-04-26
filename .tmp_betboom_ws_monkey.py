import asyncio, json, sys
from playwright.async_api import async_playwright
URL = 'https://betboom.ru/sport/football'

async def main():
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled','--no-sandbox','--disable-dev-shm-usage'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width':1920,'height':1080}, locale='ru-RU')
        page = await context.new_page()
        await page.add_init_script("""
            (() => {
              Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
              window.chrome = {runtime: {}};
              window.__ws_log = [];
              const toHex = (buf) => {
                const arr = new Uint8Array(buf instanceof ArrayBuffer ? buf : buf.buffer || []);
                return Array.from(arr.slice(0, 80)).map(x => x.toString(16).padStart(2,'0')).join('');
              };
              const NativeWS = window.WebSocket;
              window.WebSocket = function(url, protocols) {
                const ws = new NativeWS(url, protocols);
                try {
                  window.__ws_log.push({kind:'open-call', url, protocols});
                } catch {}
                const origSend = ws.send.bind(ws);
                ws.send = function(data) {
                  try {
                    if (typeof data === 'string') window.__ws_log.push({kind:'send-text', url, data:data.slice(0,300)});
                    else window.__ws_log.push({kind:'send-bin', url, len:(data.byteLength || data.size || 0), hex:toHex(data)});
                  } catch {}
                  return origSend(data);
                };
                ws.addEventListener('message', (ev) => {
                  try {
                    if (typeof ev.data === 'string') window.__ws_log.push({kind:'recv-text', url, data:ev.data.slice(0,300)});
                    else if (ev.data instanceof Blob) {
                      ev.data.arrayBuffer().then(buf => window.__ws_log.push({kind:'recv-bin', url, len:buf.byteLength, hex:toHex(buf)}));
                    } else {
                      window.__ws_log.push({kind:'recv-other', url, type: typeof ev.data});
                    }
                  } catch {}
                });
                return ws;
              };
              window.WebSocket.prototype = NativeWS.prototype;
            })();
        """)
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(20000)
        log = await page.evaluate('window.__ws_log')
        await browser.close()
        sys.stdout.buffer.write(json.dumps(log, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
