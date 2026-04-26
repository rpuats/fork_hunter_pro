#!/usr/bin/env python3
"""
Winline API Network Interceptor

Перехватывает все fetch/WebSocket/XHR вызовы для понимания
как Winline загружает события.
"""

import asyncio
import json
import sys
from playwright.async_api import async_playwright
import logging

logging.basicConfig(level=logging.INFO, format='[%(asctime)s] %(levelname)s: %(message)s')
logger = logging.getLogger(__name__)


async def analyze_network():
    """Анализирует сетевые вызовы"""
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=[
                '--disable-blink-features=AutomationControlled',
                '--disable-dev-shm-usage',
            ]
        )
        
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        )
        
        page = await context.new_page()
        
        # Перехватываем все типы запросов
        requests_captured = {
            'fetch': [],
            'xhr': [],
            'websocket': [],
            'other': [],
        }
        
        async def handle_route(route):
            """Логируем все запросы"""
            request = route.request
            url = request.url
            method = request.method
            resource_type = request.resource_type
            
            # Логируем только интересные запросы
            if any(x in url.lower() for x in ['event', 'sport', 'market', 'odd', 'api', 'data', 'graphql']):
                logger.info(f"[{resource_type.upper()}] {method} {url[:80]}")
                
                requests_captured[resource_type if resource_type in requests_captured else 'other'].append({
                    'url': url,
                    'method': method,
                    'headers': dict(request.headers),
                })
            
            await route.continue_()
        
        await page.route('**/*', handle_route)
        
        # Перехватываем fetch вызовы из JavaScript
        fetch_calls = []
        
        async def log_fetch(request_info):
            """Логируем fetch вызовы из JS"""
            fetch_calls.append(request_info)
            logger.info(f"[JS-FETCH] {request_info}")
        
        logger.info("Loading Winline...")
        await page.goto('https://winline.ru/stavki/sport/futbol', wait_until='domcontentloaded', timeout=60000)
        
        logger.info("Analyzing page structure...")
        
        # Перехватываем fetch из JavaScript
        await page.evaluate('''async () => {
            const originalFetch = window.fetch;
            window.fetch = function(...args) {
                const url = args[0];
                if (typeof url === 'string' && (url.includes('event') || url.includes('api') || url.includes('market'))) {
                    console.log('[FETCH-INTERCEPT] ' + url);
                }
                return originalFetch.apply(this, args);
            };
        }''')
        
        # Ждем сетевых запросов
        await page.wait_for_timeout(10000)
        
        # Анализируем структуру страницы
        logger.info("\n=== PAGE STRUCTURE ===")
        
        page_info = await page.evaluate('''() => {
            return {
                html: document.documentElement.outerHTML.substring(0, 500),
                web_components: Array.from(document.querySelectorAll('*'))
                    .filter(el => el.tagName.includes('-'))
                    .map(el => el.tagName),
                scripts: Array.from(document.querySelectorAll('script'))
                    .filter(s => s.src)
                    .map(s => s.src.substring(0, 100)),
                body_html: document.body.innerHTML.substring(0, 200),
            };
        }''')
        
        logger.info(f"Web Components: {page_info['web_components']}")
        logger.info(f"Scripts: {page_info['scripts'][:3]}")
        
        # Ищем API endpoints в странице/скриптах
        logger.info("\n=== SEARCHING FOR API ENDPOINTS ===")
        
        endpoints = await page.evaluate('''() => {
            const endpoints = new Set();
            
            // Ищем в window объектах
            for (let key in window) {
                try {
                    const val = window[key];
                    if (typeof val === 'object' && val && ('apiUrl' in val || 'api' in val)) {
                        console.log('Found API object: ' + key);
                    }
                } catch (e) {}
            }
            
            // Ищем в HTML
            const scripts = document.querySelectorAll('script');
            for (let script of scripts) {
                if (script.textContent) {
                    const matches = script.textContent.match(/api["\s:/]*["']([^"'\\s]+)/gi);
                    if (matches) {
                        matches.forEach(m => endpoints.add(m));
                    }
                }
            }
            
            return Array.from(endpoints);
        }''')
        
        logger.info(f"Found potential endpoints: {endpoints[:5]}")
        
        # Проверяем Network Inspector
        logger.info("\n=== MONITORING NETWORK ===")
        
        await page.wait_for_timeout(5000)
        
        logger.info(f"Fetch calls: {len(requests_captured['fetch'])}")
        logger.info(f"XHR calls: {len(requests_captured['xhr'])}")
        logger.info(f"WebSocket: {len(requests_captured['websocket'])}")
        
        # Выводим первые запросы
        if requests_captured['fetch']:
            logger.info("\nFirst fetch calls:")
            for req in requests_captured['fetch'][:5]:
                logger.info(f"  {req['url'][:100]}")
        
        # Ищем Shadow DOM элементы
        logger.info("\n=== SHADOW DOM ANALYSIS ===")
        
        shadow_info = await page.evaluate('''() => {
            const info = {};
            
            document.querySelectorAll('*').forEach(el => {
                if (el.shadowRoot) {
                    info[el.tagName] = {
                        shadow_html: el.shadowRoot.innerHTML.substring(0, 200),
                        children: el.shadowRoot.children.length,
                    };
                }
            });
            
            return info;
        }''')
        
        if shadow_info:
            logger.info(f"Elements with Shadow DOM: {Object.keys(shadow_info)}")
            for tag, info in shadow_info.items():
                logger.info(f"  {tag}: {info['children']} children")
        
        # Пробуем получить события через window объекты
        logger.info("\n=== WINDOW OBJECTS ===")
        
        window_data = await page.evaluate('''() => {
            const result = {};
            
            // Ищем объекты содержащие события
            const keys = Object.keys(window);
            for (let key of keys) {
                try {
                    const val = window[key];
                    if (val && typeof val === 'object') {
                        const str = JSON.stringify(val).substring(0, 100);
                        if (str.includes('event') || str.includes('sport') || str.includes('match')) {
                            result[key] = str;
                        }
                    }
                } catch (e) {}
            }
            
            return result;
        }''')
        
        if window_data:
            logger.info(f"Objects with event/sport data: {list(window_data.keys())[:3]}")
        
        await browser.close()
        
        # Сохраняем результаты
        results = {
            'requests': requests_captured,
            'endpoints': endpoints,
            'page_info': page_info,
            'shadow_dom': shadow_info,
            'window_objects': window_data,
        }
        
        with open('winline_api_analysis.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, indent=2, ensure_ascii=False, default=str)
        
        logger.info("\n✓ Analysis saved to winline_api_analysis.json")
        
        return results


async def main():
    logger.info("=" * 60)
    logger.info("WINLINE API NETWORK ANALYSIS")
    logger.info("=" * 60)
    
    results = await analyze_network()
    
    logger.info("\n" + "=" * 60)
    logger.info("ANALYSIS COMPLETE")
    logger.info("=" * 60)


if __name__ == '__main__':
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())
    
    asyncio.run(main())
