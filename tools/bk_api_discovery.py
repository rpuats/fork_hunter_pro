"""
BK API Discovery Tool
Перехватывает и логирует все API запросы букмекерских сайтов.
Помогает находить скрытые эндпоинты и структуру данных.

Использование:
  python bk_discovery.py --bk fonbet --url https://fonbet.win
  python bk_discovery.py --bk pari --url https://pari.ru
  python bk_discovery.py --list  # Список поддерживаемых БК
"""

import os
import sys
import json
import time
import asyncio
import logging
from datetime import datetime
from pathlib import Path
from urllib.parse import urlparse
from collections import defaultdict

# Настройка логирования
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Конфигурация букмекеров
BK_CONFIGS = {
    "fonbet": {
        "urls": ["https://fonbet.win", "https://fonbet.ru"],
        "api_patterns": ["api", "line", "live", "events", "odds", "federation"],
        "cookies_required": True
    },
    "pari": {
        "urls": ["https://pari.ru", "https://line-lb01-w.pb06e2-resources.com"],
        "api_patterns": ["events/list", "events/detail", "factors", "markets"],
        "cookies_required": False
    },
    "winline": {
        "urls": ["https://winline.ru"],
        "api_patterns": ["api", "betline", "events", "odds"],
        "cookies_required": True
    },
    "betcity": {
        "urls": ["https://betcity.ru"],
        "api_patterns": ["api", "line", "events", "coefficients"],
        "cookies_required": True
    },
    "zenit": {
        "urls": ["https://zenit.win"],
        "api_patterns": ["api", "line", "events", "odds"],
        "cookies_required": True
    },
    "baltbet": {
        "urls": ["https://baltbet.ru"],
        "api_patterns": ["api", "line", "events", "odds"],
        "cookies_required": True
    },
    "marathon": {
        "urls": ["https://marathonbet.com"],
        "api_patterns": ["api", "events", "odds", "live"],
        "cookies_required": True
    },
    "bettery": {
        "urls": ["https://bettery.ru"],
        "api_patterns": ["events/list", "events/listBase"],
        "cookies_required": False
    }
}

class BKApiDiscovery:
    def __init__(self, bk_name: str, output_dir: str = "discovery_output"):
        self.bk_name = bk_name
        self.output_dir = Path(output_dir) / bk_name
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        self.captured_requests = []
        self.api_endpoints = defaultdict(list)
        self.json_responses = {}
        
        # Загружаем конфиг
        self.config = BK_CONFIGS.get(bk_name, {
            "urls": [f"https://{bk_name}.ru"],
            "api_patterns": ["api", "data"],
            "cookies_required": True
        })
    
    def is_api_request(self, url: str) -> bool:
        """Проверяет, является ли запрос API вызовом"""
        url_lower = url.lower()
        return any(pattern in url_lower for pattern in self.config["api_patterns"])
    
    def save_request(self, request_data: dict):
        """Сохраняет перехваченный запрос"""
        url = request_data.get("url", "")
        
        if self.is_api_request(url):
            # Классифицируем запрос
            parsed = urlparse(url)
            endpoint_key = f"{parsed.path}"
            
            self.api_endpoints[endpoint_key].append({
                "timestamp": request_data.get("timestamp", ""),
                "method": request_data.get("method", "GET"),
                "url": url,
                "headers": request_data.get("headers", {}),
                "response_status": request_data.get("status", 0),
                "response_size": len(str(request_data.get("response", "")))
            })
            
            # Сохраняем примеры JSON ответов
            if "json" in request_data.get("content_type", "") and request_data.get("response"):
                response_key = f"{endpoint_key.replace('/', '_')}"
                if response_key not in self.json_responses:
                    self.json_responses[response_key] = request_data.get("response")
        
        self.captured_requests.append(request_data)
    
    def export_results(self):
        """Экспортирует результаты в файлы"""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        
        # 1. Сохраняем список эндпоинтов
        endpoints_file = self.output_dir / f"api_endpoints_{timestamp}.json"
        with open(endpoints_file, "w", encoding="utf-8") as f:
            json.dump(dict(self.api_endpoints), f, indent=2, ensure_ascii=False)
        logger.info(f"Saved {len(self.api_endpoints)} API endpoints to {endpoints_file}")
        
        # 2. Сохраняем примеры JSON
        json_dir = self.output_dir / "json_examples"
        json_dir.mkdir(exist_ok=True)
        
        for name, data in self.json_responses.items():
            json_file = json_dir / f"{name}.json"
            with open(json_file, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
        logger.info(f"Saved {len(self.json_responses)} JSON examples to {json_dir}")
        
        # 3. Генерируем шаблон парсера
        template_file = self.output_dir / f"parser_template_{self.bk_name}.py"
        self._generate_parser_template(template_file)
        logger.info(f"Generated parser template: {template_file}")
    
    def _generate_parser_template(self, filepath: Path):
        """Генерирует шаблон парсера на основе найденных эндпоинтов"""
        template = f'''"""
Автоматически сгенерированный парсер для {self.bk_name.upper()}
Сгенерировано: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}
"""

import json
import asyncio
import aiohttp
from typing import List, Dict, Any

class {self.bk_name.capitalize()}Parser:
    """Парсер {self.bk_name.upper()}"""
    
    BASE_URL = "{self.config['urls'][0]}"
    
    # Найденные API эндпоинты:
    # {chr(10).join(f'#   - {ep} ({len(reqs)} requests)' for ep, reqs in self.api_endpoints.items())}
    
    async def fetch_events(self) -> List[Dict[str, Any]]:
        """Получает список событий"""
        # TODO: Реализовать на основе найденных эндпоинтов
        pass
    
    async def fetch_odds(self, event_id: str) -> Dict[str, Any]:
        """Получает коэффициенты для события"""
        # TODO: Реализовать на основе найденных эндпоинтов
        pass
    
    def parse_events(self, data: Dict) -> List[Dict[str, Any]]:
        """Парсит JSON ответ в список событий"""
        events = []
        # TODO: Анализ структуры JSON из папки json_examples/
        return events
    
    def parse_odds(self, data: Dict) -> Dict[str, Any]:
        """Парсит JSON ответ в коэффициенты"""
        odds = {{}}
        # TODO: Анализ структуры JSON из папки json_examples/
        return odds


async def main():
    parser = {self.bk_name.capitalize()}Parser()
    
    # Пример использования:
    # events = await parser.fetch_events()
    # for event in events[:5]:
    #     print(event)
    
    print("Parser template generated successfully!")
    print("Next steps:")
    print("1. Изучите файлы в json_examples/")
    print("2. Заполните методы parse_events и parse_odds")
    print("3. Протестируйте парсер")


if __name__ == "__main__":
    asyncio.run(main())
'''
        
        with open(filepath, "w", encoding="utf-8") as f:
            f.write(template)

async def run_intercept(bk_name: str, url: str):
    """
    Запускает перехват запросов через mitmproxy.
    Требует установленный mitmproxy: pip install mitmproxy
    """
    try:
        from mitmproxy import http
        from mitmproxy.tools import main as mitm_main
    except ImportError:
        logger.error("mitmproxy не установлен. Установите: pip install mitmproxy")
        return
    
    discovery = BKApiDiscovery(bk_name)
    
    class CaptureAddon:
        def __init__(self, disc):
            self.discovery = disc
        
        def response(self, flow: http.HTTPFlow):
            request_data = {
                "url": flow.request.pretty_url,
                "method": flow.request.method,
                "headers": dict(flow.request.headers),
                "status": flow.response.status_code,
                "content_type": flow.response.headers.get("content-type", ""),
                "response": None,
                "timestamp": datetime.now().isoformat()
            }
            
            # Извлекаем тело ответа
            if flow.response.content:
                try:
                    content_type = flow.response.headers.get("content-type", "")
                    if "json" in content_type:
                        request_data["response"] = json.loads(flow.response.content)
                    else:
                        request_data["response"] = flow.response.text
                except:
                    pass
            
            self.discovery.save_request(request_data)
            logger.info(f"Captured: {flow.request.pretty_url} ({flow.response.status_code})")
    
    logger.info(f"Starting intercept for {bk_name} at {url}")
    logger.info("Откройте браузер по адресу http://localhost:8080")
    logger.info("Перейдите на сайт букмекера и подождите загрузки данных")
    logger.info("Для остановки нажмите Ctrl+C")
    
    try:
        await mitm_main.mitmweb(
            mode=f"regular",
            listen_port=8080,
            addons=[CaptureAddon(discovery)]
        )
    except KeyboardInterrupt:
        logger.info("Stopping intercept...")
    
    discovery.export_results()
    logger.info(f"Results saved to {discovery.output_dir}")


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="BK API Discovery Tool")
    parser.add_argument("--bk", type=str, help="Название букмекера")
    parser.add_argument("--url", type=str, help="URL сайта букмекера")
    parser.add_argument("--list", action="store_true", help="Список поддерживаемых БК")
    
    args = parser.parse_args()
    
    if args.list:
        print("Поддерживаемые букмекеры:")
        for bk, config in BK_CONFIGS.items():
            print(f"  - {bk}: {config['urls'][0]}")
        return
    
    if not args.bk:
        parser.error("Необходимо указать --bk <название>")
    
    url = args.url or BK_CONFIGS.get(args.bk, {}).get("urls", [f"https://{args.bk}.ru"])[0]
    
    asyncio.run(run_intercept(args.bk, url))


if __name__ == "__main__":
    main()
