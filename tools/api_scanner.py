"""
BK API Auto Scanner
Автоматически сканирует букмекеров на наличие скрытых API эндпоинтов.
Не требует браузера — работает напрямую с HTTP.
"""

import asyncio
import aiohttp
import json
import time
import logging
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Any
from urllib.parse import urljoin

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Известные API паттерны букмекеров
KNOWN_API_PATTERNS = {
    "fonbet": [
        "https://fonbet.win/api/federation/v1/get-contents",
        "https://fonbet.win/api/live-events/v2/live",
        "https://fonbet.win/api/line-events/v2/line",
        "https://line62-bg453309.bk6448348.com/federation/v1/get-contents",
    ],
    "pari": [
        "https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300",
        "https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300",
    ],
    "bettery": [
        "https://line51.at58f5-resources.com/events/list?lang=ru&scopeMarket=501",
        "https://line51.at58f5-resources.com/events/listBase?lang=ru&scopeMarket=501",
    ],
    "marathon": [
        "https://line51.tf39be-resources.com/events/list?lang=ru&scopeMarket=3000",
        "https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000",
    ],
    "winline": [
        "https://winline.ru/api/betline/events",
        "https://winline.ru/api/line",
    ],
    "betcity": [
        "https://betcity.ru/api/line",
        "https://betcity.ru/api/events",
    ],
    "zenit": [
        "https://zenit.win/api/line",
        "https://zenit.win/api/events",
    ],
    "baltbet": [
        "https://baltbet.ru/api/line",
        "https://baltbet.ru/api/events",
    ],
    "leon": [
        "https://line51.leon.ru/events/list?lang=ru",
        "https://line51.leon.ru/events/listBase?lang=ru",
    ],
    "24bet": [
        "https://line51.tf39be-resources.com/events/list?lang=ru&scopeMarket=3000",
    ],
    "sportbet": [
        "https://sportbet.ru/api/line",
    ]
}

# Дополнительные паттерны для поиска
COMMON_PATTERNS = [
    "/api/events",
    "/api/line",
    "/api/live",
    "/api/odds",
    "/api/matches",
    "/api/v1/events",
    "/api/v2/events",
    "/events/list",
    "/events/listBase",
    "/federation/v1/get-contents",
    "/line-events/v2/line",
    "/live-events/v2/live",
]

class BKApiScanner:
    def __init__(self):
        self.results = {}
        self.output_dir = Path("discovery_output")
        self.output_dir.mkdir(exist_ok=True)
    
    async def scan_url(self, session: aiohttp.ClientSession, url: str, bk_name: str) -> dict:
        """Сканирует один URL"""
        try:
            headers = {
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                "Accept": "application/json, text/plain, */*",
                "Accept-Language": "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7",
                "Origin": url.split("/")[0] + "//" + url.split("/")[2],
                "Referer": url,
            }
            
            async with session.get(url, headers=headers, timeout=aiohttp.ClientTimeout(total=10)) as response:
                if response.status == 200:
                    content_type = response.headers.get("content-type", "")
                    
                    if "json" in content_type:
                        try:
                            data = await response.json()
                            return {
                                "url": url,
                                "bk": bk_name,
                                "status": 200,
                                "content_type": content_type,
                                "size": len(str(data)),
                                "data_sample": self._get_sample(data),
                                "structure": self._analyze_structure(data)
                            }
                        except:
                            text = await response.text()
                            return {
                                "url": url,
                                "bk": bk_name,
                                "status": 200,
                                "content_type": content_type,
                                "size": len(text),
                                "data_sample": text[:500],
                                "structure": "text/html"
                            }
                    else:
                        return {
                            "url": url,
                            "bk": bk_name,
                            "status": 200,
                            "content_type": content_type,
                            "size": 0,
                            "data_sample": None,
                            "structure": "non-json"
                        }
                else:
                    return {
                        "url": url,
                        "bk": bk_name,
                        "status": response.status,
                        "content_type": response.headers.get("content-type", ""),
                        "size": 0,
                        "data_sample": None,
                        "structure": f"status_{response.status}"
                    }
        except Exception as e:
            return {
                "url": url,
                "bk": bk_name,
                "status": 0,
                "content_type": "",
                "size": 0,
                "data_sample": str(e)[:200],
                "structure": "error"
            }
    
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
        """Сканирует все известные URL букмекера"""
        urls = KNOWN_API_PATTERNS.get(bk_name, [])
        
        if not urls:
            # Генерируем URL из общих паттернов
            base_url = f"https://{bk_name}.ru"
            urls = [urljoin(base_url, p) for p in COMMON_PATTERNS]
        
        logger.info(f"Scanning {bk_name}: {len(urls)} URLs...")
        
        async with aiohttp.ClientSession() as session:
            tasks = [self.scan_url(session, url, bk_name) for url in urls]
            results = await asyncio.gather(*tasks, return_exceptions=True)
        
        # Фильтруем успешные JSON ответы
        json_results = [r for r in results if isinstance(r, dict) and r.get("status") == 200 and "json" in r.get("content_type", "")]
        
        self.results[bk_name] = {
            "total_urls": len(urls),
            "json_urls": len(json_results),
            "results": json_results
        }
        
        logger.info(f"{bk_name}: {len(json_results)}/{len(urls)} URLs returned JSON")
        
        # Сохраняем результаты
        bk_dir = self.output_dir / bk_name
        bk_dir.mkdir(exist_ok=True)
        
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        
        # Сохраняем summary
        summary_file = bk_dir / f"scan_summary_{timestamp}.json"
        with open(summary_file, "w", encoding="utf-8") as f:
            json.dump({
                "bk": bk_name,
                "timestamp": timestamp,
                "total_urls": len(urls),
                "json_urls": len(json_results),
                "endpoints": [r["url"] for r in json_results]
            }, f, indent=2, ensure_ascii=False)
        
        # Сохраняем примеры JSON
        for i, result in enumerate(json_results):
            if result.get("data_sample") and isinstance(result["data_sample"], dict):
                json_file = bk_dir / f"endpoint_{i}_{timestamp}.json"
                with open(json_file, "w", encoding="utf-8") as f:
                    json.dump(result["data_sample"], f, indent=2, ensure_ascii=False)
        
        return self.results[bk_name]
    
    async def scan_all(self):
        """Сканирует все известные БК"""
        for bk_name in KNOWN_API_PATTERNS.keys():
            try:
                await self.scan_bk(bk_name)
                await asyncio.sleep(1)  # Пауза между БК
            except Exception as e:
                logger.error(f"Error scanning {bk_name}: {e}")
        
        # Итоговый отчет
        report_file = self.output_dir / f"scan_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(report_file, "w", encoding="utf-8") as f:
            json.dump(self.results, f, indent=2, ensure_ascii=False)
        
        logger.info(f"Scan complete! Report saved to {report_file}")
        
        # Выводим итоги
        print("\n" + "="*60)
        print("BK API SCAN RESULTS")
        print("="*60)
        for bk, data in self.results.items():
            status = "✅" if data["json_urls"] > 0 else "❌"
            print(f"{status} {bk}: {data['json_urls']}/{data['total_urls']} JSON endpoints")
            for result in data.get("results", [])[:3]:
                print(f"   - {result['url']} ({result['size']} bytes)")
        print("="*60)


async def main():
    scanner = BKApiScanner()
    await scanner.scan_all()


if __name__ == "__main__":
    asyncio.run(main())
