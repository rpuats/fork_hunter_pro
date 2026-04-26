import requests, sys
base='https://www.olimp.bet/api/v4/0'
paths=['line/top/sports-with-competitions-with-events?vids%5B%5D=','line/sports-with-competitions-with-events?vids%5B%5D=','line/all/sports-with-competitions-with-events?vids%5B%5D=']
headers={'User-Agent':'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36','Accept':'application/json, text/plain, */*','Accept-Language':'ru-RU,ru;q=0.9','Referer':'https://www.olimp.bet/'}
out=[]
for p in paths:
    u=base+'/'+p
    r=requests.get(u,headers=headers,timeout=30)
    out.append({'path':p,'status':r.status_code,'len':len(r.text),'preview':r.text[:300]})
sys.stdout.buffer.write(str(out).encode('utf-8'))
