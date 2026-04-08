import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.engine import GhostScanner, ScannerConfig

print('SCANNER ENGINE CHECK:')
print(f'  GhostScanner: {"OK" if GhostScanner else "FAIL"}')
print(f'  ScannerConfig: {"OK" if ScannerConfig else "FAIL"}')
