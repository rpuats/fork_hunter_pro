import requests, re, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/App-Bkt_j_3e.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
paths = sorted(set(re.findall(r'/sporthub/[A-Za-z0-9_\-/]+', js)))
sys.stdout.buffer.write('\n'.join(paths).encode('utf-8'))
