import requests, json, sys
probes=[('football','137'),('tennis','139'),('basketball','140'),('volleyball','9027116'),('cybersport','439908280'),('pingpong','1085860065'),('baseball','326835'),('handball','5662396'),('waterpolo','8029783'),('futsal','23565786'),('rugby','466447415'),('box','8152637'),('billiard','17076577'),('races','17076134'),('amfootball','4076387'),('other','1960530'),('darts','58446467'),('trends','491109347')]
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
out=[]
for slug,cid in probes:
    total=0
    for more in ['today','tomorrow']:
        u=f'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid={cid}&more={more}&lang=rus'
        html=requests.get(u,headers=headers,timeout=30).text
        total += html.count('id="evtl') + html.count("id='evtl")
    out.append({'slug':slug,'category_id':cid,'title_count_today_plus_tomorrow':total})
out.sort(key=lambda x:x['title_count_today_plus_tomorrow'], reverse=True)
sys.stdout.buffer.write(json.dumps(out, ensure_ascii=False, indent=2).encode('utf-8'))
