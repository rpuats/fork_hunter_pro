import requests, re, sys
text = requests.get('https://sportbook.sporthub.bet/widgets/sportbook/v1/modern/App-Bkt_j_3e.js', headers={'User-Agent':'Mozilla/5.0'}, timeout=30).text
patterns = [r'subscribe[A-Za-z0-9_]+', r'unsubscribe[A-Za-z0-9_]+', r'SportbookWSApi[^"\'\s`]{0,80}', r'treeWSApi[^"\'\s`]{0,80}', r'mutation[^"\'\s`]{0,80}', r'query[^"\'\s`]{0,80}', r'matches[^"\'\s`]{0,80}', r'tournaments[^"\'\s`]{0,80}', r'stakes[^"\'\s`]{0,80}']
out=[]
for pat in patterns:
    out.extend(re.findall(pat, text, flags=re.IGNORECASE))
out=sorted(set(out))
sys.stdout.buffer.write('\n'.join(out[:800]).encode('utf-8'))
