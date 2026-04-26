"""
Анализируем main.js чтобы найти где загружаются события
"""
import re
import requests

print("Downloading main.js...")
resp = requests.get('https://winline.ru/main.6f043bc42cc485b7.js', timeout=30)
print(f"Downloaded {len(resp.text)} bytes")

# Ищем URL patterns
urls = re.findall(r'["\'](?:https?:)?//[^"\'<>\s]+["\']', resp.text)
unique_urls = sorted(set(urls))

print(f"\nFound {len(unique_urls)} unique URL references:")
for url in unique_urls[:50]:
    url_clean = url.strip('"\'')
    if 'api' in url_clean or 'events' in url_clean or 'feed' in url_clean or 'line' in url_clean or 'sport' in url_clean:
        print(f"  {url_clean}")

# Ищем ключевые слова
keywords = ['api', 'events', 'feed', 'graphql', 'endpoint', 'websocket', 'ws://', 'wss://', '/v1/', '/v2/', '/v3/']
for keyword in keywords:
    if keyword in resp.text:
        count = resp.text.count(keyword)
        print(f"\nKeyword '{keyword}': appears {count} times")
        
        # Находим контекст
        matches = re.finditer(f'.{{0,100}}{re.escape(keyword)}.{{0,100}}', resp.text)
        for i, m in enumerate(list(matches)[:3]):
            context = m.group(0).replace('\n', ' ')
            print(f"   Context {i+1}: ...{context}...")

# Ищем fetch/axios вызовы
print("\n\nLooking for fetch/axios patterns...")
patterns = [
    r'fetch\s*\(\s*["\']([^"\']+)["\']',
    r'axios\.get\s*\(\s*["\']([^"\']+)["\']',
    r'\.get\s*\(\s*["\']([^"\']+)["\']',
    r'["\'](?:/api/[^"\']+)["\']',
]

for pattern in patterns:
    matches = re.findall(pattern, resp.text)
    if matches:
        print(f"\n  Pattern: {pattern[:50]}")
        for match in set(matches)[:10]:
            print(f"    - {match}")

# Ищем WebSocket
if 'WebSocket' in resp.text or 'ws://' in resp.text or 'wss://' in resp.text:
    print("\nWebSocket support detected!")
    ws_matches = re.findall(r'(?:wss?://[^\s"\'<>]+|WebSocket\(["\']([^"\']+)["\'])', resp.text)
    for ws in set(ws_matches)[:10]:
        print(f"  - {ws}")

# Ищем конкретные паттерны для Winline
print("\n\nWinline-specific patterns...")
winline_patterns = [
    r'/api/[a-z0-9/_\-]+',
    r'sport["\']?\s*:\s*["\']?[^"\'}\s]+',
    r'event["\']?\s*:\s*["\']?[^"\'}\s]+',
]

for pattern in winline_patterns:
    matches = re.findall(pattern, resp.text, re.IGNORECASE)
    if matches:
        print(f"\n  Pattern matches for {pattern[:30]}:")
        for match in sorted(set(matches))[:15]:
            if len(match) < 100:  # Only show reasonable length matches
                print(f"    - {match}")
