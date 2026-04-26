import requests, json, sys
base = 'https://siteapi.betboom.ru/api/site_api/v1'
paths = [
    '/matches/get',
    '/matches/get_by_tournament_id',
    '/tournaments/get_by_category_id',
    '/tournaments/get_by_sport_id',
    '/tournaments_stat',
    '/get_tournament_group',
]
bodies = [
    {}, {'sport_id': 1}, {'sportId': 1}, {'category_id': 1}, {'categoryId': 1},
    {'tournament_id': 1}, {'tournamentId': 1}, {'match_id': 1}, {'matchId': 1},
    {'page': 1}, {'limit': 10}, {'sport_id': 1, 'page': 1}, {'sportId': 1, 'page': 1},
]
headers = {
    'User-Agent': 'Mozilla/5.0',
    'Content-Type': 'application/json;charset=UTF-8',
    'Accept': 'application/json, text/plain, */*',
    'X-Platform': 'web',
    'Referer': 'https://betboom.ru/sport/football',
}
out = []
for path in paths:
    for body in bodies:
        r = requests.post(base + path, headers=headers, data=json.dumps(body), timeout=30)
        out.append({
            'path': path,
            'body': body,
            'status': r.status_code,
            'text': r.text[:260],
        })
        if r.status_code == 200 and '"code":404' not in r.text:
            break
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
