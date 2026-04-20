#!/usr/bin/env python3
"""
Проверка доступности основных БК для парсинга
"""
import asyncio
import aiohttp
import json
from datetime import datetime

BOOKMAKERS = {
    "marathonbet": "https://marathonbet.com",
    "betcity": "https://betcity.com",
    "1xstavka": "https://1xstavka.ru",
    "fonbet": "https://fonbet.ru",
    "melbet": "https://melbet.com",
    "sportbet": "https://sportbet.com.ua",
    "leon": "https://leon.bet",
    "pari": "https://pari.ru",
    "baltbet": "https://baltbet.ru",
    "olimp": "https://olimp.bet",
    "olimpbet": "https://olimpbet.com",
    "betboom": "https://betboom.ru",
    "zenit": "https://zenit.bet"
}

async def check_bookmaker(session, name, url):
    """Проверяет доступность БК и получает информацию"""
    try:
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        }
        
        async with session.get(url, timeout=aiohttp.ClientTimeout(total=15), 
                             headers=headers, ssl=False, allow_redirects=True) as resp:
            status = resp.status
            content_type = resp.headers.get('content-type', '')
            
            # Пытаемся получить HTML для подсчета событий
            html = await resp.text(errors='ignore')
            
            # Считаем потенциальные события (простая проверка)
            event_indicators = html.count('match') + html.count('event') + \
                             html.count('fixture') + html.count('match-') + \
                             html.count('event-id')
            
            return {
                'name': name,
                'url': url,
                'status': status,
                'content_type': content_type,
                'html_size': len(html),
                'event_indicators': event_indicators,
                'available': status == 200 and len(html) > 5000
            }
    except Exception as e:
        return {
            'name': name,
            'url': url,
            'status': 'ERROR',
            'error': str(e),
            'available': False
        }

async def main():
    """Проверяет все БК"""
    print("=" * 70)
    print("ПРОВЕРКА ДОСТУПНОСТИ БУКМЕКЕРОВ")
    print("=" * 70)
    print(f"Начало проверки: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
    
    async with aiohttp.ClientSession() as session:
        tasks = [check_bookmaker(session, name, url) 
                for name, url in BOOKMAKERS.items()]
        results = await asyncio.gather(*tasks)
    
    # Сортируем по доступности
    available = [r for r in results if r.get('available')]
    unavailable = [r for r in results if not r.get('available')]
    
    print("\n✅ ДОСТУПНЫЕ БУКМЕКЕРЫ:")
    print("-" * 70)
    for bk in sorted(available, key=lambda x: x['html_size'], reverse=True):
        print(f"{bk['name']:15} | Status: {bk['status']} | Size: {bk['html_size']:6} | Events: {bk.get('event_indicators', 0):4}")
    
    print("\n❌ НЕДОСТУПНЫЕ БУКМЕКЕРЫ:")
    print("-" * 70)
    for bk in unavailable:
        status = bk.get('status', 'UNKNOWN')
        error = bk.get('error', '')
        if error:
            print(f"{bk['name']:15} | {status} | {error[:40]}")
        else:
            print(f"{bk['name']:15} | {status}")
    
    print("\n" + "=" * 70)
    print(f"Всего проверено: {len(results)} | Доступно: {len(available)} | Недоступно: {len(unavailable)}")
    print("=" * 70)
    
    # Сохраняем результаты
    with open('bookmakers_status.json', 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    
    print("\n📄 Результаты сохранены в bookmakers_status.json")

if __name__ == '__main__':
    asyncio.run(main())
