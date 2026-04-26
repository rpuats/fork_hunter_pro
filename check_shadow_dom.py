"""
Ищем события в Shadow DOM
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
            pass
        
        await asyncio.sleep(3)
        
        print("Checking Shadow DOM...")
        
        result = await page.evaluate("""
        () => {
            const findings = {
                shadow_hosts: [],
                ww_app_structure: {}
            };
            
            // Найдем все элементы с Shadow DOM
            document.querySelectorAll('*').forEach(el => {
                if (el.shadowRoot) {
                    findings.shadow_hosts.push({
                        tag: el.tagName,
                        has_shadow: true,
                        shadow_children: el.shadowRoot.children.length
                    });
                }
            });
            
            // Специально ищем WW-APP-DSK
            const wwapp = document.querySelector('ww-app-dsk');
            if (wwapp) {
                findings.ww_app_structure = {
                    found: true,
                    has_shadow: !!wwapp.shadowRoot,
                    shadow_html_preview: wwapp.shadowRoot ? 
                        wwapp.shadowRoot.innerHTML.slice(0, 500) : 'no shadow dom'
                };
                
                if (wwapp.shadowRoot) {
                    // Ищем события в shadow dom
                    const shadow = wwapp.shadowRoot;
                    findings.shadow_content = {
                        div_count: shadow.querySelectorAll('div').length,
                        event_cards: shadow.querySelectorAll('[class*="event"]').length,
                        match_elements: shadow.querySelectorAll('[class*="match"]').length,
                        game_elements: shadow.querySelectorAll('[class*="game"]').length,
                        all_text_length: shadow.innerText.length,
                        text_preview: shadow.innerText.slice(0, 500)
                    };
                }
            }
            
            return findings;
        }
        """)
        
        print("\nShadow DOM Analysis:")
        print(f"  Shadow hosts found: {len(result['shadow_hosts'])}")
        for host in result['shadow_hosts']:
            print(f"    - {host['tag']}: {host['shadow_children']} children")
        
        print(f"\nWW-APP-DSK structure:")
        for key, value in result['ww_app_structure'].items():
            if key != 'shadow_html_preview':
                print(f"  {key}: {value}")
            else:
                print(f"  {key} (first 200 chars): {str(value)[:200]}")
        
        if 'shadow_content' in result:
            print(f"\nShadow DOM Content:")
            for key, value in result['shadow_content'].items():
                print(f"  {key}: {value}")
        
        # Теперь пытаемся достать события через pierceHandler (это не работает в обычном JS)
        # но мы можем использовать Playwright's pierce selector
        print("\n\nTrying Playwright pierce selectors...")
        
        # Попробуем найти элементы через pierce
        try:
            # pierce не работает, но попробуем стандартные селекторы внутри shadow
            print("Attempting to extract event cards...")
            
            # Используем evaluate с DOM API чтобы пройтись по shadow tree
            events = await page.evaluate("""
            () => {
                const events = [];
                const wwapp = document.querySelector('ww-app-dsk');
                
                if (!wwapp || !wwapp.shadowRoot) {
                    return events;
                }
                
                const shadow = wwapp.shadowRoot;
                
                // Ищем любые элементы которые выглядят как события
                const allElements = shadow.querySelectorAll('*');
                let eventCount = 0;
                let textSamples = [];
                
                allElements.forEach((el, idx) => {
                    const text = el.textContent || '';
                    const html = el.outerHTML || '';
                    
                    // Ищем паттерны которые выглядят как названия команд или коэффициенты
                    if (text.includes(' - ') && text.length < 100) {
                        // Возможно название события
                        if (textSamples.length < 10) {
                            textSamples.push({
                                tag: el.tagName,
                                class: el.className,
                                text: text.slice(0, 100)
                            });
                        }
                    }
                });
                
                return {
                    total_elements: allElements.length,
                    samples: textSamples,
                    shadow_text_length: shadow.innerText.length
                };
            }
            """)
            
            print(f"\nShadow DOM events exploration:")
            print(f"  Total elements in shadow: {events['total_elements']}")
            print(f"  Shadow text length: {events['shadow_text_length']}")
            if events['samples']:
                print(f"  Text samples (potential events):")
                for sample in events['samples']:
                    print(f"    - {sample['tag']}.{sample['class']}: {sample['text']}")
        
        except Exception as e:
            print(f"Error: {e}")
        
        await browser.close()

asyncio.run(main())
