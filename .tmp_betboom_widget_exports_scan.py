import requests, re, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
for m in re.finditer(r'export\s*\{[^\}]{0,400}\}', js):
    start = max(0, m.start()-500)
    end = min(len(js), m.end()+500)
    sys.stdout.buffer.write(js[start:end].encode('utf-8'))
    sys.stdout.buffer.write(b'\n---\n')
