import json
d=json.load(open("winline_alter.json",encoding="utf-8"))
for key in ["t","c","b","l","e","m"]:
    v = d.get(key, [])
    if isinstance(v, list):
        print(f"{key}: {len(v)} items")
        if v and isinstance(v[0], dict):
            print(f"  Keys: {list(v[0].keys())[:10]}")
            if 'id' in v[0]:
                print(f"  First id: {v[0].get('id')}")
            if 'name' in v[0]:
                print(f"  First name: {v[0].get('name','')[:50]}")
    elif isinstance(v, dict):
        print(f"{key}: dict with {len(v)} keys")
    else:
        print(f"{key}: {v}")
