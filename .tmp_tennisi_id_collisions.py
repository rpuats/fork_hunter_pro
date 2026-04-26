import requests,re,collections,json,sys
probes=[('football','137'),('tennis','139'),('basketball','140'),('volleyball','9027116'),('cybersport','439908280'),('pingpong','1085860065'),('baseball','326835'),('handball','5662396'),('waterpolo','8029783'),('futsal','23565786'),('rugby','466447415'),('box','8152637'),('billiard','17076577'),('races','17076134'),('amfootball','4076387'),('other','1960530'),('darts','58446467'),('trends','491109347')]
headers={'Accept':'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8','Accept-Language':'ru-RU,ru;q=0.9,en;q=0.8','Referer':'https://tennisi.bet','User-Agent':'Mozilla/5.0'}
all_ids=[]
by_probe=[]
for slug,cid in probes:
    ids=[]
    for more in ['today','tomorrow']:
        u=f'https://tennisi.bet/rt/cgi/!rt_home.CategoryInfo?gameid=5&categoryid={cid}&more={more}&lang=rus'
        html=requests.get(u,headers=headers,timeout=30).text
        ids += re.findall(r'id=["\']evtl(\d+)', html)
    by_probe.append({'slug':slug,'count':len(ids),'unique':len(set(ids))})
    all_ids.extend(ids)
ctr=collections.Counter(all_ids)
dups=[(k,v) for k,v in ctr.items() if v>1]
print('global_total',len(all_ids),'global_unique',len(ctr),'dups',len(dups))
sys.stdout.buffer.write(json.dumps({'by_probe':by_probe,'sample_dups':dups[:80]}, ensure_ascii=False, indent=2).encode('utf-8'))
