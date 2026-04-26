import requests,re,sys
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
print('len',len(html))
print('evt_links', len(re.findall(r"id=['\"]evtl", html)))
print('row_el', len(re.findall(r"id=['\"]el\d+", html)))
print('tr_count', len(re.findall(r"<tr", html, flags=re.I)))
print('title_pairs', len(re.findall(r'[A-Za-zА-Яа-я0-9 .\-]+ - [A-Za-zА-Яа-я0-9 .\-]+', html)))
