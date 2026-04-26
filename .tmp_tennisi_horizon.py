import requests
urls=[
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&more=today&lang=rus',
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&more=tomorrow&lang=rus',
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&more=all&lang=rus',
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&more=week&lang=rus',
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&more=soon&lang=rus',
'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid=139&lang=rus',
]
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
for u in urls:
    r=requests.get(u,headers=headers,timeout=30)
    print(u, r.status_code, len(r.text))
