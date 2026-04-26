import requests, re, json, sys
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
out=[]
for a in soup.select("a[id^='evtl']")[:120]:
    txt=' '.join(a.stripped_strings)
    out.append(txt)
sys.stdout.buffer.write(json.dumps(out[:80], ensure_ascii=False, indent=2).encode('utf-8'))
