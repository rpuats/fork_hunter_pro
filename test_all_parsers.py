import asyncio
import logging
import sys
import io

if sys.platform == 'win32':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

logging.basicConfig(level=logging.WARNING)

PARSERS_TO_TEST = [
    ('Winline', 'scanner.parsers.winline_playwright.WinlinePlaywrightParser'),
    ('Pari', 'scanner.parsers.pari_playwright.PariPlaywrightParser'),
    ('Betcity', 'scanner.parsers.betcity_playwright.BetcityPlaywrightParser'),
    ('Marathon', 'scanner.parsers.marathon_playwright.MarathonPlaywrightParser'),
    ('Zenit', 'scanner.parsers.zenit_playwright.ZenitPlaywrightParser'),
    ('Leon', 'scanner.parsers.leon_api.LeonParser'),
]

async def test_parser(name, module_path):
    try:
        module_path = module_path.replace('/', '.').replace('.py', '')
        parts = module_path.rsplit('.', 1)
        module_name, class_name = parts[0], parts[1]
        
        import importlib
        module = importlib.import_module(module_name)
        cls = getattr(module, class_name)
        parser = cls()
        
        events = await parser.get_events()
        return name, len(events), 'OK'
    except Exception as e:
        return name, 0, str(e)[:50]

async def main():
    print("\n" + "=" * 60)
    print("PARSER TEST RESULTS")
    print("=" * 60)
    
    tasks = [test_parser(name, path) for name, path in PARSERS_TO_TEST]
    results = await asyncio.gather(*tasks)
    
    total = 0
    for name, count, status in results:
        status_icon = "✅" if status == "OK" and count > 0 else "❌"
        print(f"{status_icon} {name:15} | {count:3} events | {status}")
        total += count
    
    print("=" * 60)
    print(f"TOTAL EVENTS: {total}")
    print("=" * 60)

if __name__ == '__main__':
    asyncio.run(main())
