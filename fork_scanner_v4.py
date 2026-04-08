#!/usr/bin/env python3
"""
Fork Scanner v4 - ALL Bookmakers (Leon, Pari, Fonbet, Marathon, 24bet, OlimpBet, Bettery, Sportbet).
Optimized for maximum coverage.
"""
import requests
import json
import time
import re
import sys
import os
from collections import defaultdict

os.environ['PYTHONIOENCODING'] = 'utf-8'
try:
    sys.stdout.reconfigure(encoding='utf-8')
except:
    pass

def normalize(s):
    if not s: return ''
    s = re.sub(r'\(.*?\)', '', s.lower()).strip()
    return re.sub(r'\s+', ' ', s)

def is_placeholder(home, away):
    placeholders = ['хозяева', 'гости', 'home', 'away', 'team 1', 'team 2', 'команда 1', 'команда 2']
    return home.lower() in placeholders or away.lower() in placeholders

def parse_shared_platform(url, headers, bk_name):
    """Parser for Pari, Fonbet, Marathon, 24bet, Bettery (shared API structure)."""
    events = []
    try:
        r = requests.get(url, headers=headers, timeout=20)
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
            if is_placeholder(home, away): continue
            
            eid = m.get('id')
            fs = factor_map.get(eid, {})
            o1 = float(fs.get(921, 0) or 0)
            ox = float(fs.get(922, 0) or 0)
            o2 = float(fs.get(923, 0) or 0)
            
            if o1 > 1 and o2 > 1:
                events.append({
                    'home': normalize(home), 'away': normalize(away),
                    'home_raw': home, 'away_raw': away,
                    'o1': o1, 'ox': ox, 'o2': o2, 'bk': bk_name
                })
    except Exception as e:
        print(f"  ERROR {bk_name}: {e}")
    return events

def parse_leon():
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
            if is_placeholder(home, away): continue
            
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
                events.append({
                    'home': normalize(home), 'away': normalize(away),
                    'home_raw': home, 'away_raw': away,
                    'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'leon'
                })
    except Exception as e:
        print(f"  ERROR Leon: {e}")
    return events

def parse_olimpbet():
    events = []
    try:
        r = requests.get("https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D=",
                       headers={"Accept": "application/json", "Referer": "https://www.olimp.bet"}, timeout=20)
        data = r.json()
        for item in data:
            payload = item.get('payload', {})
            if not isinstance(payload, dict): continue
            comps = payload.get('competitionsWithEvents', [])
            if not isinstance(comps, list): continue
            
            for comp in comps:
                league = comp.get('name', 'Unknown')
                evts = comp.get('events', [])
                if not isinstance(evts, list): continue
                
                for e in evts:
                    home = str(e.get('team1Name', '')).strip()
                    away = str(e.get('team2Name', '')).strip()
                    if not home or not away: continue
                    if is_placeholder(home, away): continue
                    
                    o1 = ox = o2 = 0
                    for o in e.get('outcomes', []):
                        if o.get('marketId') == 1:
                            sn = o.get('shortName', '')
                            odds = float(o.get('probability', 0) or 0)
                            if sn == 'П1': o1 = odds
                            elif sn == 'Х': ox = odds
                            elif sn == 'П2': o2 = odds
                    
                    if o1 > 1 and o2 > 1:
                        events.append({
                            'home': normalize(home), 'away': normalize(away),
                            'home_raw': home, 'away_raw': away,
                            'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'olimp'
                        })
    except Exception as e:
        print(f"  ERROR OlimpBet: {e}")
    return events

def parse_sportbet():
    events = []
    try:
        r = requests.get("https://sportbet.ru/sport/v1/rating-fixtures-tree?period=ALL",
                       headers={"Accept": "application/json", "Referer": "https://sportbet.ru"}, timeout=20)
        data = r.json()
        fixtures = data.get('fixtures', {})
        markets = data.get('m', {})
        
        for fix_id, fixture in fixtures.items():
            if not isinstance(fixture, dict): continue
            comps = fixture.get('c', [])
            if len(comps) < 2: continue
            
            home = comps[0].get('n', '').strip()
            away = comps[1].get('n', '').strip()
            if not home or not away: continue
            if is_placeholder(home, away): continue
            
            fixture_markets = markets.get(fix_id, [])
            if not isinstance(fixture_markets, list): continue
            
            o1 = ox = o2 = 0
            for market in fixture_markets:
                if not isinstance(market, dict): continue
                if market.get('n', '').lower() == '1x2':
                    outcomes = market.get('m', [])
                    if isinstance(outcomes, list) and len(outcomes) > 0:
                        for outcome in outcomes:
                            if not isinstance(outcome, dict): continue
                            selections = outcome.get('sel', [])
                            for sel in selections:
                                if not isinstance(sel, dict): continue
                                name = sel.get('n', '')
                                odds = float(sel.get('o', 0) or 0)
                                if name == home: o1 = odds
                                elif 'ничья' in name.lower() or 'draw' in name.lower(): ox = odds
                                elif name == away: o2 = odds
            
            if o1 > 1 and o2 > 1:
                events.append({
                    'home': normalize(home), 'away': normalize(away),
                    'home_raw': home, 'away_raw': away,
                    'o1': o1, 'ox': ox, 'o2': o2, 'bk': 'sportbet'
                })
    except Exception as e:
        print(f"  ERROR Sportbet: {e}")
    return events

print("="*70)
print("GHOST IMPERIUM - Ultimate Fork Scanner v4 (ALL BKs)")
print("="*70)

print("\nFetching events from 8 bookmakers...")
t0 = time.time()

from concurrent.futures import ThreadPoolExecutor, as_completed

headers_pari = {"Accept": "application/json", "Referer": "https://pari.ru"}
headers_fonbet = {"Accept": "application/json", "Referer": "https://fonbet.ru"}
headers_marathon = {"Accept": "application/json", "Referer": "https://www.marathonbet.ru"}
headers_24bet = {"Accept": "application/json", "Referer": "https://24bet.ru"}
headers_bettery = {"Accept": "application/json", "Referer": "https://bettery.ru"}

all_events = {}

# Define tasks
tasks = [
    (parse_leon, 'leon'),
    (lambda: parse_shared_platform("https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300", headers_pari, 'pari'), 'pari'),
    (lambda: parse_shared_platform("https://line-lb61-w.bk6bba-resources.com/ma/events/listBase?lang=ru&scopeMarket=1600", headers_fonbet, 'fonbet'), 'fonbet'),
    (lambda: parse_shared_platform("https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000", headers_marathon, 'marathon'), 'marathon'),
    (lambda: parse_shared_platform("https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000", headers_24bet, '24bet'), '24bet'),
    (lambda: parse_shared_platform("https://line01.at58f5-resources.com/events/listBase?lang=ru&scopeMarket=501", headers_bettery, 'bettery'), 'bettery'),
    (parse_olimpbet, 'olimp'),
    (parse_sportbet, 'sportbet'),
]

with ThreadPoolExecutor(max_workers=8) as executor:
    futures = {executor.submit(func): name for func, name in tasks}
    
    for future in as_completed(futures):
        bk = futures[future]
        try:
            events = future.result()
            all_events[bk] = events
            print(f"  OK {bk}: {len(events)} events")
        except Exception as e:
            print(f"  ERROR {bk}: {e}")

total = sum(len(v) for v in all_events.values())
print(f"\nTotal: {total} events from {len(all_events)} BKs ({time.time()-t0:.1f}s)")

# Find forks
print(f"\nFinding forks (Exact Match)...")
t1 = time.time()

forks = []
bk_slugs = sorted(all_events.keys())

for i in range(len(bk_slugs)):
    for j in range(i + 1, len(bk_slugs)):
        bk_a = bk_slugs[i]
        bk_b = bk_slugs[j]
        
        # Index bk_b by home team
        home_index = defaultdict(list)
        for e in all_events[bk_b]:
            home_index[e['home']].append(e)
        
        matched = 0
        bk_forks = 0
        for ea in all_events[bk_a]:
            candidates = home_index.get(ea['home'], [])
            for eb in candidates:
                if ea['away'] == eb['away']:
                    matched += 1
                    
                    best_1 = max(ea['o1'], eb['o1'])
                    best_x = max(ea['ox'], eb['ox'])
                    best_2 = max(ea['o2'], eb['o2'])
                    
                    if best_1 > 1 and best_x > 1 and best_2 > 1:
                        margin = 1/best_1 + 1/best_x + 1/best_2
                        if margin < 1:
                            profit = (1 - margin) * 100
                            if profit > 1.0:
                                bk_forks += 1
                                forks.append({
                                    'match': f"{ea['home_raw']} vs {ea['away_raw']}",
                                    'profit_percent': round(profit, 2),
                                    'bks': f"{bk_a} vs {bk_b}",
                                    'bets': [
                                        {'outcome': '1', 'bk': bk_a if ea['o1'] > eb['o1'] else bk_b, 'odd': best_1},
                                        {'outcome': 'X', 'bk': bk_a if ea['ox'] > eb['ox'] else bk_b, 'odd': best_x},
                                        {'outcome': '2', 'bk': bk_a if ea['o2'] > eb['o2'] else bk_b, 'odd': best_2},
                                    ],
                                })
        
        if bk_forks > 0:
            print(f"  {bk_a} <-> {bk_b}: {matched} matched, {bk_forks} forks")

forks.sort(key=lambda x: -x['profit_percent'])

print(f"\nTotal forks found: {len(forks)} ({time.time()-t1:.1f}s)")

if forks:
    print(f"\n{'='*70}")
    print("TOP 10 FORKS:")
    print(f"{'='*70}")
    for i, f in enumerate(forks[:10], 1):
        print(f"\n#{i} {f['match']}")
        print(f"   Profit: {f['profit_percent']}% | BKs: {f['bks']}")
        for bet in f['bets']:
            print(f"   -> {bet['outcome']}: {bet['bk']} @ {bet['odd']}")

with open('forks_ultimate_output.json', 'w', encoding='utf-8') as fp:
    json.dump({'forks': forks[:100], 'total': len(forks)}, fp, ensure_ascii=False, indent=2)

print(f"\nSaved to forks_ultimate_output.json")
print(f"{'='*70}")
