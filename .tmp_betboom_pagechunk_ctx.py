import requests, sys, re
js = requests.get('https://site-static-green2.betboom.ru/_next/static/chunks/pages/sport/%5B%5B...all%5D%5D-e2714676eb9dbbf8.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
for needle in ['SPORTBOOK_ORIGIN_URL','widgets/sportbook/v1/modern/widget.js','o.useGate(U','token:u.token??""']:
    idx = js.find(needle)
    print(f'=== {needle} @ {idx} ===')
    if idx >= 0:
        start = max(0, idx-1200)
        end = min(len(js), idx+2800)
        sys.stdout.buffer.write(js[start:end].encode('utf-8'))
        sys.stdout.buffer.write(b'\n---\n')
