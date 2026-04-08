import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from api.websocket import ws_manager

print('WEBSOCKET MANAGER CHECK:')
print(f'  ws_manager: {"OK" if ws_manager else "FAIL"}')
