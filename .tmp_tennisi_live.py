import requests
u='https://tennisi.bet/rt/cgi/!book2_free.LiveBetsLines?val=1&gameid=5&categoryid=29010669&lang=rus&tbnohdr=1'
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
r=requests.get(u,headers=headers,timeout=30)
print(r.status_code, len(r.text))
print(r.text[:500])
