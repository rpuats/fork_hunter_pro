import requests, json, sys
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
all_titles=len(soup.select("a[id^='evtl']"))
el_rows=soup.select("tr[id^='el']")
rows_with_title=0
samples=[]
for row in el_rows:
    a=row.select_one("a[id^='evtl']")
    if a:
        rows_with_title+=1
        if len(samples)<20:
            samples.append(' '.join(a.stripped_strings))
print('all_titles',all_titles)
print('el_rows',len(el_rows))
print('rows_with_title',rows_with_title)
print('rows_without_title',len(el_rows)-rows_with_title)
sys.stdout.buffer.write(json.dumps(samples, ensure_ascii=False, indent=2).encode('utf-8'))
