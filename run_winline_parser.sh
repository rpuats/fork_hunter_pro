#!/usr/bin/env bash
# Быстрый старт для парсеров Winline

echo "╔════════════════════════════════════════════════════════════╗"
echo "║         WINLINE WORKING PARSER - QUICK START              ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Проверка Python
if ! command -v python3 &> /dev/null; then
    echo "❌ Python3 not found. Please install Python 3.8+"
    exit 1
fi

echo "✅ Python found: $(python3 --version)"
echo ""

# Проверка pip
if ! command -v pip &> /dev/null; then
    pip3=$(python3 -m pip --version)
    if [ $? -ne 0 ]; then
        echo "❌ pip not found"
        exit 1
    fi
    echo "✅ pip found: $pip3"
else
    echo "✅ pip found: $(pip --version)"
fi

echo ""
echo "📦 Installing Playwright..."
pip3 install -q playwright aiohttp

if [ $? -ne 0 ]; then
    echo "❌ Failed to install Playwright"
    exit 1
fi

echo "✅ Playwright installed"
echo ""

echo "🌐 Installing Chromium browser..."
python3 -m playwright install chromium

if [ $? -ne 0 ]; then
    echo "❌ Failed to install Chromium"
    exit 1
fi

echo "✅ Chromium installed"
echo ""

echo "╔════════════════════════════════════════════════════════════╗"
echo "║ INSTALLATION COMPLETE                                      ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

echo "🚀 Running Winline parser..."
echo ""

python3 winline_working_parser.py

echo ""
echo "For advanced testing, run:"
echo "  python3 winline_advanced_parser.py"
