import requests, re, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
for needle in ['init(', 'class ae', 'globalThis.SportbookWidgetRegister', 'routeBuilderFactory', 'destroy(', 'mount', 'container']:
    idx = js.find(needle)
    if idx >= 0:
        start = max(0, idx-1200)
        end = min(len(js), idx+3500)
        sys.stdout.buffer.write((f'\n=== {needle} ===\n').encode('utf-8'))
        sys.stdout.buffer.write(js[start:end].encode('utf-8'))
        sys.stdout.buffer.write(b'\n---\n')
