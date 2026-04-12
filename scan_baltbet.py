import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import BaltbetRegexParser

def scan():
    p = BaltbetRegexParser()
    events = p.get_events()
    print(f'BALTBET: {len(events)} events found')
    for e in events[:3]:
        print(f"  Odds: {e['home_odds']}/{e['draw_odds']}/{e['away_odds']}")

if __name__ == '__main__':
    scan()
