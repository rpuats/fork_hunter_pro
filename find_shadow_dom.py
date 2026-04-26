#!/usr/bin/env python3
"""
Ищем Shadow DOM - это то что скрывает события в Web Components
"""

import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        
        print("Loading page...")
        await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
        
        print("Waiting for content...")
        await asyncio.sleep(5)
        
        # Ищем Shadow DOM
        result = await page.evaluate("""() => {
            const data = {
                has_shadow_dom: false,
                shadow_hosts: [],
                total_text: 0,
            };
            
            // Рекурсивно ищем Shadow DOM
            function searchShadow(element, depth = 0) {
                if (depth > 10) return;
                
                // Проверяем есть ли shadowRoot
                if (element.shadowRoot) {
                    data.has_shadow_dom = true;
                    data.shadow_hosts.push({
                        tag: element.tagName,
                        class: element.className,
                        text_length: element.shadowRoot.textContent.length,
                        text_sample: element.shadowRoot.textContent.substring(0, 100)
                    });
                    
                    data.total_text += element.shadowRoot.textContent.length;
                    
                    // Ищем дальше в shadowRoot
                    element.shadowRoot.querySelectorAll('*').forEach(el => {
                        searchShadow(el, depth + 1);
                    });
                }
                
                // Ищем в children
                Array.from(element.children).forEach(child => {
                    searchShadow(child, depth + 1);
                });
            }
            
            searchShadow(document.documentElement);
            
            return data;
        }""")
        
        print("\n" + "=" * 70)
        print("SHADOW DOM ANALYSIS")
        print("=" * 70 + "\n")
        
        print(f"Has Shadow DOM: {result['has_shadow_dom']}")
        print(f"Total Shadow DOM text: {result['total_text']} bytes")
        print(f"\nFound {len(result['shadow_hosts'])} Shadow DOM hosts:\n")
        
        for i, host in enumerate(result['shadow_hosts'][:20], 1):
            print(f"{i}. {host['tag']} class='{host['class']}'")
            print(f"   Text length: {host['text_length']} bytes")
            print(f"   Sample: {host['text_sample'][:80]}")
            print()
        
        await browser.close()

asyncio.run(main())
