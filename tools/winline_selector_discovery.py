#!/usr/bin/env python3
"""
Winline DOM Selector Discovery Tool

Analyzes current Winline HTML structure to identify working selectors for:
- Event cards
- Team names
- Odds/coefficients
- Markets
- Tournament names
"""

import asyncio
import json
from playwright.async_api import async_playwright
import re
from typing import Dict, List, Set
import sys
import os

# Fix encoding for Windows
if sys.platform == 'win32':
    os.environ['PYTHONIOENCODING'] = 'utf-8'


async def discover_winline_selectors():
    """Discover working selectors on current Winline site"""
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={'width': 1920, 'height': 1080},
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        )
        page = await context.new_page()
        
        # Navigate to Winline
        print("[*] Navigating to Winline...")
        try:
            await page.goto('https://winline.ru/stavki/sport/futbol', timeout=15000)
            await page.wait_for_load_state('networkidle')
        except Exception as e:
            print(f"[!] Failed to load Winline: {e}")
            return
        
        # Discover selectors
        results = await page.evaluate(r"""
        () => {
            const discover = {};
            
            // Discover event card selectors
            discover.eventCards = {
                webComponents: Array.from(document.querySelectorAll('*'))
                    .filter(el => el.tagName.includes('-'))
                    .slice(0, 5)
                    .map(el => el.tagName),
                classes: new Set(),
                ids: new Set(),
            };
            
            // Sample all element classes
            Array.from(document.querySelectorAll('[class*="event"], [class*="card"], [class*="match"]'))
                .forEach(el => {
                    el.className.split(' ').forEach(cls => {
                        if (cls.length > 2) discover.eventCards.classes.add(cls);
                    });
                });
            
            // Discover odds/coefficient selectors  
            discover.odds = {
                webComponents: Array.from(document.querySelectorAll('*'))
                    .filter(el => el.textContent && /^[0-9][.,0-9]*$/.test(el.textContent.trim()))
                    .filter(el => el.tagName.includes('-'))
                    .map(el => el.tagName)
                    .slice(0, 5),
                classes: new Set(),
                buttons: new Set(),
            };
            
            Array.from(document.querySelectorAll('[class*="coef"], [class*="odd"], button'))
                .forEach(el => {
                    el.className.split(' ').forEach(cls => {
                        if (cls.length > 2) discover.odds.classes.add(cls);
                    });
                    if (el.textContent && /^[0-9][.,0-9]*$/.test(el.textContent.trim())) {
                        if (el.className) discover.odds.buttons.add(el.className);
                    }
                });
            
            // Discover team name patterns
            discover.teamNames = {
                patterns: [],
                selectorsThatWork: [],
            };
            
            const teamLikeText = Array.from(document.querySelectorAll('*'))
                .filter(el => {
                    const text = el.textContent?.trim() || '';
                    return text.length > 2 && text.length < 50 && 
                           !text.includes('\n') && 
                           !/^[0-9:.,\\s-]+$/.test(text);
                })
                .slice(0, 100);
            
            discover.teamNames.samples = teamLikeText
                .map(el => ({
                    text: el.textContent?.trim().substring(0, 30),
                    className: el.className,
                    tagName: el.tagName,
                }))
                .slice(0, 10);
            
            // Discover market/tournament names
            discover.tournaments = {
                classes: new Set(),
                webComponents: [],
            };
            
            Array.from(document.querySelectorAll('[class*="tournament"], [class*="league"], [class*="sport"]'))
                .forEach(el => {
                    if (el.tagName.includes('-')) {
                        discover.tournaments.webComponents.push(el.tagName);
                    }
                    el.className.split(' ').forEach(cls => {
                        if (cls.length > 2) discover.tournaments.classes.add(cls);
                    });
                });
            
            // Convert Sets to Arrays for JSON
            return {
                eventCards: {
                    webComponents: discover.eventCards.webComponents,
                    classes: Array.from(discover.eventCards.classes).slice(0, 20),
                },
                odds: {
                    webComponents: discover.odds.webComponents,
                    classes: Array.from(discover.odds.classes).slice(0, 20),
                    buttons: Array.from(discover.odds.buttons).slice(0, 20),
                },
                teamNames: discover.teamNames,
                tournaments: {
                    webComponents: Array.from(new Set(discover.tournaments.webComponents)),
                    classes: Array.from(discover.tournaments.classes).slice(0, 20),
                },
                pageStructure: {
                    title: document.title,
                    url: window.location.href,
                    mainElements: Array.from(document.querySelectorAll('main, [role="main"], .main, #main, .container, [class*="main"]'))
                        .map(el => ({ tag: el.tagName, className: el.className }))
                        .slice(0, 5),
                },
            };
        }
        """)
        
        # Print results
        print("\n[+] DISCOVERED SELECTORS:\n")
        print(json.dumps(results, indent=2, default=str))
        
        # Save results
        with open('winline_selectors_discovery.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, indent=2, default=str)
        print("\n[OK] Saved to winline_selectors_discovery.json")
        
        # Close browser
        await browser.close()


if __name__ == '__main__':
    asyncio.run(discover_winline_selectors())
