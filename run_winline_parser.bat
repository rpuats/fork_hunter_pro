@echo off
REM Быстрый старт для Windows - Winline Working Parser

echo.
echo ============================================================
echo.   WINLINE WORKING PARSER - QUICK START
echo.
echo ============================================================
echo.

REM Проверка Python
python --version >nul 2>&1
if %errorlevel% neq 0 (
    echo X Python not found. Please install Python 3.8+
    echo   Download from: https://www.python.org/downloads/
    exit /b 1
)

for /f "tokens=*" %%i in ('python --version') do set PYTHON_VERSION=%%i
echo.  OK %PYTHON_VERSION%

echo.
echo. Installing Playwright and dependencies...
python -m pip install -q playwright aiohttp

if %errorlevel% neq 0 (
    echo X Failed to install dependencies
    exit /b 1
)

echo.  OK Playwright installed

echo.
echo. Installing Chromium browser (this takes 1-2 minutes)...
python -m playwright install chromium

if %errorlevel% neq 0 (
    echo X Failed to install Chromium
    exit /b 1
)

echo.  OK Chromium installed

echo.
echo ============================================================
echo.
echo. INSTALLATION COMPLETE - Running parser...
echo.
echo ============================================================
echo.

python winline_working_parser.py

echo.
echo. Done! Results saved to winline_events.json
echo.
echo. For advanced testing, run:
echo.   python winline_advanced_parser.py
echo.
pause
