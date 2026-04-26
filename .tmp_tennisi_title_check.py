import requests, json, re, sys
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')

def norm(s):
    return ' '.join((s or '').split()).strip()

def sanitize(v):
    for suf in ['. Специальные ставки',' Специальные ставки','. Спец. ставки',' Спец. ставки']:
        if v.endswith(suf):
            v=v[:-len(suf)]
    return norm(v)

def split_title(title):
    for sep in [' - ',' – ',' — ',' vs ',' VS ',' v ']:
        parts=title.split(sep,1)
        if len(parts)==2:
            a=norm(parts[0]); b=norm(parts[1]);
            if a and b and a!=b: return a,b
    for sep in ['-','–','—']:
        m=title.find(sep)
        if m==-1: continue
        before=title[m-1] if m>0 else ''
        after=title[m+1] if m+1<len(title) else ''
        if not (before.isspace() or after.isspace()):
            continue
        a=norm(title[:m]); b=norm(title[m+1:]);
        if a and b and a!=b: return a,b
    return None

def valid(name):
    t=name.strip();
    if len(t)<2 or len(t)>120: return False
    lower=t.lower()
    invalid={'live','матч','событие','ставки','specials','специальные ставки','unknown','tbd','n/a'}
    if lower in invalid: return False
    if all(ch.isdigit() or ch in '.- ' for ch in t): return False
    return True
rows=[]
for a in soup.select("a[id^='evtl']"):
    title=sanitize(' '.join(a.stripped_strings))
    pair=split_title(title)
    rows.append({'title':title,'pair':pair,'valid': pair and valid(pair[0]) and valid(pair[1])})
print('titles',len(rows))
print('pair_ok',sum(1 for r in rows if r['pair']))
print('valid_ok',sum(1 for r in rows if r['valid']))
failed=[r['title'] for r in rows if not r['valid']][:40]
sys.stdout.buffer.write(json.dumps(failed, ensure_ascii=False, indent=2).encode('utf-8'))
