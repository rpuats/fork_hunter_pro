#!/usr/bin/env python3
"""
Fork Scanner Engine - main entry point.
Fetches events from all BKs, normalizes, finds forks.
"""
import asyncio
import sys
import time
import json

sys.stdout.reconfigure(encoding='utf-8')


async def scan_forks(min_profit: float = 0.5, min_confidence: float = 0.8):
    """Main fork scanning function."""
    from scanner.parsers.leon_parser import LeonParser
    from scanner.parsers.pari_api import PariParser as PariApiParser
    from scanner.parsers.fonbet_api import FonbetParser as FonbetApiParser
    from scanner.parsers.bettery_api import BetteryParser
    from scanner.parsers.olimp_parser import OlimpParser as OlimpApiParser
    from scanner.core.normalizer import EventNormalizer
    from scanner.core.fork_calculator import ForkCalculator
    
    print("="*70)
    print("🔍 GHOST IMPERIUM — Fork Scanner")
    print("="*70)
    
    # Step 1: Fetch events from all BKs
    print("\n📥 Fetching events from bookmakers...")
    parsers = {
        'fonbet': FonbetApiParser(),
        'bettery': BetteryParser(),
        'pari': PariApiParser(),
        'leon': LeonParser(),
        'olimp': OlimpApiParser(),
    }
    
    all_events = {}
    total_events = 0
    
    for slug, parser in parsers.items():
        try:
            events = await parser.get_events()
            all_events[slug] = events
            total_events += len(events)
            print(f"  ✅ {slug}: {len(events)} events")
        except Exception as e:
            print(f"  ❌ {slug}: ERROR - {e}")
    
    print(f"\n📊 Total events: {total_events}")
    
    # Step 2: Find matched events between all BK pairs
    print(f"\n🔗 Matching events (min confidence: {min_confidence})...")
    bk_slugs = list(all_events.keys())
    all_matched = []
    
    for i in range(len(bk_slugs)):
        for j in range(i + 1, len(bk_slugs)):
            bk_a = bk_slugs[i]
            bk_b = bk_slugs[j]
            
            matches = EventNormalizer.match_events(
                all_events.get(bk_a, []),
                all_events.get(bk_b, []),
                min_confidence
            )
            
            for evt_a, evt_b, conf in matches:
                all_matched.append((evt_a, evt_b, conf))
            
            print(f"  {bk_a} ↔ {bk_b}: {len(matches)} matched")
    
    print(f"\n📊 Total matched pairs: {len(all_matched)}")
    
    # Step 3: Find forks
    print(f"\n💰 Finding forks (min profit: {min_profit}%)...")
    forks = ForkCalculator.find_all_forks(all_matched, min_profit=min_profit)
    
    print(f"\n🎯 Found {len(forks)} forks!")
    
    if forks:
        print(f"\n{'='*70}")
        print("TOP FORKS:")
        print(f"{'='*70}")
        
        for i, fork in enumerate(forks[:20], 1):
            print(f"\n#{i} 🏆 {fork['match']}")
            print(f"   Profit: {fork['profit_percent']}% | Payout: {fork['guaranteed_payout']}₽ | Profit: {fork['guaranteed_profit']}₽")
            print(f"   BKs: {fork['bookmakers']} | Confidence: {fork['confidence']}%")
            
            for bet in fork['bets']:
                print(f"   → {bet['outcome']}: {bet['bk']} @ {bet['odd']} (stake: {bet['stake']}₽)")
    
    # Save results
    results = {
        'timestamp': time.time(),
        'total_events': total_events,
        'matched_pairs': len(all_matched),
        'forks_found': len(forks),
        'forks': forks[:50],
    }
    
    with open('forks_output.json', 'w', encoding='utf-8') as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
    
    print(f"\n💾 Results saved to forks_output.json")
    print(f"{'='*70}")
    
    return forks


if __name__ == '__main__':
    asyncio.run(scan_forks(min_profit=0.5, min_confidence=0.8))
