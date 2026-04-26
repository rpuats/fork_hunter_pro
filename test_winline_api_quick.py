#!/usr/bin/env python3
"""
Быстрый тест Winline REST API - проверить что работает
"""
import requests
import json
from typing import Dict, List
import time

class WinlineAPITest:
    def __init__(self):
        self.base_url = "https://winline.ru"
        self.session = requests.Session()
        
        # Правильные headers
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36',
            'Referer': 'https://winline.ru/stavki/sport/futbol',
            'Accept': 'application/json, text/plain, */*',
            'Accept-Language': 'en-US,en;q=0.9,ru;q=0.8',
            'Accept-Encoding': 'gzip, deflate, br',
            'X-Requested-With': 'XMLHttpRequest',
            'Sec-Fetch-Dest': 'empty',
            'Sec-Fetch-Mode': 'cors',
            'Sec-Fetch-Site': 'same-origin',
            'Cache-Control': 'no-cache',
            'Pragma': 'no-cache',
        }
        self.session.headers.update(self.headers)
    
    def test_ip_endpoint(self) -> bool:
        """Тестирует /api/v2/getip"""
        print("\n[1] Testing IP endpoint...")
        try:
            resp = self.session.get(f"{self.base_url}/api/v2/getip?_format=json", timeout=10)
            print(f"  Status: {resp.status_code}")
            
            if resp.status_code == 200:
                data = resp.json()
                ip = data.get('my_ip')
                print(f"  ✓ Got IP: {ip}")
                return True
            else:
                print(f"  ✗ HTTP {resp.status_code}")
                return False
        except Exception as e:
            print(f"  ✗ Error: {e}")
            return False
    
    def test_main_page(self) -> bool:
        """Загружает основную страницу для cookies"""
        print("\n[2] Loading main page...")
        try:
            resp = self.session.get(f"{self.base_url}/stavki/sport/futbol", timeout=15)
            print(f"  Status: {resp.status_code}")
            print(f"  Cookies: {len(self.session.cookies)}")
            print(f"  Content length: {len(resp.content)} bytes")
            
            if resp.status_code == 200:
                print(f"  ✓ Page loaded")
                return True
            return False
        except Exception as e:
            print(f"  ✗ Error: {e}")
            return False
    
    def test_cls_api(self) -> bool:
        """Тестирует CLS API endpoints"""
        print("\n[3] Testing CLS API...")
        endpoints = [
            '/api/cls/menu/sport/205',
            '/api/cls/menu/sport/1',
            '/api/cls/sports',
        ]
        
        for endpoint in endpoints:
            print(f"  Trying {endpoint}...")
            try:
                resp = self.session.get(f"{self.base_url}{endpoint}", timeout=10)
                print(f"    Status: {resp.status_code}")
                
                if resp.status_code == 200:
                    content_type = resp.headers.get('content-type', '')
                    print(f"    Type: {content_type}")
                    
                    # Проверяем что это JSON
                    try:
                        data = resp.json()
                        print(f"    ✓ Got JSON with {len(str(data))} chars")
                        
                        # Ищем события
                        events = self.find_events(data)
                        if events:
                            print(f"    ✓ Found {len(events)} events!")
                            for evt in events[:2]:
                                print(f"      - {evt}")
                            return True
                    except:
                        print(f"    ✗ Not JSON or parse error")
            except Exception as e:
                print(f"    ✗ Error: {e}")
            
            time.sleep(2)  # Задержка между запросами
        
        return False
    
    def test_xds_api(self) -> bool:
        """Тестирует XDS API endpoints"""
        print("\n[4] Testing XDS API...")
        endpoints = [
            '/api/xds/v2/sport/205',
            '/api/xds/v2/sports',
            '/api/xds/v2/events',
        ]
        
        for endpoint in endpoints:
            print(f"  Trying {endpoint}...")
            try:
                resp = self.session.get(f"{self.base_url}{endpoint}", timeout=10)
                print(f"    Status: {resp.status_code}")
                
                if resp.status_code == 200:
                    try:
                        data = resp.json()
                        print(f"    ✓ Got JSON")
                        
                        events = self.find_events(data)
                        if events:
                            print(f"    ✓ Found {len(events)} events!")
                            return True
                    except:
                        print(f"    ✗ Parse error")
            except Exception as e:
                print(f"    ✗ Error: {e}")
            
            time.sleep(2)
        
        return False
    
    def find_events(self, obj, depth=0, found=None) -> List[Dict]:
        """Рекурсивно ищет события"""
        if found is None:
            found = []
        
        if depth > 10:
            return found
        
        if isinstance(obj, dict):
            # Проверяем если это событие
            if 'id' in obj and ('home' in obj or 'name' in obj or 'title' in obj):
                if isinstance(obj.get('id'), int):
                    event = {
                        'id': obj.get('id'),
                        'home': obj.get('home') or obj.get('name'),
                        'away': obj.get('away'),
                    }
                    found.append(event)
            
            # Ищем дальше
            for v in obj.values():
                self.find_events(v, depth + 1, found)
        
        elif isinstance(obj, list):
            for item in obj:
                self.find_events(item, depth + 1, found)
        
        return found


def main():
    print("=" * 60)
    print("WINLINE REST API QUICK TEST")
    print("=" * 60)
    
    tester = WinlineAPITest()
    
    results = []
    
    # Тест 1: IP endpoint
    results.append(("IP endpoint", tester.test_ip_endpoint()))
    
    # Тест 2: Main page
    results.append(("Main page load", tester.test_main_page()))
    
    # Тест 3: CLS API
    results.append(("CLS API", tester.test_cls_api()))
    
    # Тест 4: XDS API
    results.append(("XDS API", tester.test_xds_api()))
    
    # Итоги
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    for name, result in results:
        status = "✓ PASS" if result else "✗ FAIL"
        print(f"{name:20} {status}")
    
    passed = sum(1 for _, r in results if r)
    print(f"\nPassed: {passed}/{len(results)}")


if __name__ == '__main__':
    main()
