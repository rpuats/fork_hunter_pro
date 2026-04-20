#!/usr/bin/env python3
"""
🚀 FORK-OS PARALLEL AGENTS ORCHESTRATOR
Запускает 10+ агентов параллельно для непрерывного сканирования и оптимизации
"""

import asyncio
import subprocess
import sys
import os
import json
import time
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed

# Colors for output
class Colors:
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKCYAN = '\033[96m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'

# Agent definitions
AGENTS = {
    # 7 Parsers (one per BK)
    "pari_parser": {
        "name": "Pari Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser pari",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "fonbet_parser": {
        "name": "Fonbet Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser fonbet",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "bettery_parser": {
        "name": "Bettery Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser bettery",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "marathon_parser": {
        "name": "Marathon Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser marathon",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "24bet_parser": {
        "name": "24bet Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser 24bet",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "leon_parser": {
        "name": "Leon Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser leon",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    "sportbet_parser": {
        "name": "Sportbet Parser",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --parser sportbet",
        "timeout": 300,
        "category": "parser",
        "emoji": "🕷️"
    },
    
    # Core Engine Agents (2)
    "calculator": {
        "name": "Surebet Calculator",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --mode calculate",
        "timeout": 300,
        "category": "engine",
        "emoji": "🧮"
    },
    "normalizer": {
        "name": "Event Normalizer",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --mode normalize",
        "timeout": 300,
        "category": "engine",
        "emoji": "🧹"
    },
    
    # Specialized Agents (3)
    "cross_bk_matcher": {
        "name": "Cross-BK Matcher",
        "cmd": "cargo run --release --bin fork-hunter-bin -- --mode match",
        "timeout": 300,
        "category": "analysis",
        "emoji": "🔀"
    },
    "problem_debugger": {
        "name": "Problem BK Debugger (Olimp, Zenit, Betcity)",
        "cmd": "python debug_problem_bks.py",
        "timeout": 600,
        "category": "debug",
        "emoji": "🔍"
    },
    "optimizer": {
        "name": "Parser Optimizer & Profiler",
        "cmd": "python optimize_parsers.py",
        "timeout": 600,
        "category": "optimization",
        "emoji": "⚡"
    },
}

class AgentStatus:
    def __init__(self, name: str):
        self.name = name
        self.status = "pending"
        self.start_time = None
        self.end_time = None
        self.error = None
        self.output = []
        self.process = None
        self.return_code = None

    def duration(self):
        if self.start_time and self.end_time:
            return self.end_time - self.start_time
        elif self.start_time:
            return time.time() - self.start_time
        return 0

    def to_dict(self):
        return {
            "name": self.name,
            "status": self.status,
            "duration": f"{self.duration():.1f}s",
            "return_code": self.return_code,
            "error": self.error[:200] if self.error else None,
        }

class AgentOrchestrator:
    def __init__(self):
        self.agents: Dict[str, AgentStatus] = {}
        self.results_dir = Path("agent_results")
        self.results_dir.mkdir(exist_ok=True)
        self.start_time = None
        self.executor = ThreadPoolExecutor(max_workers=12)  # 12 parallel agents

    def run_all(self):
        """Run all agents in parallel"""
        print(f"{Colors.HEADER}{Colors.BOLD}")
        print("=" * 80)
        print("🚀 FORK-OS PARALLEL AGENTS ORCHESTRATOR")
        print("=" * 80)
        print(f"{Colors.ENDC}")
        
        self.start_time = time.time()
        
        # Initialize agent statuses
        for agent_id, agent_config in AGENTS.items():
            self.agents[agent_id] = AgentStatus(agent_config["name"])

        print(f"\n{Colors.BOLD}Starting {len(AGENTS)} agents in parallel...{Colors.ENDC}\n")
        self._print_agent_list()
        print(f"\n{Colors.OKCYAN}Launching all agents...{Colors.ENDC}\n")

        # Submit all agents to executor
        futures = {}
        for agent_id, agent_config in AGENTS.items():
            future = self.executor.submit(self._run_agent, agent_id, agent_config)
            futures[agent_id] = future

        # Monitor progress
        completed = 0
        while completed < len(futures):
            done_count = sum(1 for f in futures.values() if f.done())
            if done_count > completed:
                completed = done_count
                self._print_status_summary()
            time.sleep(5)

        # Wait for all to complete and print final summary
        for agent_id, future in futures.items():
            try:
                future.result()
            except Exception as e:
                print(f"{Colors.FAIL}Agent {agent_id} exception: {e}{Colors.ENDC}")

        self._print_final_summary()

    def _run_agent(self, agent_id: str, agent_config: Dict):
        """Run a single agent"""
        agent_status = self.agents[agent_id]
        agent_status.status = "running"
        agent_status.start_time = time.time()

        emoji = agent_config.get("emoji", "⚙️")
        print(f"{emoji} {Colors.OKBLUE}Starting:{Colors.ENDC} {agent_config['name']}")

        try:
            # Run the command
            cmd = agent_config["cmd"]
            print(f"   Command: {cmd}")
            
            process = subprocess.Popen(
                cmd,
                shell=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            agent_status.process = process

            # Wait for completion with timeout
            try:
                stdout, stderr = process.communicate(timeout=agent_config["timeout"])
                agent_status.return_code = process.returncode
                
                if process.returncode == 0:
                    agent_status.status = "completed"
                    print(f"✅ {agent_config['name']}: SUCCESS")
                else:
                    agent_status.status = "failed"
                    agent_status.error = stderr[:500]
                    print(f"❌ {agent_config['name']}: FAILED (exit code {process.returncode})")
                    if stderr:
                        print(f"   Error: {stderr[:200]}")
                
                agent_status.output = stdout.split('\n') if stdout else []
                
            except subprocess.TimeoutExpired:
                process.kill()
                agent_status.status = "timeout"
                agent_status.error = f"Timeout after {agent_config['timeout']}s"
                print(f"⏱️  {agent_config['name']}: TIMEOUT")

        except Exception as e:
            agent_status.status = "error"
            agent_status.error = str(e)
            print(f"⚠️  {agent_config['name']}: ERROR - {e}")

        agent_status.end_time = time.time()
        
        # Save results
        self._save_agent_results(agent_id, agent_status)

    def _save_agent_results(self, agent_id: str, status: AgentStatus):
        """Save agent results to file"""
        result_file = self.results_dir / f"{agent_id}_result.json"
        with open(result_file, "w") as f:
            json.dump(status.to_dict(), f, indent=2)

    def _print_agent_list(self):
        """Print list of agents to run"""
        print(f"{Colors.BOLD}Agents to run:{Colors.ENDC}")
        
        by_category = {}
        for agent_id, config in AGENTS.items():
            cat = config.get("category", "other")
            if cat not in by_category:
                by_category[cat] = []
            by_category[cat].append((agent_id, config))

        for category in ["parser", "engine", "analysis", "debug", "optimization"]:
            if category in by_category:
                agents = by_category[category]
                cat_name = {
                    "parser": "🕷️  Parsers (7)",
                    "engine": "🧮 Engine (2)",
                    "analysis": "🔀 Analysis",
                    "debug": "🔍 Debugging",
                    "optimization": "⚡ Optimization"
                }.get(category, category)
                
                print(f"\n  {cat_name}:")
                for agent_id, config in agents:
                    print(f"    - {config['emoji']} {config['name']}")

    def _print_status_summary(self):
        """Print current status of all agents"""
        print(f"\n{Colors.OKGREEN}{'='*80}{Colors.ENDC}")
        print(f"{Colors.BOLD}Agent Status Summary{Colors.ENDC}")
        print(f"{Colors.OKGREEN}{'='*80}{Colors.ENDC}")
        
        status_counts = {}
        for status in self.agents.values():
            s = status.status
            status_counts[s] = status_counts.get(s, 0) + 1

        total_time = time.time() - self.start_time
        print(f"\nElapsed: {total_time:.0f}s")
        print(f"Status breakdown: ", end="")
        
        status_symbols = {
            "pending": "⏳",
            "running": "🏃",
            "completed": "✅",
            "failed": "❌",
            "timeout": "⏱️",
            "error": "⚠️"
        }
        
        for status, count in sorted(status_counts.items()):
            symbol = status_symbols.get(status, "?")
            print(f"{symbol} {status}={count} ", end="")
        print("\n")

    def _print_final_summary(self):
        """Print final summary of all agents"""
        print(f"\n{Colors.OKGREEN}{'='*80}{Colors.ENDC}")
        print(f"{Colors.HEADER}{Colors.BOLD}🎯 FINAL RESULTS{Colors.ENDC}")
        print(f"{Colors.OKGREEN}{'='*80}{Colors.ENDC}\n")

        # Count by status
        by_status = {}
        for agent_id, status in self.agents.items():
            s = status.status
            if s not in by_status:
                by_status[s] = []
            by_status[s].append((agent_id, status))

        # Print successful agents
        if "completed" in by_status:
            print(f"{Colors.OKGREEN}✅ COMPLETED ({len(by_status['completed'])}){Colors.ENDC}")
            for agent_id, status in by_status["completed"]:
                agent_config = AGENTS[agent_id]
                emoji = agent_config.get("emoji", "⚙️")
                print(f"   {emoji} {status.name} ({status.duration():.1f}s)")

        # Print failed agents
        if "failed" in by_status:
            print(f"\n{Colors.FAIL}❌ FAILED ({len(by_status['failed'])}){Colors.ENDC}")
            for agent_id, status in by_status["failed"]:
                agent_config = AGENTS[agent_id]
                emoji = agent_config.get("emoji", "⚙️")
                print(f"   {emoji} {status.name}")
                if status.error:
                    print(f"      Error: {status.error[:150]}")

        # Print timeouts
        if "timeout" in by_status:
            print(f"\n{Colors.WARNING}⏱️  TIMEOUT ({len(by_status['timeout'])}){Colors.ENDC}")
            for agent_id, status in by_status["timeout"]:
                agent_config = AGENTS[agent_id]
                emoji = agent_config.get("emoji", "⚙️")
                print(f"   {emoji} {status.name}")

        # Print running (shouldn't happen but just in case)
        if "running" in by_status:
            print(f"\n{Colors.OKCYAN}🏃 STILL RUNNING ({len(by_status['running'])}){Colors.ENDC}")
            for agent_id, status in by_status["running"]:
                agent_config = AGENTS[agent_id]
                emoji = agent_config.get("emoji", "⚙️")
                print(f"   {emoji} {status.name} ({status.duration():.1f}s)")

        # Summary stats
        total_time = time.time() - self.start_time
        print(f"\n{Colors.BOLD}Summary:{Colors.ENDC}")
        print(f"  Total time: {total_time:.1f}s")
        print(f"  Total agents: {len(self.agents)}")
        print(f"  Success rate: {len(by_status.get('completed', []))}/{len(self.agents)}")
        print(f"  Results saved to: {self.results_dir}")

        # Overall exit code
        if "failed" in by_status or "timeout" in by_status or "error" in by_status:
            print(f"\n{Colors.FAIL}Overall Status: SOME AGENTS FAILED{Colors.ENDC}")
            return 1
        else:
            print(f"\n{Colors.OKGREEN}Overall Status: ALL AGENTS COMPLETED{Colors.ENDC}")
            return 0

if __name__ == "__main__":
    try:
        orchestrator = AgentOrchestrator()
        orchestrator.run_all()
    except KeyboardInterrupt:
        print(f"\n{Colors.WARNING}Interrupted by user{Colors.ENDC}")
        sys.exit(1)
