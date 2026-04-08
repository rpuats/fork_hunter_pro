import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.team_normalizer import team_normalizer
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser, ZenitPlaywrightParser

async def find():
    all_events = []
    for name, cls in [('Winline', WinlinePlaywrightParser), ('Pari', PariPlaywrightParser), ('Zenit', ZenitPlaywrightParser)]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            for e in events:
                e['_source'] = name
                all_events.append(e)
            print(f'{name}: {len(events)} events')
        except Exception as ex:
            print(f'{name}: ERROR - {ex}')
    
    # Show sample events and their normalized keys
    print('\n--- SAMPLE EVENTS ---')
    for e in all_events[:5]:
        home = e.get('home_team', '')
        away = e.get('away_team', '')
        key = team_normalizer.get_key(home, away)
        print(f'{e["_source"]}: {home} vs {away} -> key: {key}')
    
    # Check all unique keys
    from collections import defaultdict
    groups = defaultdict(list)
    for e in all_events:
        home = e.get('home_team', '')
        away = e.get('away_team', '')
        key = team_normalizer.get_key(home, away)
        groups[key].append(e)
    
    print(f'\nTotal unique keys: {len(groups)}')
    print(f'Total events: {len(all_events)}')
    
    # Show keys with multiple events (even from same BK)
    multi = {k: v for k, v in groups.items() if len(v) > 1}
    print(f'Keys with 2+ events: {len(multi)}')
    for k, events in list(multi.items())[:5]:
        sources = [e['_source'] for e in events]
        print(f'  {k}: {sources}')
    
    # Find matches with 2+ bookmakers
    multi_bk = {k: v for k, v in groups.items() if len(set(e['_source'] for e in v)) >= 2}
    print(f'\nMATCHES ACROSS BKs: {len(multi_bk)}')
    for k, events in list(multi_bk.items())[:10]:
        sources = set(e['_source'] for e in events)
        print(f'  {k}: {sources}')

asyncio.run(find())
