import requests, re, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
patterns = [
    r'https://[^"\'\s`]+',
    r'/api/[^"\'\s`]+',
    r'tree_ws[^"\'\s`]+',
    r'[A-Za-z0-9_]+WS_URL',
    r'api:\{url:[^}]{0,200}',
    r'feedWS:\{url:[^}]{0,120}',
    r'marketBetStatsWS:\{url:[^}]{0,120}',
    r'betsHistoryWS:\{url:[^}]{0,120}',
    r'graphql[^"\'\s`]*',
]
out = []
for pat in patterns:
    out.extend(re.findall(pat, js))
out = sorted(set(out))
sys.stdout.buffer.write('\n'.join(out[:300]).encode('utf-8'))
