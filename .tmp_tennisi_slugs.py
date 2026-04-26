import requests, re, json, sys
from bs4 import BeautifulSoup
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get('https://tennisi.bet/sport',headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
out=[]
for a in soup.select('a[href]'):
    href=a.get('href','')
    text=' '.join(a.stripped_strings).strip()
    m=re.match(r'^/sport/([a-z0-9_-]+)$', href)
    if m and text:
        out.append({'slug':m.group(1),'text':text})
seen={}
for item in out:
    seen.setdefault(item['slug'], item)
out=list(seen.values())
out.sort(key=lambda x:x['slug'])
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
