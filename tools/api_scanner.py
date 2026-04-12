"""
BK API Auto Scanner with Playwright
Automatically scans bookmakers for hidden API endpoints using browser automation.
Uses Playwright to simulate user interactions and intercept XHR requests.
"""

import asyncio
import json
import logging
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Any, Set
from urllib.parse import urljoin
from playwright.async_api import async_playwright

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Bookmaker configurations for browser scanning
BOOKMAKERS = {
    "fonbet": {
        "main_url": "https://fonbet.ru",
        "live_url": "https://fonbet.ru/live",
        "line_url": "https://fonbet.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "pari": {
        "main_url": "https://pari.ru",
        "live_url": "https://pari.ru/live",
        "line_url": "https://pari.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "bettery": {
        "main_url": "https://bettery.ru",
        "live_url": "https://bettery.ru/live",
        "line_url": "https://bettery.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "marathon": {
        "main_url": "https://marathonbet.com",
        "live_url": "https://marathonbet.com/ru/live",
        "line_url": "https://marathonbet.com/ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "winline": {
        "main_url": "https://winline.ru",
        "live_url": "https://winline.ru/live",
        "line_url": "https://winline.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "betcity": {
        "main_url": "https://betcity.ru",
        "live_url": "https://betcity.ru/live",
        "line_url": "https://betcity.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "zenit": {
        "main_url": "https://zenit.ru",
        "live_url": "https://zenit.ru/live",
        "line_url": "https://zenit.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "baltbet": {
        "main_url": "https://baltbet.ru",
        "live_url": "https://baltbet.ru/live",
        "line_url": "https://baltbet.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "leon": {
        "main_url": "https://leon.ru",
        "live_url": "https://leon.ru/live",
        "line_url": "https://leon.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "24bet": {
        "main_url": "https://24betting.ru",
        "live_url": "https://24betting.ru/live",
        "line_url": "https://24betting.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    },
    "sportbet": {
        "main_url": "https://sportbet.ru",
        "live_url": "https://sportbet.ru/live",
        "line_url": "https://sportbet.ru/line",
        "sports": ["Футбол", "Баскетбол", "Хоккей", "Теннис", "Волейбол", "Football", "Basketball", "Hockey", "Tennis", "Volleyball"]
    }
}

class BKApiScanner:
    def __init__(self):
        self.results = {}
        self.output_dir = Path("discovery_output")
        self.output_dir.mkdir(exist_ok=True)

    async def scan_bk_with_browser(self, bk_name: str, config: dict) -> dict:
        """Scan bookmaker using browser automation"""
        logger.info(f"Scanning {bk_name} with browser...")

        live_endpoints = set()
        prematch_endpoints = set()
        general_endpoints = set()
        captured_responses = []

        async with async_playwright() as p:
            browser = await p.chromium.launch(
                headless=True,
                args=[
                    '--disable-blink-features=AutomationControlled',
                    '--no-sandbox',
                    '--disable-dev-shm-usage',
                    '--disable-web-security',
                    '--disable-features=IsolateOrigins,site-per-process',
                    '--disable-infobars',
                ]
            )

            context = await browser.new_context(
                viewport={'width': 1920, 'height': 1080},
                user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
                locale='ru-RU',
            )

            page = await context.new_page()

            # Track current page type
            current_page_type = "general"

            # Intercept responses
            async def on_response(response):
                try:
                    url = response.url
                    content_type = response.headers.get('content-type', '')

                    # Check if it's a JSON API response
                    if (response.status == 200 and
                        ('application/json' in content_type or
                         'text/json' in content_type or
                         any(keyword in url.lower() for keyword in ['api', 'line', 'event', 'odds', 'bet', 'factor', 'coeff', 'sport', 'federation'])) and
                        not any(skip in url.lower() for skip in ['google', 'facebook', 'vk.com', 'yandex', 'analytics', 'tracking', 'advertisement'])):

                        try:
                            text = await response.text()
                            if len(text) > 100:  # Skip small responses
                                data = json.loads(text)
                                sample = self._get_sample(data)
                                structure = self._analyze_structure(data)

                                endpoint_info = {
                                    "url": url,
                                    "method": response.request.method,
                                    "content_type": content_type,
                                    "size": len(text),
                                    "data_sample": sample,
                                    "structure": structure,
                                    "page_type": current_page_type,
                                    "timestamp": datetime.now().isoformat()
                                }

                                captured_responses.append(endpoint_info)

                                # Add to appropriate set
                                if current_page_type == "live":
                                    live_endpoints.add(url)
                                elif current_page_type == "prematch":
                                    prematch_endpoints.add(url)
                                else:
                                    general_endpoints.add(url)

                                logger.info(f"  [{current_page_type.upper()}] Captured API: {url[:80]}... ({len(text)} bytes)")
                        except json.JSONDecodeError:
                            pass
                        except Exception as e:
                            logger.debug(f"  Error processing response: {e}")

                except Exception as e:
                    logger.debug(f"  Response handler error: {e}")

            page.on('response', on_response)

            try:
                # Visit main page and wait for JS
                logger.info(f"  Visiting main page: {config['main_url']}")
                current_page_type = "general"
                await page.goto(config['main_url'], wait_until='domcontentloaded', timeout=30000)
                await page.wait_for_load_state('networkidle', timeout=10000)
                await asyncio.sleep(3)

                # Scan live page separately
                logger.info(f"  Navigating to live page: {config['live_url']}")
                current_page_type = "live"
                await page.goto(config['live_url'], wait_until='domcontentloaded', timeout=30000)
                await page.wait_for_load_state('networkidle', timeout=10000)
                await asyncio.sleep(2)

                # Click on sports tabs for live
                await self._click_sports_tabs(page, config['sports'], wait_time=3)

                # Wait longer for live API calls
                await asyncio.sleep(8)

                # Scan prematch line page separately
                logger.info(f"  Navigating to line page: {config['line_url']}")
                current_page_type = "prematch"
                await page.goto(config['line_url'], wait_until='domcontentloaded', timeout=30000)
                await page.wait_for_load_state('networkidle', timeout=10000)
                await asyncio.sleep(2)

                # Click on sports tabs for prematch
                await self._click_sports_tabs(page, config['sports'], wait_time=3)

                # Wait longer for prematch API calls
                await asyncio.sleep(8)

            except Exception as e:
                logger.error(f"  Error during scanning {bk_name}: {e}")

            await browser.close()

        # Combine all endpoints for backward compatibility
        all_endpoints = live_endpoints | prematch_endpoints | general_endpoints

        result = {
            "total_endpoints": len(all_endpoints),
            "endpoints": list(all_endpoints),
            "live_endpoints": list(live_endpoints),
            "prematch_endpoints": list(prematch_endpoints),
            "general_endpoints": list(general_endpoints),
            "captured_responses": captured_responses
        }

        logger.info(f"{bk_name}: captured {len(live_endpoints)} live, {len(prematch_endpoints)} prematch, {len(general_endpoints)} general API endpoints")
        return result

    async def _click_sports_tabs(self, page, sports: list, wait_time: int = 1):
        """Try to click on sports tabs to trigger API calls"""
        for sport in sports:
            try:
                # Try different selectors for sport tabs
                selectors = [
                    f"text={sport}",
                    f"[data-sport='{sport.lower()}']",
                    f"a:has-text('{sport}')",
                    f"button:has-text('{sport}')",
                    f"div:has-text('{sport}')",
                    f"span:has-text('{sport}')",
                    f"li:has-text('{sport}')"
                ]

                for selector in selectors:
                    try:
                        elements = page.locator(selector)
                        count = await elements.count()
                        if count > 0:
                            await elements.first.click()
                            logger.info(f"    Clicked on {sport} tab")
                            await asyncio.sleep(wait_time)
                            break
                    except:
                        continue

            except Exception as e:
                logger.debug(f"    Error clicking {sport}: {e}")

    def _get_sample(self, data: Any) -> dict:
        """Получает пример структуры данных"""
        if isinstance(data, dict):
            return {k: str(v)[:100] if not isinstance(v, (dict, list)) else f"<{type(v).__name__}>" for k, v in list(data.items())[:10]}
        elif isinstance(data, list):
            return {"type": "list", "length": len(data), "first_item": str(data[0])[:200] if data else None}
        return {"type": type(data).__name__, "value": str(data)[:200]}

    def _analyze_structure(self, data: Any) -> dict:
        """Анализирует структуру JSON для генерации парсера"""
        if isinstance(data, dict):
            keys = list(data.keys())
            nested = {k: self._analyze_structure(v) for k, v in list(data.items())[:5] if isinstance(v, (dict, list))}
            return {"type": "object", "keys": keys, "nested": nested}
        elif isinstance(data, list):
            if data and isinstance(data[0], dict):
                return {"type": "array", "item_keys": list(data[0].keys())[:15]}
            return {"type": "array", "length": len(data)}
        return {"type": type(data).__name__}
    
    async def scan_bk(self, bk_name: str):
        """Scan bookmaker using browser automation"""
        config = BOOKMAKERS.get(bk_name)
        if not config:
            logger.warning(f"No config found for {bk_name}")
            return {"total_endpoints": 0, "endpoints": [], "captured_responses": []}

        result = await self.scan_bk_with_browser(bk_name, config)

        # Save results
        bk_dir = self.output_dir / bk_name
        bk_dir.mkdir(exist_ok=True)

        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

        # Save summary
        summary_file = bk_dir / f"scan_summary_{timestamp}.json"
        with open(summary_file, "w", encoding="utf-8") as f:
            json.dump({
                "bk": bk_name,
                "timestamp": timestamp,
                "total_endpoints": result["total_endpoints"],
                "endpoints": result["endpoints"],
                "live_endpoints": result["live_endpoints"],
                "prematch_endpoints": result["prematch_endpoints"],
                "general_endpoints": result["general_endpoints"]
            }, f, indent=2, ensure_ascii=False)

        # Save live endpoints
        if result["live_endpoints"]:
            live_file = bk_dir / f"live_endpoints_{timestamp}.json"
            with open(live_file, "w", encoding="utf-8") as f:
                json.dump({
                    "bk": bk_name,
                    "type": "live",
                    "timestamp": timestamp,
                    "endpoints": result["live_endpoints"]
                }, f, indent=2, ensure_ascii=False)

        # Save prematch endpoints
        if result["prematch_endpoints"]:
            prematch_file = bk_dir / f"prematch_endpoints_{timestamp}.json"
            with open(prematch_file, "w", encoding="utf-8") as f:
                json.dump({
                    "bk": bk_name,
                    "type": "prematch",
                    "timestamp": timestamp,
                    "endpoints": result["prematch_endpoints"]
                }, f, indent=2, ensure_ascii=False)

        # Save captured responses
        responses_file = bk_dir / f"captured_responses_{timestamp}.json"
        with open(responses_file, "w", encoding="utf-8") as f:
            json.dump(result["captured_responses"], f, indent=2, ensure_ascii=False)

        self.results[bk_name] = result
        return result
    
    async def scan_all(self):
        """Scan all bookmakers in parallel"""
        bk_names = list(BOOKMAKERS.keys())
        logger.info(f"Scanning {len(bk_names)} bookmakers in parallel...")

        # Run scans in parallel with semaphore to limit concurrent browsers
        semaphore = asyncio.Semaphore(6)  # Limit to 6 concurrent browsers for swarm mode

        async def scan_with_semaphore(bk_name):
            async with semaphore:
                try:
                    return await self.scan_bk(bk_name)
                except Exception as e:
                    logger.error(f"Error scanning {bk_name}: {e}")
                    return {"total_endpoints": 0, "endpoints": [], "live_endpoints": [], "prematch_endpoints": [], "general_endpoints": [], "captured_responses": []}

        tasks = [scan_with_semaphore(bk_name) for bk_name in bk_names]
        results = await asyncio.gather(*tasks, return_exceptions=True)

        # Store results
        for bk_name, result in zip(bk_names, results):
            if isinstance(result, dict):
                self.results[bk_name] = result

        # Save final report
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        report_file = self.output_dir / f"scan_report_{timestamp}.json"
        with open(report_file, "w", encoding="utf-8") as f:
            json.dump(self.results, f, indent=2, ensure_ascii=False)

        logger.info(f"Scan complete! Report saved to {report_file}")

        # Print summary
        print("\n" + "="*80)
        print("BK API SCAN RESULTS (Browser Automation - Enhanced)")
        print("="*80)
        total_endpoints = 0
        total_live = 0
        total_prematch = 0
        for bk, data in self.results.items():
            endpoints_count = data.get("total_endpoints", 0)
            live_count = len(data.get("live_endpoints", []))
            prematch_count = len(data.get("prematch_endpoints", []))
            total_endpoints += endpoints_count
            total_live += live_count
            total_prematch += prematch_count
            status = "✅" if endpoints_count > 0 else "❌"
            print(f"{status} {bk}: {endpoints_count} total ({live_count} live, {prematch_count} prematch) API endpoints")
            if live_count > 0:
                print("  LIVE endpoints:")
                for endpoint in data.get("live_endpoints", [])[:2]:
                    print(f"   - {endpoint}")
                if len(data.get("live_endpoints", [])) > 2:
                    print(f"   ... and {len(data.get('live_endpoints', [])) - 2} more live")
            if prematch_count > 0:
                print("  PREMATCH endpoints:")
                for endpoint in data.get("prematch_endpoints", [])[:2]:
                    print(f"   - {endpoint}")
                if len(data.get("prematch_endpoints", [])) > 2:
                    print(f"   ... and {len(data.get('prematch_endpoints', [])) - 2} more prematch")
        print("="*80)
        print(f"Total unique API endpoints discovered: {total_endpoints}")
        print(f"  - Live endpoints: {total_live}")
        print(f"  - Prematch endpoints: {total_prematch}")
        print(f"Results saved in: {self.output_dir}")


async def main():
    print("="*80)
    print("BK API SCANNER WITH PLAYWRIGHT")
    print("Automatically discovers bookmaker APIs using browser automation")
    print("="*80)

    scanner = BKApiScanner()
    await scanner.scan_all()


if __name__ == "__main__":
    asyncio.run(main())
