import requests, re, sys
js = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/widget.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
# pull likely endpoint/path literals
patterns = [
    r'"/[A-Za-z0-9_\-/?=&]{4,120}"',
    r"'/[A-Za-z0-9_\-/?=&]{4,120}'",
    r'[A-Za-z0-9_]+Subscribe[A-Za-z0-9_]*',
    r'[A-Za-z0-9_]+Query',
    r'[A-Za-z0-9_]+Mutation',
    r'[A-Za-z0-9_]+RouteBuilder',
    r'setSport\([^)]*\)',
    r'setCategory\([^)]*\)',
    r'setTournament\([^)]*\)',
    r'setEvent\([^)]*\)',
]
out = []
for pat in patterns:
    out.extend(re.findall(pat, js))
out = sorted(set(out))
filtered = [x for x in out if any(k in x.lower() for k in ['sport','match','event','stake','tournament','search','favorite','subscribe','query','mutation','route'])]
sys.stdout.buffer.write('\n'.join(filtered[:500]).encode('utf-8'))
