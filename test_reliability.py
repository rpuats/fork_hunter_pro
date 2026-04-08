import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from services.reliability import ReliabilityScorer

print('RELIABILITY SCORER CHECK:')
rs = ReliabilityScorer()
status = "OK" if rs else "FAIL"
print(f'  ReliabilityScorer class: {status}')
