import requests, re, sys
base = 'https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/'
js = requests.get(base + 'App-Bkt_j_3e.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
patterns = [
    r'"/[A-Za-z0-9_\-/?=&]{4,180}"',
    r"'/[A-Za-z0-9_\-/?=&]{4,180}'",
    r'[A-Za-z0-9_]+Subscribe[A-Za-z0-9_]*',
    r'[A-Za-z0-9_]+Query',
    r'[A-Za-z0-9_]+Mutation',
    r'[A-Za-z0-9_]+Api',
    r'tree_ws[^"\'\s`]{0,60}',
    r'[A-Za-z0-9_]+stake[A-Za-z0-9_]*',
    r'[A-Za-z0-9_]+match[A-Za-z0-9_]*',
    r'[A-Za-z0-9_]+event[A-Za-z0-9_]*',
    r'[A-Za-z0-9_]+tournament[A-Za-z0-9_]*',
]
out = []
for pat in patterns:
    out.extend(re.findall(pat, js, flags=re.IGNORECASE))
out = sorted(set(out))
sys.stdout.buffer.write('\n'.join(out[:800]).encode('utf-8'))
