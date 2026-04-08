import asyncio
import aiohttp
import json

async def test_leon():
    url = 'https://leon.ru/api-2/betline/headline-matches?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc&merged=true'
    headers = {
        'Accept': 'application/json',
        'Referer': 'https://leon.ru',
    }
    
    async with aiohttp.ClientSession() as session:
        async with session.get(url, headers=headers, timeout=aiohttp.ClientTimeout(total=15)) as resp:
            print(f'Status: {resp.status}')
            data = await resp.json()
            print(f'Keys: {list(data.keys())}')
            events_data = data.get('events', {})
            events = events_data.get('events', []) if isinstance(events_data, dict) else []
            print(f'Events: {len(events)}')
            for e in events[:5]:
                comps = e.get('competitors', [])
                if len(comps) >= 2:
                    print(f'  {comps[0].get("name")} vs {comps[1].get("name")}')

asyncio.run(test_leon())
