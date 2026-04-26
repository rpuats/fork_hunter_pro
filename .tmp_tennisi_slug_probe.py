import requests,re
slugs=['badminton','cricket','mma','snooker','esoccer','esports','counterstrike','cs2','lol','dota2','tabletennis','table-tennis','futsal','boxing','cycling','golf']
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
for slug in slugs:
    u=f'https://tennisi.bet/sport/{slug}'
    try:
        html=requests.get(u,headers=headers,timeout=20).text
        m=re.search(r'categoryid=(\d+)', html)
        print(slug, 'cid', m.group(1) if m else None, 'len', len(html))
    except Exception as e:
        print(slug, 'ERR', e)
