import asyncio
import logging
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

logging.basicConfig(level=logging.INFO)

async def find_events_with_totals():
    pw = await async_playwright().start()
    browser = await pw.chromium.launch(
        headless=True,
        args=['--disable-blink-features=AutomationControlled']
    )
    config = generate_stealth_config()
    context = await create_stealth_context(browser, config)
    page = await context.new_page()
    
    # Try pre-match first - might have more totals
    url = "https://pari.ru/line/football"
    print(f"Navigating to: {url}")
    await page.goto(url, wait_until='networkidle', timeout=60000)
    await asyncio.sleep(5)
    
    # Find events with actual totals data
    result = await page.evaluate("""
        () => {
            const events = document.querySelectorAll('[class*="sport-base-event"]');
            const found = [];
            
            events.forEach((container, idx) => {
                if (found.length >= 5) return;
                
                // Get all factor elements
                const allFactors = container.querySelectorAll('.factor-value--zrkpK');
                const complexFactors = container.querySelectorAll('.factor-value--zrkpK.table-component-factor-value_complex--HFX8T');
                const simpleFactors = container.querySelectorAll('.factor-value--zrkpK:not(.table-component-factor-value_complex--HFX8T)');
                
                // Check if any complex factor has actual data (not disabled/empty)
                let hasRealTotals = false;
                const totalsData = [];
                
                complexFactors.forEach(f => {
                    const isEmpty = f.classList.contains('_empty--GIWnm') || f.classList.contains('_disable--MkuDy');
                    const paramEl = f.querySelector('.param--qbIN_');
                    const valueEl = f.querySelector('.value--OUKql, [class*="value"]');
                    
                    if (!isEmpty && paramEl && valueEl) {
                        const param = paramEl.textContent.trim();
                        const value = parseFloat(valueEl.textContent.trim().replace(',', '.'));
                        if (!isNaN(value) && value >= 1.01) {
                            hasRealTotals = true;
                            totalsData.push({
                                param: param,
                                value: value
                            });
                        }
                    }
                });
                
                // Get team names
                const nameEls = container.querySelectorAll('[class*="event__name"], [class*="name"]');
                let home = '', away = '';
                if (nameEls.length >= 2) {
                    home = nameEls[0].textContent.trim();
                    away = nameEls[1].textContent.trim();
                }
                
                // Get simple odds
                const simpleOdds = [];
                simpleFactors.forEach(f => {
                    const valEl = f.querySelector('.value--OUKql, [class*="value"]');
                    if (valEl) {
                        const val = parseFloat(valEl.textContent.trim().replace(',', '.'));
                        if (!isNaN(val) && val >= 1.01) {
                            simpleOdds.push(val);
                        }
                    }
                });
                
                if (hasRealTotals || simpleOdds.length >= 2) {
                    found.push({
                        idx: idx,
                        home: home,
                        away: away,
                        simpleOdds: simpleOdds.slice(0, 3),
                        hasTotals: hasRealTotals,
                        totalsData: totalsData,
                        totalComplexFactors: complexFactors.length,
                        totalSimpleFactors: simpleFactors.length
                    });
                }
            });
            
            return {
                totalEvents: events.length,
                eventsWithTotals: found.filter(e => e.hasTotals).length,
                sample: found.slice(0, 5)
            };
        }
    """)
    
    print(f"Total events: {result['totalEvents']}")
    print(f"Events with totals: {result['eventsWithTotals']}")
    
    for ev in result['sample']:
        home = ev['home'].encode('ascii', 'replace').decode('ascii')
        away = ev['away'].encode('ascii', 'replace').decode('ascii')
        print(f"\n--- Event {ev['idx']} ---")
        print(f"Teams: {home} vs {away}")
        print(f"Simple odds: {ev['simpleOdds']}")
        print(f"Has totals: {ev['hasTotals']}")
        if ev['totalsData']:
            for td in ev['totalsData']:
                param = td['param'].encode('ascii', 'replace').decode('ascii')
                print(f"  {param}: {td['value']}")
        print(f"Complex factors: {ev['totalComplexFactors']}, Simple factors: {ev['totalSimpleFactors']}")
    
    await browser.close()

asyncio.run(find_events_with_totals())
