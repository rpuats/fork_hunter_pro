#!/usr/bin/env python3
"""
Выводим весь DOM и ищем события
"""

import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        
        print("Loading page...")
        await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
        
        print("Waiting for content to load...")
        await asyncio.sleep(5)
        
        # Ищем все элементы которые могут содержать события
        print("\n" + "=" * 70)
        print("SEARCHING DOM FOR EVENT DATA")
        print("=" * 70 + "\n")
        
        # Проверяем есть ли элементы с текстом содержащим названия команд
        result = await page.evaluate("""() => {
            const data = {
                total_elements: document.querySelectorAll('*').length,
                body_text: document.body.innerText.length,
                body_text_sample: document.body.innerText.substring(0, 1000),
                elements_with_text: [],
            };
            
            // Ищем элементы которые выглядят как события
            const keywords = ['vs', 'live', '20:00', 'матч', 'событие', 'футбол', 'лига'];
            
            document.querySelectorAll('[class*="event"], [class*="match"], [class*="game"], [id*="event"], [id*="match"]').forEach(el => {
                if (el.textContent.length > 10 && el.textContent.length < 500) {
                    data.elements_with_text.push({
                        tag: el.tagName,
                        class: el.className,
                        id: el.id,
                        text: el.textContent.substring(0, 100)
                    });
                }
            });
            
            return data;
        }""")
        
        print(f"Total elements: {result['total_elements']}")
        print(f"Body text size: {result['body_text']} bytes")
        print(f"\nBody text sample:")
        print(result['body_text_sample'])
        
        if result['elements_with_text']:
            print(f"\n\nFound {len(result['elements_with_text'])} potential event elements:")
            for elem in result['elements_with_text'][:20]:
                print(f"\n  {elem['tag']} class='{elem['class']}' id='{elem['id']}'")
                print(f"    Text: {elem['text'][:80]}")
        else:
            print("\n❌ No event elements found in DOM")
        
        # Попробуем найти все текстовые узлы которые содержат названия команд/событий
        print("\n\n" + "=" * 70)
        print("SEARCHING FOR TEAM NAMES AND EVENT KEYWORDS")
        print("=" * 70 + "\n")
        
        keywords_result = await page.evaluate("""() => {
            const keywords = ['madrid', 'barcelona', 'juventus', 'milan', 'liverpool', 'manchester', 'match', 'live', 'event', 'vs', '20:00', '19:00', '18:00'];
            const found = [];
            
            function searchText(node) {
                if (node.nodeType === 3) { // Text node
                    const text = node.textContent.toLowerCase();
                    for (const keyword of keywords) {
                        if (text.includes(keyword)) {
                            found.push({
                                text: node.textContent.trim().substring(0, 100),
                                parent: node.parentElement?.tagName,
                                parent_class: node.parentElement?.className
                            });
                            break;
                        }
                    }
                } else {
                    for (let i = 0; i < node.childNodes.length; i++) {
                        searchText(node.childNodes[i]);
                    }
                }
            }
            
            searchText(document.body);
            return found.slice(0, 50);
        }""")
        
        if keywords_result:
            print(f"Found {len(keywords_result)} elements with team/event keywords:")
            for item in keywords_result[:10]:
                print(f"\n  {item['parent']} class='{item['parent_class']}'")
                print(f"    {item['text']}")
        else:
            print("❌ No team names or event keywords found")
        
        await browser.close()

asyncio.run(main())
