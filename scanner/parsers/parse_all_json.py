"""Universal parser bridge - calls Python Playwright parsers and outputs JSON"""
import sys, json, os, time
# Add project root to sys.path (2 levels up from scanner/parsers/)
project_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

PARSERS = {
    'winline': {
        'module': 'winline_playwright',
        'class': 'WinlinePlaywrightParser',
        'urls': [
            "https://winline.ru/football",
            "https://winline.ru/live/football",
            "https://winline.ru/basketball",
            "https://winline.ru/live/basketball",
        ]
    },
    'zenit': {
        'module': 'zenit_playwright',
        'class': 'ZenitPlaywrightParser',
        'urls': [
            "https://zenit.win/line/football",
            "https://zenit.win/live/football",
        ]
    },
    'betcity': {
        'module': 'betcity_playwright',
        'class': 'BetcityPlaywrightParser',
        'urls': [
            "https://betcity.ru/ru/line/football",
            "https://betcity.ru/ru/live/football",
        ]
    },
    'baltbet': {
        'module': 'baltbet_playwright',
        'class': 'BaltbetPlaywrightParser',
        'urls': [
            "https://baltbet.ru/line",
            "https://baltbet.ru/live",
        ]
    }
}

async def run_parser(name):
    if name not in PARSERS:
        return []
    
    config = PARSERS[name]
    module_name = config['module']
    class_name = config['class']
    urls = config['urls']
    
    try:
        module = __import__(f'scanner.parsers.{module_name}', fromlist=[class_name])
        parser_class = getattr(module, class_name)
        
        async with parser_class() as parser:
            parser.urls = urls
            events = await parser.get_events()
            return events
    except Exception as e:
        print(f"Error parsing {name}: {e}", file=sys.stderr)
        return []

async def main():
    if len(sys.argv) < 2:
        # Parse all parsers
        all_events = []
        for name in PARSERS:
            events = await run_parser(name)
            all_events.extend(events)
            print(f"Parsed {name}: {len(events)} events", file=sys.stderr)
        print(json.dumps(all_events, ensure_ascii=False, default=str))
    else:
        name = sys.argv[1]
        events = await run_parser(name)
        print(json.dumps(events, ensure_ascii=False, default=str))

if __name__ == '__main__':
    import asyncio
    asyncio.run(main())
