import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.event_pool import EventPool

print('EVENT POOL CHECK:')
pool = EventPool(max_size=1000)
status = "OK" if pool else "FAIL"
print(f'  EventPool class: {status}')
