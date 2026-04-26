import requests, json, sys
from bs4 import BeautifulSoup
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
html=requests.get(u,headers=headers,timeout=30).text
soup=BeautifulSoup(html,'html.parser')
invalid=[]
valid=0
for a in soup.select("a[id^='evtl']"):
    title=' '.join(a.stripped_strings)
    cleaned=title
    for suf in ['. Специальные ставки',' Специальные ставки','. Спец. ставки',' Спец. ставки']:
        if cleaned.endswith(suf):
            cleaned=cleaned[:-len(suf)]
    cleaned=' '.join(cleaned.split()).strip()
    pair=None
    for sep in [' - ',' – ',' — ',' vs ',' VS ',' v ']:
        parts=cleaned.split(sep,1)
        if len(parts)==2:
            a1=' '.join(parts[0].split()).strip(); b1=' '.join(parts[1].split()).strip()
            if a1 and b1 and a1!=b1:
                pair=(a1,b1); break
    if pair is None:
        for sep in ['-','–','—']:
            idx=cleaned.find(sep)
            if idx==-1: continue
            before=cleaned[idx-1] if idx>0 else ''
            after=cleaned[idx+1] if idx+1<len(cleaned) else ''
            if not (before.isspace() or after.isspace()):
                continue
            a1=' '.join(cleaned[:idx].split()).strip(); b1=' '.join(cleaned[idx+1:].split()).strip()
            if a1 and b1 and a1!=b1:
                pair=(a1,b1); break
    def valid_name(name):
        t=name.strip();
        if len(t)<2 or len(t)>120: return False
        lower=t.lower()
        invalid_exact={'live','матч','событие','ставки','specials','специальные ставки','unknown','tbd','n/a'}
        if lower in invalid_exact: return False
        if all(ch.isdigit() or ch in '.- ' for ch in t): return False
        return True
    if pair and valid_name(pair[0]) and valid_name(pair[1]):
        valid += 1
    else:
        invalid.append(cleaned)
print('total', len(soup.select("a[id^=\'evtl\']")))
print('valid', valid)
print('invalid', len(invalid))
sys.stdout.buffer.write(json.dumps(invalid[:60], ensure_ascii=False, indent=2).encode('utf-8'))
