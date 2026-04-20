#!/usr/bin/env bash
# 🚀 Fork-OS Quick Start - One-liner to launch everything

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}"
echo "╔════════════════════════════════════════════════════════════════════════════╗"
echo "║                    🚀 FORK-OS PARALLEL AGENTS SYSTEM                       ║"
echo "║                    Запуск всех агентов одновременно                        ║"
echo "╚════════════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Check prerequisites
echo -e "${BLUE}ℹ️  Checking prerequisites...${NC}"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Rust not installed! Install from https://rustup.rs/${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Rust/Cargo found${NC}"

if ! command -v python3 &> /dev/null; then
    echo -e "${RED}❌ Python3 not installed!${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Python3 found${NC}"

# Build project
echo -e "${BLUE}ℹ️  Building Rust project...${NC}"
cargo build --release 2>&1 | tail -5

if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Build successful${NC}"

# Create results directory
mkdir -p agent_results

# Function to start agent in background
start_agent() {
    local name=$1
    local script=$2
    local emoji=$3
    
    echo -e "${GREEN}${emoji} Starting: ${name}${NC}"
    
    # Run in background with output redirection
    eval "$script" > "agent_results/${name}.log" 2>&1 &
    local pid=$!
    echo $pid > "agent_results/${name}.pid"
    
    echo -e "${GREEN}✓ ${name} started [PID: ${pid}]${NC}"
}

# Start all agents
echo -e "\n${CYAN}Launching 12 agents in parallel...${NC}\n"

# 1. Parser Agents (7)
start_agent "pari_parser" "cargo run --release --bin fork-hunter-bin -- --parser pari" "🕷️"
start_agent "fonbet_parser" "cargo run --release --bin fork-hunter-bin -- --parser fonbet" "🕷️"
start_agent "bettery_parser" "cargo run --release --bin fork-hunter-bin -- --parser bettery" "🕷️"
start_agent "marathon_parser" "cargo run --release --bin fork-hunter-bin -- --parser marathon" "🕷️"
start_agent "24bet_parser" "cargo run --release --bin fork-hunter-bin -- --parser 24bet" "🕷️"
start_agent "leon_parser" "cargo run --release --bin fork-hunter-bin -- --parser leon" "🕷️"
start_agent "sportbet_parser" "cargo run --release --bin fork-hunter-bin -- --parser sportbet" "🕷️"

# 2. Engine Agents (2)
start_agent "calculator" "cargo run --release --bin fork-hunter-bin -- --mode calculate" "🧮"
start_agent "normalizer" "cargo run --release --bin fork-hunter-bin -- --mode normalize" "🧹"

# 3. Helper Agents (3)
start_agent "cross_bk_matcher" "cargo run --release --bin fork-hunter-bin -- --mode match" "🔀"
start_agent "problem_debugger" "python3 debug_problem_bks.py" "🔍"
start_agent "parser_optimizer" "python3 optimize_parsers.py" "⚡"

sleep 2

# Print summary
echo -e "\n${CYAN}═════════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}✅ ALL AGENTS STARTED${NC}"
echo -e "${CYAN}═════════════════════════════════════════════════════════════════════════════${NC}\n"

echo -e "${YELLOW}📊 Running Agents:${NC}"
echo "  • Pari Parser (🕷️)"
echo "  • Fonbet Parser (🕷️)"
echo "  • Bettery Parser (🕷️)"
echo "  • Marathon Parser (🕷️)"
echo "  • 24bet Parser (🕷️)"
echo "  • Leon Parser (🕷️)"
echo "  • Sportbet Parser (🕷️)"
echo "  • Surebet Calculator (🧮)"
echo "  • Event Normalizer (🧹)"
echo "  • Cross-BK Matcher (🔀)"
echo "  • Problem BK Debugger (🔍)"
echo "  • Parser Optimizer (⚡)"

echo -e "\n${YELLOW}🌐 API Access:${NC}"
echo "  • Health: http://localhost:8080/api/v1/health"
echo "  • Metrics: http://localhost:8080/api/v1/metrics"
echo "  • Surebets: http://localhost:8080/api/v1/surebets"
echo "  • WebSocket: ws://localhost:8080/ws"

echo -e "\n${YELLOW}📁 Logs & Results:${NC}"
echo "  • Agent logs: agent_results/*.log"
echo "  • Agent PIDs: agent_results/*.pid"
echo "  • Generated ideas: generated_ideas.jsonl"
echo "  • Parser metrics: parser_performance.json"

echo -e "\n${YELLOW}📈 Monitor in Real-Time:${NC}"
echo "  # Watch API metrics"
echo "  while true; do clear; curl http://localhost:8080/api/v1/metrics 2>/dev/null | jq .; sleep 5; done"
echo ""
echo "  # Watch agent logs"
echo "  tail -f agent_results/*.log"
echo ""
echo "  # Watch parser health"
echo "  watch -n5 'curl http://localhost:8080/api/v1/parsers/health 2>/dev/null | jq .'"

echo -e "\n${YELLOW}⏹️  To Stop All Agents:${NC}"
echo "  # Stop individual agent"
echo "  kill \$(cat agent_results/pari_parser.pid)"
echo ""
echo "  # Stop all agents"
echo "  for pid in agent_results/*.pid; do kill \$(cat \$pid) 2>/dev/null; done"
echo ""
echo "  # Or run stop script"
echo "  chmod +x ./stop_agents.sh && ./stop_agents.sh"

echo -e "\n${GREEN}═════════════════════════════════════════════════════════════════════════════${NC}\n"

# Keep running and show status
echo -e "${CYAN}Monitoring agents... (press Ctrl+C to stop)${NC}\n"

while true; do
    alive=0
    for pid_file in agent_results/*.pid; do
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            if kill -0 $pid 2>/dev/null; then
                alive=$((alive + 1))
            else
                agent_name=$(basename "$pid_file" .pid)
                echo -e "${YELLOW}⚠️  Agent ${agent_name} exited${NC}"
                rm "$pid_file"
            fi
        fi
    done
    
    timestamp=$(date '+%H:%M:%S')
    echo -e "[$timestamp] Agents running: ${alive}/12"
    sleep 30
done
