import requests, json, sys, re
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
ids=[]
for row in soup.select("tr[id^='el']"):
    rid=row.get('id','')
    ids.append(rid[2:])
from collections import Counter
c=Counter(ids)
dup={k:v for k,v in c.items() if v>1}
print('rows',len(ids),'unique',len(c),'dups',len(dup))
print(list(dup.items())[:20])
