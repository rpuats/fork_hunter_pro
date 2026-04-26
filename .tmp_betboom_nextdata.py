import requests, re, json, sys
html = requests.get('https://betboom.ru/sport/football', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
m = re.search(r'<script id="__NEXT_DATA__" type="application/json">(.*?)</script>', html)
if not m:
    print('NO_NEXT_DATA')
    raise SystemExit(1)
data = json.loads(m.group(1))
out = {
    'page': data.get('page'),
    'query': data.get('query'),
    'buildId': data.get('buildId'),
    'runtimeConfig': data.get('runtimeConfig'),
    'pagePropsKeys': list(data.get('props', {}).get('pageProps', {}).keys())[:80],
    'pageProps': data.get('props', {}).get('pageProps', {}),
}
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
