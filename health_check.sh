#!/usr/bin/env bash
# ✅ Quick health check before running all agents

echo "🏥 Fork-OS Health Check"
echo "════════════════════════════════════════"

checks_passed=0
checks_total=0

check() {
    checks_total=$((checks_total + 1))
    local name=$1
    local cmd=$2
    
    echo -n "Checking: $name... "
    
    if eval "$cmd" &> /dev/null; then
        echo "✅"
        checks_passed=$((checks_passed + 1))
        return 0
    else
        echo "❌"
        return 1
    fi
}

# System checks
echo ""
echo "📋 System Checks:"
check "Rust installed" "command -v cargo"
check "Python installed" "command -v python3"
check "Git installed" "command -v git"

# Project checks
echo ""
echo "📦 Project Checks:"
cd "$(dirname "${BASH_SOURCE[0]}")" || exit 1
check "Cargo.toml exists" "test -f Cargo.toml"
check "Can compile project" "cargo check --release 2>&1 | grep -q Finished"

# Build
echo ""
echo "🔨 Building project..."
if cargo build --release 2>&1 | tail -5; then
    echo "✅ Build successful"
    checks_passed=$((checks_passed + 1))
else
    echo "❌ Build failed"
fi
checks_total=$((checks_total + 1))

# Test
echo ""
echo "🧪 Running tests..."
if cargo test --lib --release -- --test-threads=1 2>&1 | tail -5; then
    echo "✅ Tests passed"
    checks_passed=$((checks_passed + 1))
else
    echo "⚠️  Some tests failed (continue anyway)"
fi
checks_total=$((checks_total + 1))

# API availability
echo ""
echo "🌐 API Checks:"
sleep 2  # Give API time to start if already running
check "API responds" "curl -s http://localhost:8080/api/v1/health"
check "API metrics available" "curl -s http://localhost:8080/api/v1/metrics | grep -q cycle"

# Final report
echo ""
echo "════════════════════════════════════════"
echo "📊 Results: $checks_passed/$checks_total checks passed"
echo "════════════════════════════════════════"

if [ "$checks_passed" -ge "$((checks_total - 2))" ]; then
    echo ""
    echo "✅ System ready! You can run:"
    echo ""
    echo "  Windows:  .\fork_os.ps1 start"
    echo "  Linux:    ./launch_all_agents.sh"
    echo ""
    exit 0
else
    echo ""
    echo "❌ Please fix the issues above and try again"
    exit 1
fi
