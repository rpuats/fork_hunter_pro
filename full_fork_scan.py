#!/usr/bin/env python3
"""
Full Fork Scanner - scans ALL available bookmakers via REST API.
"""
import asyncio
import sys
import time
import json

sys.stdout.reconfigure(encoding='utf-8')

async def full_scan():
    print("="*70)
    print("🔍 GHOST IMPERIUM — Full Fork Scanner v2")
    print("="*70)
    
    from scanner.core.normalizer import EventNormalizer
    from scanner.core.fork_calculator import ForkCalculator
    
    # Fetch from ALL API-based BKs in parallel
    print("\n📥 Fetching events from all bookmakers...")
    t0 = time.time()
    
    parsers = {}
    
    # Leon
    try:
        from scanner.parsers.leon_parser import LeonParser
        parsers['leon'] = LeonParser()
    except: pass
    
    # Pari
    try:
        from scanner.parsers.pari_api import PariParser
        parsers['pari'] = PariParser()
    except: pass
    
    # Fonbet
    try:
        from scanner.parsers.fonbet_api import FonbetParser
        parsers['fonbet'] = FonbetParser()
    except: pass
    
    # Bettery
    try:
        from scanner.parsers.bettery_api import BetteryParser
        parsers['bettery'] = BetteryParser()
    except: pass
    
    # OlimpBet
    try:
        from scanner.parsers.olimp_parser import OlimpParser
        parsers['olimp'] = OlimpParser()
    except: pass
    
    # MarathonBet
    try:
        from scanner.parsers.marathon_api import MarathonApiParser
        parsers['marathon'] = MarathonApiParser()
    except: pass
    
    # 24bet
    try:
        from scanner.parsers._24bet_api import _24betApiParser
        parsers['24bet'] = _24betApiParser()
    except: pass
    
    # Sportbet
    try:
        from scanner.parsers.sportbet_api import SportbetApiParser
        parsers['sportbet'] = SportbetApiParser()
    except: pass
    
    print(f"  Found {len(parsers)} parsers")
    
    # Fetch all in parallel
    async def fetch_one(slug, parser):
        try:
            events = await parser.get_events()
            return slug, events, None
        except Exception as e:
            return slug, [], str(e)
    
    tasks = [fetch_one(slug, p) for slug, p in parsers.items()]
    results = await asyncio.gather(*tasks, return_exceptions=True)
    
    all_events = {}
    total_events = 0
    
    for result in results:
        if isinstance(result, Exception):
            continue
        slug, events, error = result
        if error:
            print(f"  ❌ {slug}: {error}")
        else:
            all_events[slug] = events
            total_events += len(events)
            print(f"  ✅ {slug}: {len(events)} events")
    
    elapsed = time.time() - t0
    print(f"\n📊 Total: {total_events} events from {len(all_events)} BKs ({elapsed:.1f}s)")
    
    # Find forks between all pairs
    print(f"\n🔗 Finding forks...")
    t1 = time.time()
    
    all_forks = []
    bk_slugs = list(all_events.keys())
    
    for i in range(len(bk_slugs)):
        for j in range(i + 1, len(bk_slugs)):
            bk_a = bk_slugs[i]
            bk_b = bk_slugs[j]
            
            matches = EventNormalizer.match_events(
                all_events[bk_a], all_events[bk_b], min_confidence=0.85
            )
            
            forks = ForkCalculator.find_all_forks(matches, min_profit=0.5)
            all_forks.extend(forks)
            
            if matches:
                print(f"  {bk_a} ↔ {bk_b}: {len(matches)} matched, {len(forks)} forks")
    
    all_forks.sort(key=lambda x: -x.get('profit_percent', 0))
    
    elapsed2 = time.time() - t1
    print(f"\n🎯 Total forks found: {len(all_forks)} ({elapsed2:.1f}s)")
    
    if all_forks:
        print(f"\n{'='*70}")
        print("TOP 20 FORKS:")
        print(f"{'='*70}")
        
        for i, fork in enumerate(all_forks[:20], 1):
            print(f"\n#{i} 🏆 {fork.get('match', '?')}")
            print(f"   Profit: {fork.get('profit_percent')}% | Payout: {fork.get('guaranteed_payout')}₽")
            print(f"   BKs: {fork.get('bookmakers', '?')}")
            for bet in fork.get('bets', []):
                print(f"   → {bet['outcome']}: {bet['bk']} @ {bet['odd']} (stake: {bet['stake']}₽)")
    
    # Save
    results = {
        'timestamp': time.time(),
        'total_events': total_events,
        'bks': {k: len(v) for k, v in all_events.items()},
        'forks_found': len(all_forks),
        'top_forks': all_forks[:50],
    }
    
    with open('forks_full_output.json', 'w', encoding='utf-8') as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
    
    print(f"\n💾 Saved to forks_full_output.json")
    print(f"{'='*70}")

asyncio.run(full_scan())
