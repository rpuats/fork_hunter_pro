import requests, json, sys, re
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
report=[]
for row in soup.select("tr[id^='el']")[:160]:
    rid=row.get('id','')[2:]
    a=row.select_one("a[id^='evtl']")
    title=' '.join(a.stripped_strings) if a else ''
    cells=[' '.join(td.stripped_strings).strip() for td in row.select('th,td')]
    report.append({'id':rid,'title':title,'cell_count':len(cells),'cells':cells[:12]})
sys.stdout.buffer.write(json.dumps(report[:40], ensure_ascii=False, indent=2).encode('utf-8'))
