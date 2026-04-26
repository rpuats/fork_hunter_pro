import requests, json, sys
from bs4 import BeautifulSoup
probes=[('football','137'),('tennis','139'),('pingpong','1085860065'),('baseball','326835')]
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
for slug,cid in probes:
    u=f'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid={cid}&more=today&lang=rus'
    html=requests.get(u,headers=headers,timeout=30).text
    soup=BeautifulSoup(html,'html.parser')
    titles=[' '.join(a.stripped_strings) for a in soup.select("a[id^='evtl']")]
    print(slug, 'titles', len(titles))
