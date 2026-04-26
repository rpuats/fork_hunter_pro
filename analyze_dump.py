import json

with open('winline_network_dump.json', 'r', encoding='utf-8') as f:
    data = json.load(f)

print('All API/XHR requests:')
print()

for req in data['all_requests']:
    url = req['url']
    if 'api' in url or 'wsys' in url or 'events' in url.lower():
        clean_url = url.split('?')[0]
        print(f'  {clean_url}')

print()
print('All responses with significant data:')
for resp in data['all_responses']:
    if resp['has_data'] and resp['size'] > 1000:
        url = resp['url'].split('?')[0]
        print(f'  {resp["status"]} {url[:80]}... ({resp["size"]} bytes)')

print()
print('Full dump structure:')
print(f"  Requests: {len(data['all_requests'])}")
print(f"  Responses: {len(data['all_responses'])}")
