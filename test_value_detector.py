import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.value_detector import ValueBetDetector

print('VALUE BET DETECTOR CHECK:')
detector = ValueBetDetector(min_edge=2.0)
status = "OK" if detector else "FAIL"
print(f'  ValueBetDetector class: {status}')
