import sys
import io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.team_normalizer import team_normalizer

print('TEAM NORMALIZER CHECK:')
tests = [
    ('Манчестер Юнайтед', 'Манчестер Юн.'),
    ('Реал Мадрид', 'Реал М'),
    ('Барселона', 'Барса'),
]
for t1, t2 in tests:
    k1 = team_normalizer.get_key(t1, '')
    k2 = team_normalizer.get_key(t2, '')
    match = k1 == k2
    print(f'  "{t1}" vs "{t2}": {"MATCH" if match else "NO MATCH"}')
