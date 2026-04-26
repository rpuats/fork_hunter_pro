import requests, json, sys
headers = {
    'User-Agent': 'Mozilla/5.0',
    'Accept': 'application/json, text/plain, */*',
    'Content-Type': 'application/json;charset=UTF-8',
    'X-Platform': 'web',
    'Referer': 'https://betboom.ru/sport'
}
r = requests.post('https://betboom.ru/api/games/get_game_kinds', headers=headers, data='{}', timeout=30)
out = {
    'status': r.status_code,
    'text_preview': r.text[:500],
}
try:
    obj = r.json()
    out['keys'] = list(obj.keys())
    for k, v in list(obj.items())[:5]:
        if isinstance(v, list):
            out[f'{k}_len'] = len(v)
            out[f'{k}_sample'] = v[:5]
        elif isinstance(v, dict):
            out[f'{k}_sample'] = dict(list(v.items())[:10])
        else:
            out[f'{k}_value'] = v
except Exception as e:
    out['json_error'] = str(e)
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
