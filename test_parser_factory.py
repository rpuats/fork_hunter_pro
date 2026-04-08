import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers.factory import ParserFactory, auto_register_parsers

print('PARSER FACTORY CHECK:')
pf_status = "OK" if ParserFactory else "FAIL"
ar_status = "OK" if auto_register_parsers else "FAIL"
print(f'  ParserFactory: {pf_status}')
print(f'  auto_register_parsers: {ar_status}')
