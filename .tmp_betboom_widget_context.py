import requests, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
for needle in ['FEED_WS_URL','MARKET_BETSTATS_WS_URL','BETS_HISTORY_WS_URL','tree_ws/v1','api:{url']:
    idx = js.find(needle)
    if idx >= 0:
        start = max(0, idx-800)
        end = min(len(js), idx+2200)
        sys.stdout.buffer.write((f'\n=== {needle} ===\n' + js[start:end] + '\n').encode('utf-8'))
