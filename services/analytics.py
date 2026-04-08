# services/analytics.py
import asyncio
import time
from datetime import datetime, timedelta
from typing import List, Dict, Optional, Tuple
from collections import defaultdict
import json
import logging

logger = logging.getLogger(__name__)


class AnalyticsEngine:
    """Analytics engine for tracking surebet history and performance"""
    
    def __init__(self):
        self.surebet_history: List[Dict] = []
        self.profit_history: List[Dict] = []
        self.performance_stats: Dict = {
            'total_surebets_found': 0,
            'total_profit': 0.0,
            'avg_profit_percent': 0.0,
            'best_profit_percent': 0.0,
            'worst_profit_percent': 0.0,
            'bookmaker_stats': defaultdict(lambda: {'count': 0, 'total_profit': 0.0}),
            'sport_stats': defaultdict(lambda: {'count': 0, 'total_profit': 0.0}),
            'hourly_stats': defaultdict(int),
            'daily_stats': defaultdict(int),
        }
        self._lock = asyncio.Lock()
    
    async def record_surebet(self, surebet: Dict):
        """Record a found surebet"""
        async with self._lock:
            self.surebet_history.append({
                **surebet,
                'recorded_at': datetime.utcnow().isoformat()
            })
            
            profit = surebet.get('profit_percent', 0)
            self.performance_stats['total_surebets_found'] += 1
            self.performance_stats['total_profit'] += profit
            
            if profit > self.performance_stats['best_profit_percent']:
                self.performance_stats['best_profit_percent'] = profit
            
            if profit < self.performance_stats['worst_profit_percent'] or self.performance_stats['worst_profit_percent'] == 0:
                self.performance_stats['worst_profit_percent'] = profit
            
            total = self.performance_stats['total_surebets_found']
            self.performance_stats['avg_profit_percent'] = self.performance_stats['total_profit'] / total if total > 0 else 0
            
            for bk in surebet.get('bookmakers', []):
                self.performance_stats['bookmaker_stats'][bk]['count'] += 1
                self.performance_stats['bookmaker_stats'][bk]['total_profit'] += profit
            
            sport = surebet.get('sport', 'unknown')
            self.performance_stats['sport_stats'][sport]['count'] += 1
            self.performance_stats['sport_stats'][sport]['total_profit'] += profit
            
            now = datetime.utcnow()
            self.performance_stats['hourly_stats'][now.hour] += 1
            self.performance_stats['daily_stats'][now.strftime('%Y-%m-%d')] += 1
            
            if len(self.surebet_history) > 10000:
                self.surebet_history = self.surebet_history[-5000:]
    
    def get_summary(self) -> Dict:
        """Get analytics summary"""
        return {
            'total_surebets': self.performance_stats['total_surebets_found'],
            'total_profit_percent': round(self.performance_stats['total_profit'], 2),
            'avg_profit_percent': round(self.performance_stats['avg_profit_percent'], 2),
            'best_profit_percent': round(self.performance_stats['best_profit_percent'], 2),
            'worst_profit_percent': round(self.performance_stats['worst_profit_percent'], 2),
            'top_bookmakers': self._get_top_bookmakers(5),
            'top_sports': self._get_top_sports(5),
            'hourly_distribution': dict(self.performance_stats['hourly_stats']),
            'daily_distribution': dict(self.performance_stats['daily_stats']),
        }
    
    def _get_top_bookmakers(self, limit: int = 5) -> List[Dict]:
        """Get top bookmakers by surebet count"""
        bk_stats = self.performance_stats['bookmaker_stats']
        sorted_bks = sorted(bk_stats.items(), key=lambda x: x[1]['count'], reverse=True)
        
        return [
            {
                'bookmaker': name,
                'count': stats['count'],
                'total_profit': round(stats['total_profit'], 2),
                'avg_profit': round(stats['total_profit'] / stats['count'], 2) if stats['count'] > 0 else 0
            }
            for name, stats in sorted_bks[:limit]
        ]
    
    def _get_top_sports(self, limit: int = 5) -> List[Dict]:
        """Get top sports by surebet count"""
        sport_stats = self.performance_stats['sport_stats']
        sorted_sports = sorted(sport_stats.items(), key=lambda x: x[1]['count'], reverse=True)
        
        return [
            {
                'sport': name,
                'count': stats['count'],
                'total_profit': round(stats['total_profit'], 2),
                'avg_profit': round(stats['total_profit'] / stats['count'], 2) if stats['count'] > 0 else 0
            }
            for name, stats in sorted_sports[:limit]
        ]
    
    def get_history(self, limit: int = 50, hours: Optional[int] = None) -> List[Dict]:
        """Get surebet history"""
        history = self.surebet_history
        
        if hours:
            cutoff = datetime.utcnow() - timedelta(hours=hours)
            history = [
                sb for sb in history
                if datetime.fromisoformat(sb.get('recorded_at', '')) > cutoff
            ]
        
        return history[-limit:]
    
    def get_profit_chart_data(self, hours: int = 24) -> List[Dict]:
        """Get profit data for chart"""
        now = datetime.utcnow()
        chart_data = []
        
        for i in range(hours, 0, -1):
            hour_start = now - timedelta(hours=i)
            hour_end = now - timedelta(hours=i-1)
            
            hour_surebets = [
                sb for sb in self.surebet_history
                if hour_start <= datetime.fromisoformat(sb.get('recorded_at', '')) < hour_end
            ]
            
            chart_data.append({
                'time': hour_start.strftime('%H:00'),
                'count': len(hour_surebets),
                'total_profit': round(sum(sb.get('profit_percent', 0) for sb in hour_surebets), 2)
            })
        
        return chart_data
    
    def get_bookmaker_comparison(self) -> List[Dict]:
        """Get comparison of bookmakers"""
        bk_stats = self.performance_stats['bookmaker_stats']
        
        return [
            {
                'bookmaker': name,
                'surebets': stats['count'],
                'total_profit': round(stats['total_profit'], 2),
                'avg_profit': round(stats['total_profit'] / stats['count'], 2) if stats['count'] > 0 else 0,
                'efficiency': round(stats['count'] / max(self.performance_stats['total_surebets_found'], 1) * 100, 1)
            }
            for name, stats in bk_stats.items()
        ]
    
    def export_data(self) -> Dict:
        """Export all analytics data"""
        return {
            'summary': self.get_summary(),
            'history': self.get_history(limit=100),
            'chart_data': self.get_profit_chart_data(),
            'bookmaker_comparison': self.get_bookmaker_comparison(),
            'exported_at': datetime.utcnow().isoformat()
        }


analytics_engine = AnalyticsEngine()
