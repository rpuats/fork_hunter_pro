import requests, sys, json
headers={'User-Agent':'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36','Accept':'application/json, text/plain, */*','Accept-Language':'ru-RU,ru;q=0.9','Referer':'https://www.olimp.bet/'}
paths=[]
for sport_id in ['1','2','4','5','6','7','8','9','10','11','12','13']:
    for suffix in ['line/top/sports-with-competitions-with-events?vids%5B%5D=','line/sports-with-competitions-with-events?vids%5B%5D=']:
        u=f'https://www.olimp.bet/api/v4/{sport_id}/{suffix}'
        try:
            r=requests.get(u,headers=headers,timeout=25)
            paths.append({'url':u,'status':r.status_code,'len':len(r.text),'preview':r.text[:120]})
        except Exception as e:
            paths.append({'url':u,'error':str(e)})
sys.stdout.buffer.write(json.dumps(paths, ensure_ascii=False, indent=2).encode('utf-8'))
