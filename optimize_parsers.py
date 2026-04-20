#!/usr/bin/env python3
"""
⚡ PARSER OPTIMIZER & PROFILER AGENT
Профилирует скорость парсеров, ищет узкие места, предлагает оптимизации
"""

import time
import requests
import json
from typing import Dict, List, Tuple
from datetime import datetime
from pathlib import Path
import statistics

class ParserOptimizer:
    def __init__(self):
        self.api_base = "http://localhost:8080/api/v1"
        self.metrics_history = {}  # parser -> [metrics]
        self.log_file = Path("optimizer_results.log")
        self.results_file = Path("parser_performance.json")

    def run(self):
        """Main optimization loop"""
        print("⚡ Parser Optimizer & Profiler Agent Started")
        print("=" * 60)
        print(f"Start time: {datetime.now()}")
        print("=" * 60)
        
        iteration = 0
        while True:
            try:
                iteration += 1
                print(f"\n[Iteration {iteration}] {datetime.now()}")
                
                # Collect metrics
                metrics = self._collect_metrics()
                
                # Analyze and print
                self._analyze_metrics(metrics)
                
                # Save results
                self._save_results(metrics)
                
                # Wait before next cycle
                print("\n⏳ Waiting 120 seconds for next profiling cycle...")
                time.sleep(120)
                
            except Exception as e:
                print(f"❌ Error in optimizer: {e}")
                time.sleep(60)

    def _collect_metrics(self) -> Dict:
        """Collect current metrics from API"""
        print("\n📊 Collecting parser metrics...")
        
        metrics = {
            "timestamp": datetime.now().isoformat(),
            "parsers": {}
        }
        
        try:
            # Get parser coverage
            response = requests.get(
                f"{self.api_base}/parsers/coverage",
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                metrics["parsers"] = data.get("parsers", {})
                
                # Get scanner metrics for timing
                metrics_response = requests.get(
                    f"{self.api_base}/metrics",
                    timeout=10
                )
                
                if metrics_response.status_code == 200:
                    metrics["scanner"] = metrics_response.json()
                
                return metrics
                
        except Exception as e:
            print(f"   ❌ Error collecting metrics: {e}")
            return metrics

    def _analyze_metrics(self, metrics: Dict):
        """Analyze metrics and print insights"""
        print("\n📈 Parser Performance Analysis:")
        print("-" * 60)
        
        parsers = metrics.get("parsers", {})
        
        if not parsers:
            print("   No parser data available yet")
            return
        
        # Sort by events (most productive first)
        sorted_parsers = sorted(
            parsers.items(),
            key=lambda x: x[1].get("event_count", 0),
            reverse=True
        )
        
        total_events = 0
        
        for parser_name, data in sorted_parsers:
            event_count = data.get("event_count", 0)
            parse_time = data.get("parse_time_ms", 0)
            error_rate = data.get("error_rate", 0)
            status = data.get("status", "unknown")
            
            total_events += event_count
            
            # Calculate efficiency metrics
            efficiency = event_count / (parse_time / 1000) if parse_time > 0 else 0
            
            # Print with indicators
            emoji = self._get_status_emoji(status)
            speed = self._get_speed_indicator(parse_time)
            efficiency_bar = self._get_efficiency_bar(efficiency)
            
            print(f"\n  {emoji} {parser_name.upper()}")
            print(f"      Events: {event_count:,} | Time: {parse_time}ms {speed}")
            print(f"      Efficiency: {efficiency:.1f} events/sec {efficiency_bar}")
            print(f"      Error rate: {error_rate:.1f}% | Status: {status}")
            
            # Give recommendations
            self._give_recommendation(parser_name, data)
        
        print(f"\n  📊 Total events: {total_events:,}")
        
        # Scanner metrics
        scanner = metrics.get("scanner", {})
        if scanner:
            cycle_time = scanner.get("cycle_time_ms", 0)
            print(f"  🔄 Scan cycle: {cycle_time}ms (~{cycle_time/1000:.1f}s)")

    def _give_recommendation(self, parser_name: str, data: Dict):
        """Give optimization recommendations"""
        recommendations = []
        
        event_count = data.get("event_count", 0)
        parse_time = data.get("parse_time_ms", 0)
        error_rate = data.get("error_rate", 0)
        
        # Check for issues
        if event_count == 0:
            recommendations.append("⚠️  No events - check API connectivity")
        
        if parse_time > 10000:  # > 10 seconds
            recommendations.append("🐢 Slow parser - consider async/parallel fetching")
        
        if error_rate > 5:
            recommendations.append("❌ High error rate - check error handling")
        
        if parse_time > 5000 and event_count < 1000:
            recommendations.append("💡 Low throughput - optimize parsing logic")
        
        # Positive feedback
        if event_count > 5000 and parse_time < 5000 and error_rate < 1:
            recommendations.append("✅ Excellent performance")
        
        if recommendations:
            for rec in recommendations:
                print(f"       {rec}")

    def _save_results(self, metrics: Dict):
        """Save results to file"""
        try:
            with open(self.results_file, "a") as f:
                f.write(json.dumps(metrics) + "\n")
            print(f"   ✅ Results saved to {self.results_file}")
        except Exception as e:
            print(f"   ❌ Failed to save: {e}")

    def _get_status_emoji(self, status: str) -> str:
        status_map = {
            "healthy": "✅",
            "degraded": "🟡",
            "blocked": "🔴",
            "timeout": "⏱️",
            "error": "❌",
            "unknown": "❓"
        }
        return status_map.get(status.lower(), "❓")

    def _get_speed_indicator(self, parse_time: float) -> str:
        if parse_time < 2000:
            return "⚡ (very fast)"
        elif parse_time < 5000:
            return "🏃 (fast)"
        elif parse_time < 10000:
            return "🚶 (normal)"
        else:
            return "🐢 (slow)"

    def _get_efficiency_bar(self, efficiency: float) -> str:
        """Create ASCII efficiency bar"""
        if efficiency > 1000:
            return "████████████ (excellent)"
        elif efficiency > 500:
            return "████████ (good)"
        elif efficiency > 100:
            return "████ (ok)"
        else:
            return "█ (poor)"

if __name__ == "__main__":
    optimizer = ParserOptimizer()
    optimizer.run()
