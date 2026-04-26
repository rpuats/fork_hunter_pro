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
        page.on('console', lambda msg: print('CONSOLE:', msg.type, msg.text))
        page.on('pageerror', lambda exc: print('PAGEERROR:', exc))
        await page.goto(URL, wait_until='domcontentloaded', timeout=30000)
        await page.wait_for_timeout(3000)
        result = await page.evaluate("""async (widgetUrl) => {
          const next = JSON.parse(document.getElementById('__NEXT_DATA__').textContent);
          const cfg = {
            balances: [],
            container: document.getElementById('sportApp'),
            api: { url: next.runtimeConfig.SPORTBOOK_API_URL },
            feedWS: { url: next.runtimeConfig.SPORTBOOK_FEED_WS_URL },
            betsHistoryWS: { url: next.runtimeConfig.SPORTBOOK_BETS_HISTORY_WS_URL },
            marketBetStatsWS: { url: next.runtimeConfig.SPORTBOOK_MARKET_BET_STATS_WS_URL },
            currency: 'RUB',
            coefficientType: 'decimal',
            language: 'ru',
            theme: 'dark',
            router: { initialRoute: '/sport/football' },
            partnerName: 'ru',
            parentLayout: { footerHeight: 72, headerHeight: 80, headerVisible: true, headerTransition: { duration: 300, timingFunction: 'ease' } },
            user: null,
            analytics: { target: 'bb-ru' },
            experimentalFeatures: { routeRedirectData: true, hideQuickGames: true, showMatchesResults: true },
            coupon: { showSupportChatButton: true }
          };
          try {
            const script = document.createElement('script');
            script.type = 'module';
            script.crossOrigin = 'anonymous';
            script.textContent = `import * as mod from '${widgetUrl}'; window.__bb_mod_keys = Object.keys(mod); window.__bb_has_init = !!mod.init; if (mod.init) { const res = mod.init(${JSON.stringify('__CFG__')}); window.__bb_init_started = true; window.__bb_init_result = !!res; }`;
            document.head.appendChild(script);
            await new Promise(r => setTimeout(r, 6000));
            return {
              hasRegister: !!window.SportbookWidgetRegister,
              modKeys: window.__bb_mod_keys || null,
              hasInit: window.__bb_has_init || false,
              initStarted: window.__bb_init_started || false,
              initResult: window.__bb_init_result || false,
              bodyLen: ((document.body && document.body.innerText) || '').length,
              bbNm: document.querySelectorAll('.bb-Nm').length,
              bbRm: document.querySelectorAll('.bb-Rm').length,
              sportAppHtmlLen: (document.getElementById('sportApp')?.innerHTML || '').length,
            };
          } catch (e) {
            return { error: String(e) };
          }
        }""".replace(JSON.stringify('__CFG__'), 'cfg'), WIDGET)
        await browser.close()
        sys.stdout.buffer.write(json.dumps(result, ensure_ascii=False, indent=2).encode('utf-8'))
asyncio.run(main())
