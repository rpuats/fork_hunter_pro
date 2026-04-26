import requests
probes=[('football','137'),('tennis','139'),('basketball','140'),('volleyball','9027116'),('cybersport','439908280'),('pingpong','1085860065'),('baseball','326835')]
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
for slug,cid in probes:
    for more in ['today','tomorrow']:
        u=f'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid={cid}&more={more}&lang=rus'
        r=requests.get(u,headers=headers,timeout=30)
        print(slug, more, r.status_code, len(r.text))
