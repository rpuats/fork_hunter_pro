import requests,re,json,sys
from bs4 import BeautifulSoup
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get('https://tennisi.bet',headers=headers,timeout=30).text
# dump candidate category ids from whole homepage
pairs=[]
for m in re.finditer(r'categoryid=(\d+)', html):
    cid=m.group(1)
    start=max(0,m.start()-120)
    end=min(len(html),m.end()+180)
    pairs.append({'category_id':cid,'context':html[start:end]})
# unique by cid
seen={}
for p in pairs:
    seen.setdefault(p['category_id'], p)
out=list(seen.values())
sys.stdout.buffer.write(json.dumps(out[:120], ensure_ascii=False, indent=2).encode('utf-8'))
