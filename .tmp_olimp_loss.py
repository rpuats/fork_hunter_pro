import requests, json, sys, re
u='https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D='
headers={'User-Agent':'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36','Accept':'application/json, text/plain, */*','Accept-Language':'ru-RU,ru;q=0.9','Referer':'https://www.olimp.bet/'}
obj=requests.get(u,headers=headers,timeout=30).json()
counts={'total':0,'has_team_fields':0,'has_name_only':0,'name_split_ok':0,'other':0}
samples=[]
for sport in obj:
    payload=sport.get('payload',{})
    for comp in payload.get('competitionsWithEvents',[]):
        for ev in comp.get('events',[]):
            counts['total']+=1
            if ev.get('team1Name') and ev.get('team2Name'):
                counts['has_team_fields']+=1
            else:
                name=ev.get('name') or (ev.get('names') or {}).get('0') or ''
                if name:
                    counts['has_name_only']+=1
                    split=None
                    for sep in [' - ',' – ',' — ',' vs ',' VS ',' v ','-']:
                        parts=name.split(sep,1)
                        if len(parts)==2:
                            a=parts[0].strip(); b=parts[1].strip()
                            if a and b and a!=b:
                                split=(a,b); break
                    if split:
                        counts['name_split_ok']+=1
                    elif len(samples)<25:
                        samples.append({'name':name,'keys':list(ev.keys())[:20]})
                else:
                    counts['other']+=1
print(counts)
sys.stdout.buffer.write(json.dumps(samples, ensure_ascii=False, indent=2).encode('utf-8'))
