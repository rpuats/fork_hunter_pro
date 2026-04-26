#!/usr/bin/env python3
"""
Winline Deep Structure Analysis

Запускает headless Chrome, загружает сайт, анализирует реальную DOM,
тестирует селекторы, проверяет защиту, вытягивает события.
"""

import asyncio
import json
import sys
from pathlib import Path
from playwright.async_api import async_playwright, expect
import logging

logging.basicConfig(
    level=logging.INFO,
    format='[%(asctime)s] %(levelname)s: %(message)s'
)
logger = logging.getLogger(__name__)


class WinlineAnalyzer:
    def __init__(self):
        self.results = {
            'page_structure': {},
            'selector_tests': {},
            'protection': {},
            'events_found': [],
            'api_calls': [],
            'javascript_analysis': {},
        }
        self.base_url = 'https://winline.ru/stavki/sport/futbol'
        
    async def analyze(self):
        """Main analysis entry point"""
        async with async_playwright() as p:
            browser = await p.chromium.launch(
                headless=True,
                args=[
                    '--disable-blink-features=AutomationControlled',
                    '--disable-dev-shm-usage',
                    '--no-first-run',
                    '--no-default-browser-check',
                ]
            )
            
            context = await browser.new_context(
                user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36',
                viewport={'width': 1920, 'height': 1080},
                bypass_csp=True,
            )
            
            # Stealth режим - скрываем что это бот
            await context.add_init_script('''
                Object.defineProperty(navigator, 'webdriver', {
                    get: () => undefined,
                });
                Object.defineProperty(navigator, 'plugins', {
                    get: () => [1, 2, 3, 4, 5],
                });
            ''')
            
            page = await context.new_page()
            
            # Логируем сетевые запросы
            api_calls = []
            async def handle_route(route):
                api_calls.append({
                    'url': route.request.url,
                    'method': route.request.method,
                    'headers': dict(route.request.headers),
                })
                await route.continue_()
            
            await page.route('**/*', handle_route)
            
            logger.info(f"Loading {self.base_url}...")
            
            try:
                # Сначала загружаем с нормальным ожиданием
                await page.goto(self.base_url, wait_until='domcontentloaded', timeout=60000)
                logger.info("✓ DOM loaded, waiting for content...")
                
                # Ждем пока загрузятся события
                await page.wait_for_timeout(5000)
                logger.info("✓ Page fully loaded")
            except Exception as e:
                logger.error(f"✗ Failed to load page: {e}")
                logger.info("Trying with reduced wait strategy...")
                await browser.close()
                return self.results
            
            # 1. Анализ структуры страницы
            await self._analyze_page_structure(page)
            
            # 2. Тестирование селекторов
            await self._test_selectors(page)
            
            # 3. Проверка защиты
            await self._check_protection(page)
            
            # 4. Анализ JavaScript
            await self._analyze_javascript(page)
            
            # 5. Вытягивание событий
            await self._extract_events(page)
            
            # 6. Анализ API вызовов
            self.results['api_calls'] = api_calls[:20]  # Первые 20 для анализа
            
            await browser.close()
        
        return self.results
    
    async def _analyze_page_structure(self, page):
        """Анализ DOM структуры страницы"""
        logger.info("\n[1/5] Analyzing page structure...")
        
        # Получаем общую структуру
        structure = await page.evaluate('''() => {
            return {
                title: document.title,
                url: window.location.href,
                html_size: document.documentElement.outerHTML.length,
                body_classes: document.body.className,
                meta_tags: Array.from(document.querySelectorAll('meta')).map(m => ({
                    name: m.getAttribute('name'),
                    content: m.getAttribute('content')?.substring(0, 100)
                })),
                scripts: document.querySelectorAll('script').length,
                iframes: document.querySelectorAll('iframe').length,
            };
        }''')
        
        logger.info(f"  Title: {structure['title']}")
        logger.info(f"  HTML size: {structure['html_size']} bytes")
        logger.info(f"  Scripts: {structure['scripts']}")
        logger.info(f"  iFrames: {structure['iframes']}")
        
        # Найдем основные контейнеры
        containers = await page.evaluate('''() => {
            const result = {};
            
            // Проверяем разные варианты контейнеров
            const selectors = [
                { name: '.pinned-event', desc: 'New pinned events' },
                { name: '.event-card', desc: 'Event cards' },
                { name: '.card', desc: 'Generic cards' },
                { name: 'ww-feature-block-event-dsk', desc: 'Web component block' },
                { name: '.ww-events-info', desc: 'Events info' },
                { name: '[data-event-id]', desc: 'Data attribute events' },
                { name: '.main-event', desc: 'Main event' },
                { name: 'ww-feature-event-mini-card-dsk', desc: 'Mini card web component' },
            ];
            
            for (const sel of selectors) {
                const els = document.querySelectorAll(sel.name);
                if (els.length > 0) {
                    result[sel.name] = {
                        count: els.length,
                        description: sel.desc,
                        first_html: els[0]?.outerHTML?.substring(0, 200),
                        classes: Array.from(els[0]?.classList || []),
                        data_attrs: Array.from(els[0]?.attributes || [])
                            .filter(a => a.name.startsWith('data-'))
                            .map(a => ({ name: a.name, value: a.value?.substring(0, 50) }))
                    };
                }
            }
            
            return result;
        }''')
        
        for sel, data in containers.items():
            logger.info(f"  {sel}: {data['count']} elements found")
        
        self.results['page_structure'] = {
            'page_info': structure,
            'containers': containers,
        }
    
    async def _test_selectors(self, page):
        """Тестирование всех селекторов"""
        logger.info("\n[2/5] Testing CSS selectors...")
        
        selectors_to_test = [
            # Новые селекторы
            ('.pinned-event', 'Live pinned events'),
            ('.event-card', 'Event card structure'),
            ('.pinned-event__team', 'Team names in pinned'),
            ('.pinned-event__match', 'Match info in pinned'),
            
            # Коэффициенты
            ('.coefficient-button', 'Coefficient button (primary)'),
            ('.coefficient-button_fill', 'Filled coefficient'),
            ('.coeffs-wrapper', 'Coefficients wrapper'),
            ('.card__coeffs', 'Card coefficients'),
            
            # Старые селекторы
            ('ww-feature-block-event-dsk', 'Web component block'),
            ('ww-feature-event-mini-card-dsk', 'Web component mini'),
            ('.main-event', 'Main event'),
            
            # Альтернативные
            ('[data-event-id]', 'Data attribute selector'),
            ('[href*="/stavki/event/"]', 'Event link'),
        ]
        
        results = {}
        for selector, desc in selectors_to_test:
            count = await page.locator(selector).count()
            results[selector] = {
                'count': count,
                'description': desc,
                'working': count > 0,
            }
            
            if count > 0:
                logger.info(f"  ✓ {selector}: {count} elements")
            else:
                logger.info(f"  ✗ {selector}: NOT FOUND")
        
        self.results['selector_tests'] = results
    
    async def _check_protection(self, page):
        """Проверка защиты сайта"""
        logger.info("\n[3/5] Checking site protection...")
        
        protection_info = await page.evaluate('''() => {
            return {
                robots_meta: document.querySelector('meta[name="robots"]')?.content,
                cloudflare_challenge: !!document.querySelector('script[src*="challenge"]'),
                rate_limit_headers: document.querySelectorAll('script').length > 100,
                captcha_present: !!document.querySelector('[data-sitekey], .g-recaptcha, .h-captcha'),
                blocked_elements: Array.from(document.querySelectorAll('*')).filter(el => 
                    el.style.display === 'none' || el.style.visibility === 'hidden'
                ).length,
                javascript_disabled: !document.body,
                web_components: Array.from(document.querySelectorAll('*'))
                    .filter(el => el.tagName.includes('-'))
                    .map(el => el.tagName)
                    .filter((v, i, a) => a.indexOf(v) === i)
            };
        }''')
        
        logger.info(f"  Robots meta: {protection_info['robots_meta']}")
        logger.info(f"  Cloudflare challenge: {protection_info['cloudflare_challenge']}")
        logger.info(f"  Web components: {protection_info['web_components']}")
        logger.info(f"  Captcha: {protection_info['captcha_present']}")
        
        self.results['protection'] = protection_info
    
    async def _analyze_javascript(self, page):
        """Анализ JavaScript кода"""
        logger.info("\n[4/5] Analyzing JavaScript...")
        
        js_info = await page.evaluate('''() => {
            return {
                angular: !!window.angular,
                react: !!window.React || !!window.__REACT,
                vue: !!window.Vue,
                jquery: !!window.jQuery || !!window.$,
                app_state: typeof window.__INITIAL_STATE__ !== 'undefined' ? 'present' : 'missing',
                event_data: Object.keys(window).filter(k => 
                    k.toLowerCase().includes('event') || 
                    k.toLowerCase().includes('sport') ||
                    k.toLowerCase().includes('data')
                ).slice(0, 10),
                fetch_calls: !!window.fetch,
                websocket: !!window.WebSocket,
            };
        }''')
        
        for key, value in js_info.items():
            logger.info(f"  {key}: {value}")
        
        self.results['javascript_analysis'] = js_info
    
    async def _extract_events(self, page):
        """Вытягивание событий с помощью разных методов"""
        logger.info("\n[5/5] Extracting events...")
        
        # Метод 1: Прямой парсинг DOM (новые селекторы)
        events_method1 = await page.evaluate('''() => {
            const events = [];
            
            // Метод 1: .pinned-event
            document.querySelectorAll('.pinned-event').forEach(el => {
                const home = el.querySelector('.pinned-event__team:first-child')?.textContent?.trim();
                const away = el.querySelector('.pinned-event__team:last-child')?.textContent?.trim();
                const link = el.querySelector('a[href*="/stavki/event/"]')?.href;
                const id = link?.match(/\/event\/(\d+)/)?.[1];
                const odds = Array.from(el.querySelectorAll('.coefficient-button')).map(o => 
                    ({ text: o.textContent?.trim(), class: o.className })
                );
                
                if (home && away) {
                    events.push({
                        home, away, id, link, odds_count: odds.length, method: 'pinned-event'
                    });
                }
            });
            
            return events;
        }''')
        
        logger.info(f"  Method 1 (.pinned-event): {len(events_method1)} events")
        if events_method1:
            logger.info(f"    Sample: {events_method1[0]}")
        
        # Метод 2: Старые селекторы
        events_method2 = await page.evaluate('''() => {
            const events = [];
            
            document.querySelectorAll('ww-feature-block-event-dsk').forEach(el => {
                const text = el.textContent || '';
                const link = el.querySelector('a[href*="/stavki/event/"]')?.href;
                const id = link?.match(/\/event\/(\d+)/)?.[1];
                
                if (link) {
                    events.push({
                        text: text.substring(0, 50), link, id, method: 'ww-feature-block-event-dsk'
                    });
                }
            });
            
            return events;
        }''')
        
        logger.info(f"  Method 2 (legacy): {len(events_method2)} events")
        
        # Метод 3: Альтернативный поиск по data-атрибутам
        events_method3 = await page.evaluate('''() => {
            const events = [];
            
            document.querySelectorAll('[data-event-id]').forEach(el => {
                const id = el.getAttribute('data-event-id');
                const link = el.querySelector('a[href*="/stavki/event/"]')?.href;
                events.push({ id, link, method: 'data-event-id' });
            });
            
            return events;
        }''')
        
        logger.info(f"  Method 3 (data attr): {len(events_method3)} events")
        
        all_events = events_method1 + events_method2 + events_method3
        # Дедупликация
        unique_events = {e.get('id', e.get('link', '')): e for e in all_events}.values()
        
        logger.info(f"\n  TOTAL UNIQUE EVENTS: {len(unique_events)}")
        
        self.results['events_found'] = list(unique_events)[:50]  # Первые 50


async def main():
    logger.info("=" * 60)
    logger.info("WINLINE DEEP STRUCTURE ANALYSIS")
    logger.info("=" * 60)
    
    analyzer = WinlineAnalyzer()
    results = await analyzer.analyze()
    
    # Сохраняем результаты
    with open('winline_deep_analysis.json', 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    
    logger.info("\n" + "=" * 60)
    logger.info("ANALYSIS COMPLETE")
    logger.info(f"Results saved to: winline_deep_analysis.json")
    logger.info("=" * 60)
    
    # Выводим итоги
    print("\n[SUMMARY]")
    print(f"Events found: {len(results['events_found'])}")
    print(f"Working selectors: {sum(1 for s in results['selector_tests'].values() if s['working'])}")
    print(f"Protection detected: {results['protection'].get('captcha_present', False)}")
    
    return results


if __name__ == '__main__':
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())
    
    asyncio.run(main())
