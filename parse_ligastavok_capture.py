import json

data = json.load(open('network_capture/ligastavok_network.json', encoding='utf-8'))

# Find API responses
api_responses = [r for r in data['responses'] if 'eventsList' in r['url'] or 'actionLines' in r['url'] or 'tournamentTree' in r['url']]

for resp in api_responses:
    url = resp['url']
    body_len = resp.get('body_length', 0)
    print(f"\n{'='*80}")
    print(f"URL: {url}")
    print(f"Body length: {body_len}")
    
    if body_len > 0 and body_len < 50000 and 'body' in resp:
        try:
            body = json.loads(resp['body'])
            print(f"Type: {type(body).__name__}")
            
            if isinstance(body, dict):
                print(f"Keys: {list(body.keys())}")
                result = body.get('result', {})
                if isinstance(result, dict):
                    items = result.get('data', [])
                    print(f"Items: {len(items)}")
                    if items:
                        item = items[0]
                        print(f"Item keys: {list(item.keys())}")
                        event = item.get('event', {})
                        print(f"Match: {event.get('team1', '')} vs {event.get('team2', '')}")
                        outcomes = item.get('outcomes', {})
                        print(f"Outcomes: {len(outcomes)}")
                        for key, out in list(outcomes.items())[:5]:
                            print(f"  {key}: title={out.get('title')}, value={out.get('value')}, adValue={out.get('adValue')}")
                elif isinstance(result, list):
                    print(f"Result is list with {len(result)} items")
            elif isinstance(body, list):
                print(f"Body is list with {len(body)} items")
                
        except json.JSONDecodeError as e:
            print(f"JSON error: {e}")
            print(f"Body: {resp.get('body', '')[:200]}")
