#!/usr/bin/env python3
"""
Ultra-Fast Fork Scanner - uses hash-based matching for speed.
"""
import asyncio
import sys
import time
import json
import re
from collections import defaultdict

sys.stdout.reconfigure(encoding='utf-8')

def normalize(s):
    if not s: return ''
    s = re.sub(r'\(.*?\)', '', s.lower()).strip()
    return re.sub(r'\s+', ' ', s)

def fast_scan():
    print("="*70)
    print("🔍 GHOST IMPERIUM — Ultra-Fast Fork Scanner")
    print("="*70)
    
    import requests
    
    # Fetch from all fast APIs in parallel using threads
    from concurrent.futures import ThreadPoolExecutor, as_completed
    
    def fetch_leon():
        events = []
        try:
            r = requests.get("https://leon.ru/api-2/betline/events/prematch?ctag=ru-RU", 
                           headers={"Accept": "application/json", "Referer": "https://leon.ru"}, timeout=20)
            data = r.json()
            for m in data.get('events', []):
                comps = m.get('competitors', [])
                if len(comps) < 2: continue
                home = comps[0].get('name', '').strip()
                away = comps[1].get('name', '').strip()
                if not home or not away: continue
                o1 = ox = o2 = 0
                for market in m.get('markets', []):
                    runners = market.get('runners', [])
                    if len(runners) == 3 and runners[0].get('name') == '1':
                        for r in runners:
                            n = r.get('name', '')
                            p = float(r.get('price', 0) or 0)
                            if n == '1': o1 = p
                            elif n == 'X': ox = p
                            elif n == '2': o2 = p
                        break
                if o1 > 1 and o2 > 1:
                    events.append({'home': normalize(home), 'away': normalize(away), 
                                   'home_raw': home, 'away_raw': away,
                                   'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'leon'})
        except Exception as e:
            print(f"  ❌ Leon: {e}")
        return events
    
    def fetch_pari():
        events = []
        try:
            r = requests.get("https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300",
                           headers={"Accept": "application/json", "Referer": "https://pari.ru"}, timeout=20)
            data = r.json()
            matches = data.get('events', [])
            factors = data.get('customFactors', [])
            factor_map = {}
            for cf in factors:
                if isinstance(cf, dict):
                    eid = cf.get('e')
                    fs = cf.get('factors', [])
                    if eid and isinstance(fs, list):
                        factor_map[eid] = {f.get('f'): f.get('v') for f in fs if isinstance(f, dict)}
            for m in matches:
                if not isinstance(m, dict): continue
                home = str(m.get('team1', '')).strip()
                away = str(m.get('team2', '')).strip()
                if not home or not away: continue
                eid = m.get('id')
                fs = factor_map.get(eid, {})
                o1 = float(fs.get(921, 0) or 0)
                ox = float(fs.get(922, 0) or 0)
                o2 = float(fs.get(923, 0) or 0)
                if o1 > 1 and o2 > 1:
                    events.append({'home': normalize(home), 'away': normalize(away),
                                   'home_raw': home, 'away_raw': away,
                                   'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'pari'})
        except Exception as e:
            print(f"  ❌ Pari: {e}")
        return events
    
    def fetch_fonbet():
        events = []
        try:
            r = requests.get("https://line-lb61-w.bk6bba-resources.com/ma/events/listBase?lang=ru&scopeMarket=1600",
                           headers={"Accept": "application/json", "Referer": "https://fonbet.ru"}, timeout=20)
            data = r.json()
            matches = data.get('events', [])
            factors = data.get('customFactors', [])
            factor_map = {}
            for cf in factors:
                if isinstance(cf, dict):
                    eid = cf.get('e')
                    fs = cf.get('factors', [])
                    if eid and isinstance(fs, list):
                        factor_map[eid] = {f.get('f'): f.get('v') for f in fs if isinstance(f, dict)}
            for m in matches:
                if not isinstance(m, dict): continue
                home = str(m.get('team1', '')).strip()
                away = str(m.get('team2', '')).strip()
                if not home or not away: continue
                eid = m.get('id')
                fs = factor_map.get(eid, {})
                o1 = float(fs.get(921, 0) or 0)
                ox = float(fs.get(922, 0) or 0)
                o2 = float(fs.get(923, 0) or 0)
                if o1 > 1 and o2 > 1:
                    events.append({'home': normalize(home), 'away': normalize(away),
                                   'home_raw': home, 'away_raw': away,
                                   'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'fonbet'})
        except Exception as e:
            print(f"  ❌ Fonbet: {e}")
        return events
    
    def fetch_marathon():
        events = []
        try:
            r = requests.get("https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000",
                           headers={"Accept": "application/json", "Referer": "https://www.marathonbet.ru"}, timeout=20)
            data = r.json()
            matches = data.get('events', [])
            factors = data.get('customFactors', [])
            factor_map = {}
            for cf in factors:
                if isinstance(cf, dict):
                    eid = cf.get('e')
                    fs = cf.get('factors', [])
                    if eid and isinstance(fs, list):
                        factor_map[eid] = {f.get('f'): f.get('v') for f in fs if isinstance(f, dict)}
            for m in matches:
                if not isinstance(m, dict): continue
                home = str(m.get('team1', '')).strip()
                away = str(m.get('team2', '')).strip()
                if not home or not away: continue
                eid = m.get('id')
                fs = factor_map.get(eid, {})
                o1 = float(fs.get(921, 0) or 0)
                ox = float(fs.get(922, 0) or 0)
                o2 = float(fs.get(923, 0) or 0)
                if o1 > 1 and o2 > 1:
                    events.append({'home': normalize(home), 'away': normalize(away),
                                   'home_raw': home, 'away_raw': away,
                                   'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'marathon'})
        except Exception as e:
            print(f"  ❌ Marathon: {e}")
        return events
    
    def fetch_24bet():
        events = []
        try:
            r = requests.get("https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000",
                           headers={"Accept": "application/json", "Referer": "https://24bet.ru"}, timeout=20)
            data = r.json()
            matches = data.get('events', [])
            factors = data.get('customFactors', [])
            factor_map = {}
            for cf in factors:
                if isinstance(cf, dict):
                    eid = cf.get('e')
                    fs = cf.get('factors', [])
                    if eid and isinstance(fs, list):
                        factor_map[eid] = {f.get('f'): f.get('v') for f in fs if isinstance(f, dict)}
            for m in matches:
                if not isinstance(m, dict): continue
                home = str(m.get('team1', '')).strip()
                away = str(m.get('team2', '')).strip()
                if not home or not away: continue
                eid = m.get('id')
                fs = factor_map.get(eid, {})
                o1 = float(fs.get(921, 0) or 0)
                ox = float(fs.get(922, 0) or 0)
                o2 = float(fs.get(923, 0) or 0)
                if o1 > 1 and o2 > 1:
                    events.append({'home': normalize(home), 'away': normalize(away),
                                   'home_raw': home, 'away_raw': away,
                                   'o1': o1, 'ox': ox, 'o2': o2, 'bk': '24bet'})
        except Exception as e:
            print(f"  ❌ 24bet: {e}")
        return events
    
    # Fetch all in parallel
    print("\n📥 Fetching events from all bookmakers...")
    t0 = time.time()
    
    all_events = {}
    with ThreadPoolExecutor(max_workers=5) as executor:
        futures = {
            executor.submit(fetch_leon): 'leon',
            executor.submit(fetch_pari): 'pari',
            executor.submit(fetch_fonbet): 'fonbet',
            executor.submit(fetch_marathon): 'marathon',
            executor.submit(fetch_24bet): '24bet',
        }
        
        for future in as_completed(futures):
            bk = futures[future]
            try:
                events = future.result()
                all_events[bk] = events
                print(f"  ✅ {bk}: {len(events)} events")
            except Exception as e:
                print(f"  ❌ {bk}: {e}")
    
    total = sum(len(v) for v in all_events.values())
    print(f"\n📊 Total: {total} events from {len(all_events)} BKs ({time.time()-t0:.1f}s)")
    
    # Fast matching using hash index
    print(f"\n🔗 Finding forks...")
    t1 = time.time()
    
    forks = []
    bk_slugs = list(all_events.keys())
    
    for i in range(len(bk_slugs)):
        for j in range(i + 1, len(bk_slugs)):
            bk_a = bk_slugs[i]
            bk_b = bk_slugs[j]
            
            # Build index for bk_b
            home_index = defaultdict(list)
            away_index = defaultdict(list)
            for e in all_events[bk_b]:
                home_index[e['home']].append(e)
                home_index[e['away']].append(e)
                away_index[e['home']].append(e)
                away_index[e['away']].append(e)
            
            matched = 0
            for ea in all_events[bk_a]:
                candidates = home_index.get(ea['home'], [])
                for eb in candidates:
                    if (ea['home'] == eb['home'] and ea['away'] == eb['away']) or \
                       (ea['home'] == eb['away'] and ea['away'] == eb['home']):
                        matched += 1
                        
                        # Check fork
                        best_1 = max(ea['o1'], eb['o1'])
                        best_x = max(ea['ox'], eb['ox'])
                        best_2 = max(ea['o2'], eb['o2'])
                        
                        if best_1 > 1 and best_x > 1 and best_2 > 1:
                            margin = 1/best_1 + 1/best_x + 1/best_2
                            if margin < 1:
                                profit = (1 - margin) * 100
                                if profit > 0.5:
                                    forks.append({
                                        'match': f"{ea['home_raw']} vs {ea['away_raw']}",
                                        'profit_percent': round(profit, 2),
                                        'margin': round(margin, 4),
                                        'bks': f"{bk_a} vs {bk_b}",
                                        'bets': [
                                            {'outcome': '1', 'bk': bk_a if ea['o1'] > eb['o1'] else bk_b, 'odd': best_1, 'stake': round(1000 * (1/best_1) / margin, 2)},
                                            {'outcome': 'X', 'bk': bk_a if ea['ox'] > eb['ox'] else bk_b, 'odd': best_x, 'stake': round(1000 * (1/best_x) / margin, 2)},
                                            {'outcome': '2', 'bk': bk_a if ea['o2'] > eb['o2'] else bk_b, 'odd': best_2, 'stake': round(1000 * (1/best_2) / margin, 2)},
                                        ],
                                        'payout': round(1000 / margin, 2),
                                    })
            
            print(f"  {bk_a} ↔ {bk_b}: {matched} matched, {sum(1 for f in forks if f['bks'] == f'{bk_a} vs {bk_b}')} forks")
    
    forks.sort(key=lambda x: -x['profit_percent'])
    
    print(f"\n🎯 Total forks found: {len(forks)} ({time.time()-t1:.1f}s)")
    
    if forks:
        print(f"\n{'='*70}")
        print("TOP 20 FORKS:")
        print(f"{'='*70}")
        for i, f in enumerate(forks[:20], 1):
            print(f"\n#{i} 🏆 {f['match']}")
            print(f"   Profit: {f['profit_percent']}% | Payout: {f['payout']}₽")
            print(f"   BKs: {f['bks']}")
            for bet in f['bets']:
                print(f"   → {bet['outcome']}: {bet['bk']} @ {bet['odd']} (stake: {bet['stake']}₽)")
    
    with open('forks_ultra_output.json', 'w', encoding='utf-8') as fp:
        json.dump({'forks': forks[:100], 'total': len(forks)}, fp, ensure_ascii=False, indent=2)
    
    print(f"\n💾 Saved to forks_ultra_output.json")
    print(f"{'='*70}")

fast_scan()
