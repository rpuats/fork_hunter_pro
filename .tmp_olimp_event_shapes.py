import requests, json, sys
u='https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D='
headers={'User-Agent':'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36','Accept':'application/json, text/plain, */*','Accept-Language':'ru-RU,ru;q=0.9','Referer':'https://www.olimp.bet/'}
obj=requests.get(u,headers=headers,timeout=30).json()
out=[]
count=0
for sport in obj:
    payload=sport.get('payload',{})
    for comp in payload.get('competitionsWithEvents',[])[:8]:
        for ev in comp.get('events',[])[:20]:
            if count>=40:
                break
            out.append({'keys':list(ev.keys())[:40],'sample':{k:ev.get(k) for k in list(ev.keys())[:15]}})
            count+=1
        if count>=40: break
    if count>=40: break
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
