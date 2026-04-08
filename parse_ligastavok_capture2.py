import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

with open('network_capture/ligastavok_network.json', encoding='utf-8') as f:
    data = json.load(f)

for resp in data['responses']:
    if 'eventsList' in resp['url'] and resp.get('body'):
        try:
            body = json.loads(resp['body'])
            result = body.get('result', {})
            if isinstance(result, dict):
                items = result.get('data', [])
                print(f"Found {len(items)} events")
                
                if items:
                    item = items[0]
                    event = item.get('event', {})
                    print(f"Match: {event.get('team1', '')} vs {event.get('team2', '')}")
                    print(f"League: {event.get('tournamentTitle', '')}")
                    
                    outcomes = item.get('outcomes', {})
                    print(f"Outcomes: {len(outcomes)}")
                    
                    for key, out in list(outcomes.items())[:10]:
                        title = out.get('title', '')
                        value = out.get('value', '')
                        adValue = out.get('adValue', '')
                        print(f"  {key}: title={title}, value={value}, adValue={adValue}")
                    
                    totals = [o for o in outcomes.values() if o.get('title') in ['Мен', 'Меньше', 'Under', 'ТМ', 'Бол', 'Больше', 'Over', 'ТБ']]
                    handicaps = [o for o in outcomes.values() if o.get('title', '').startswith('Ф') or o.get('title', '').startswith('H')]
                    print(f"\nTotals: {len(totals)}")
                    print(f"Handicaps: {len(handicaps)}")
                break
        except json.JSONDecodeError as e:
            print(f"JSON error: {e}")
            print(f"Body length: {len(resp.get('body', ''))}")
