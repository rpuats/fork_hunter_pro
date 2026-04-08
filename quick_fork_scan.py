#!/usr/bin/env python3
"""
Quick Fork Scanner - tests normalizer + calculator on real data.
"""
import asyncio
import sys
import time
import json

sys.stdout.reconfigure(encoding='utf-8')

async def quick_scan():
    print("="*70)
    print("🔍 GHOST IMPERIUM — Quick Fork Scanner")
    print("="*70)
    
    # Fetch from 2 fast BKs first (API-based, no Playwright)
    print("\n📥 Fetching events...")
    
    from scanner.parsers.leon_parser import LeonParser
    from scanner.parsers.olimp_parser import OlimpParser as OlimpApiParser
    
    t0 = time.time()
    
    # Fetch Leon + Olimp in parallel
    leon_p = LeonParser()
    olimp_p = OlimpApiParser()
    
    leon_events, olimp_events = await asyncio.gather(
        leon_p.get_events(),
        olimp_p.get_events(),
        return_exceptions=True
    )
    
    if isinstance(leon_events, Exception):
        print(f"  ❌ Leon: {leon_events}")
        leon_events = []
    if isinstance(olimp_events, Exception):
        print(f"  ❌ OlimpBet: {olimp_events}")
        olimp_events = []
    
    print(f"  ✅ Leon: {len(leon_events)} events ({time.time()-t0:.1f}s)")
    print(f"  ✅ OlimpBet: {len(olimp_events)} events ({time.time()-t0:.1f}s)")
    
    # Normalize and match
    from scanner.core.normalizer import EventNormalizer
    from scanner.core.fork_calculator import ForkCalculator
    
    print(f"\n🔗 Matching events...")
    matches = EventNormalizer.match_events(leon_events, olimp_events, min_confidence=0.8)
    print(f"  Matched: {len(matches)} pairs")
    
    # Find forks
    print(f"\n💰 Finding forks...")
    forks = ForkCalculator.find_all_forks(matches, min_profit=0.5)
    print(f"  Found: {len(forks)} forks!")
    
    if forks:
        print(f"\n{'='*70}")
        print("TOP FORKS:")
        print(f"{'='*70}")
        
        for i, fork in enumerate(forks[:10], 1):
            print(f"\n#{i} 🏆 {fork['match']}")
            print(f"   Profit: {fork['profit_percent']}% | Payout: {fork['guaranteed_payout']}₽")
            print(f"   BKs: {fork['bookmakers']} | Live: {'✅' if fork.get('is_live') else '❌'}")
            for bet in fork['bets']:
                print(f"   → {bet['outcome']}: {bet['bk']} @ {bet['odd']} (stake: {bet['stake']}₽)")
    
    # Save
    results = {
        'timestamp': time.time(),
        'leon_events': len(leon_events),
        'olimp_events': len(olimp_events),
        'matched': len(matches),
        'forks': len(forks),
        'top_forks': forks[:20],
    }
    
    with open('forks_output.json', 'w', encoding='utf-8') as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
    
    print(f"\n💾 Saved to forks_output.json")
    print(f"{'='*70}")
    
    return forks

asyncio.run(quick_scan())
