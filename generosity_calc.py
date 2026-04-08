import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')

async def calc():
    all_events = []
    parsers = __import__('scanner.parsers', fromlist=['WinlinePlaywrightParser', 'PariPlaywrightParser', 'ZenitPlaywrightParser', 'BaltbetPlaywrightParser'])
    for name in ['WinlinePlaywrightParser', 'PariPlaywrightParser', 'ZenitPlaywrightParser', 'BaltbetPlaywrightParser']:
        try:
            p = getattr(parsers, name)()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            short_name = name.replace("PlaywrightParser", "")
            print(f'{short_name}: {len(events)} events')
        except Exception as e:
            print(f'{name}: FAILED - {e}')
    
    from collections import defaultdict
    bk_margins = defaultdict(list)
    for e in all_events:
        bk = e.get('bookmaker', '')
        h = e.get('home_odds', 0)
        d = e.get('draw_odds', 0) or 0
        a = e.get('away_odds', 0)
        if h > 1 and a > 1:
            margin = (1/h + 1/(d if d > 1 else 3.0) + 1/a - 1) * 100
            bk_margins[bk].append(margin)
    
    print('\nAVG MARGIN PER BK (lower = more generous):')
    for bk, margins in sorted(bk_margins.items(), key=lambda x: sum(x[1])/len(x[1])):
        avg = sum(margins) / len(margins)
        print(f'  {bk}: {avg:.2f}% margin ({len(margins)} events)')

asyncio.run(calc())
