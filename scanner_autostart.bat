@echo off
chcp 65001 >nul
title Ghost Imperium Scanner

echo.
echo  ╔══════════════════════════════════════════════════════════╗
echo  ║              GHOST IMPERIUM SCANNER                   ║
echo  ║         Professional Arbitrage Scanner                 ║
echo  ╚══════════════════════════════════════════════════════════╝
echo.

cd /d "%~dp0"

REM Check Python
python --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Python not found!
    pause
    exit /b 1
)

REM Install dependencies if needed
echo [INFO] Checking dependencies...
pip show playwright >nul 2>&1
if errorlevel 1 (
    echo [INFO] Installing playwright...
    pip install playwright
    playwright install chromium
)

REM Run scanner
echo.
echo [INFO] Starting scanner...
echo [INFO] Press Ctrl+C to stop
echo.

python -c "
import asyncio
import sys
import io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

from scanner.parsers import (
    WinlinePlaywrightParser,
    PariPlaywrightParser,
    BetcityPlaywrightParser,
    MarathonPlaywrightParser,
    ZenitPlaywrightParser,
)

async def scan_loop():
    parsers = [
        ('Winline', WinlinePlaywrightParser()),
        ('Pari', PariPlaywrightParser()),
        ('Betcity', BetcityPlaywrightParser()),
        ('Marathon', MarathonPlaywrightParser()),
        ('Zenit', ZenitPlaywrightParser()),
    ]
    
    from core.finder import SurebetCalculator
    calculator = SurebetCalculator()
    
    cycle = 0
    while True:
        cycle += 1
        print(f'\n--- Cycle {cycle} ---')
        
        all_events = []
        for name, parser in parsers:
            try:
                events = await parser.get_events()
                all_events.extend(events)
                print(f'  {name}: {len(events)} events')
            except Exception as e:
                print(f'  {name}: ERROR - {e}')
        
        print(f'  TOTAL: {len(all_events)} events')
        
        two_way = calculator.find_2way_surebets(all_events)
        three_way = calculator.find_3way_surebets(all_events)
        surebets = two_way + three_way
        
        if surebets:
            print(f'  [ALERT] Found {len(surebets)} surebets!')
            for sb in sorted(surebets, key=lambda x: x.get('profit_percent', 0), reverse=True)[:3]:
                print(f'    - {sb.get(\"event_name\")}: {sb.get(\"profit_percent\", 0):.1f}%')
        
        await asyncio.sleep(30)

try:
    asyncio.run(scan_loop())
except KeyboardInterrupt:
    print('\n[INFO] Scanner stopped')
"
