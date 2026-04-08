#!/usr/bin/env python3
"""
Fast Fork Scanner - uses hash-based matching for speed.
"""
import asyncio
import sys
import time
import json
import re

sys.stdout.reconfigure(encoding='utf-8')

def normalize(s):
    if not s:
        return ''
    s = s.lower().strip()
    s = re.sub(r'\(.*?\)', '', s).strip()
    s = re.sub(r'\s+', ' ', s).strip()
    return s

def fast_scan():
    print("="*70)
    print("🔍 GHOST IMPERIUM — Fast Fork Scanner")
    print("="*70)
    
    # Load pre-fetched data or fetch quickly
    print("\n📥 Fetching events from Leon + OlimpBet...")
    
    import requests
    
    # Leon
    t0 = time.time()
    leon_events = []
    try:
        r = requests.get("https://leon.ru/api-2/betline/events/prematch?ctag=ru-RU", 
                        headers={"Accept": "application/json", "Referer": "https://leon.ru"}, timeout=20)
        data = r.json()
        for m in data.get('events', []):
            comps = m.get('competitors', [])
            if len(comps) < 2:
                continue
            home = comps[0].get('name', '').strip()
            away = comps[1].get('name', '').strip()
            if not home or not away:
                continue
            
            o1 = ox = o2 = 0
            for market in m.get('markets', []):
                runners = market.get('runners', [])
                if len(runners) == 3 and runners[0].get('name') == '1':
                    for r in runners:
                        name = r.get('name', '')
                        price = float(r.get('price', 0) or 0)
                        if name == '1': o1 = price
                        elif name == 'X': ox = price
                        elif name == '2': o2 = price
                    break
            
            if o1 > 1 and o2 > 1:
                leon_events.append({
                    'home': normalize(home), 'away': normalize(away),
                    'home_raw': home, 'away_raw': away,
                    'o1': o1, 'ox': ox, 'o2': o2,
                    'bk': 'leon'
                })
    except Exception as e:
        print(f"  Leon error: {e}")
    
    print(f"  ✅ Leon: {len(leon_events)} events ({time.time()-t0:.1f}s)")
    
    # OlimpBet
    t0 = time.time()
    olimp_events = []
    try:
        r = requests.get("https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D=",
                        headers={"Accept": "application/json", "Referer": "https://www.olimp.bet", 
                                 "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"}, timeout=20)
        data = r.json()
        for item in data:
            payload = item.get('payload', {})
            sport = payload.get('sport', {})
            sport_name = sport.get('name', '')
            if 'футбол' not in sport_name.lower():
                continue
            
            comps = payload.get('competitionsWithEvents', [])
            for comp in comps:
                league = comp.get('name', '')
                evts = comp.get('events', [])
                for e in evts:
                    home = e.get('team1Name', '').strip()
                    away = e.get('team2Name', '').strip()
                    if not home or not away:
                        continue
                    
                    o1 = ox = o2 = 0
                    for o in e.get('outcomes', []):
                        if o.get('marketId') == 1:
                            sn = o.get('shortName', '')
                            odds = float(o.get('probability', 0) or 0)
                            if sn == 'П1': o1 = odds
                            elif sn == 'Х': ox = odds
                            elif sn == 'П2': o2 = odds
                    
                    if o1 > 1 and o2 > 1:
                        olimp_events.append({
                            'home': normalize(home), 'away': normalize(away),
                            'home_raw': home, 'away_raw': away,
                            'o1': o1, 'ox': ox, 'o2': o2,
                            'bk': 'olimp', 'league': league
                        })
    except Exception as e:
        print(f"  OlimpBet error: {e}")
    
    print(f"  ✅ OlimpBet: {len(olimp_events)} events ({time.time()-t0:.1f}s)")
    
    # Fast matching using hash index
    print(f"\n🔗 Fast matching...")
    t0 = time.time()
    
    # Build index: normalized_name -> list of events
    from collections import defaultdict
    home_index = defaultdict(list)
    away_index = defaultdict(list)
    
    for e in olimp_events:
        home_index[e['home']].append(e)
        away_index[e['away']].append(e)
    
    forks = []
    matched = 0
    
    for le in leon_events:
        # Find potential matches by exact home or away name
        candidates = home_index.get(le['home'], []) + away_index.get(le['home'], [])
        
        for oe in candidates:
            # Verify both teams match (in either direction)
            if (le['home'] == oe['home'] and le['away'] == oe['away']) or \
               (le['home'] == oe['away'] and le['away'] == oe['home']):
                matched += 1
                
                # Check for fork
                # Best odds for each outcome
                best_1 = max(le['o1'], oe['o1'])
                best_x = max(le['ox'], oe['ox']) if le['ox'] > 1 and oe['ox'] > 1 else max(le['ox'], oe['ox'])
                best_2 = max(le['o2'], oe['o2'])
                
                if best_1 > 1 and best_x > 1 and best_2 > 1:
                    margin = 1/best_1 + 1/best_x + 1/best_2
                    if margin < 1:
                        profit = (1 - margin) * 100
                        if profit > 0.3:
                            forks.append({
                                'match': f"{le['home_raw']} vs {le['away_raw']}",
                                'profit': round(profit, 2),
                                'margin': round(margin, 4),
                                'leon': {'1': le['o1'], 'X': le['ox'], '2': le['o2']},
                                'olimp': {'1': oe['o1'], 'X': oe['ox'], '2': oe['o2']},
                                'best': {'1': best_1, 'X': best_x, '2': best_2},
                            })
    
    forks.sort(key=lambda x: -x['profit'])
    
    print(f"  Matched: {matched} pairs ({time.time()-t0:.2f}s)")
    print(f"  🎯 Forks found: {len(forks)}")
    
    if forks:
        print(f"\n{'='*70}")
        print("TOP FORKS:")
        print(f"{'='*70}")
        for i, f in enumerate(forks[:15], 1):
            print(f"\n#{i} 🏆 {f['match']}")
            print(f"   Profit: {f['profit']}%")
            print(f"   Leon:  1={f['leon']['1']}, X={f['leon']['X']}, 2={f['leon']['2']}")
            print(f"   Olimp: 1={f['olimp']['1']}, X={f['olimp']['X']}, 2={f['olimp']['2']}")
            print(f"   Best:  1={f['best']['1']}, X={f['best']['X']}, 2={f['best']['2']}")
    
    # Save
    with open('forks_output.json', 'w', encoding='utf-8') as f:
        json.dump({'forks': forks[:50], 'total': len(forks)}, f, ensure_ascii=False, indent=2)
    
    print(f"\n💾 Saved {len(forks)} forks to forks_output.json")
    print(f"{'='*70}")

fast_scan()
