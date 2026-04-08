# core/finder.py
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass
import uuid
from datetime import datetime
import logging
import time
from collections import defaultdict

from core.team_normalizer import team_normalizer

logger = logging.getLogger(__name__)


@dataclass
class SurebetLeg:
    bookmaker: str
    market: str
    selection: str
    odds: float
    event_name: str
    calculated_stake: float = 0.0
    stake_percent: float = 0.0


@dataclass
class Surebet:
    id: str
    event_name: str
    sport: str
    market_type: str
    is_live: bool
    profit_percent: float
    total_stake: float
    estimated_profit: float
    legs: List[SurebetLeg]
    bookmakers: List[str]
    found_at: datetime
    expires_at: Optional[datetime] = None
    
    @classmethod
    def from_dict(cls, data: Dict) -> 'Surebet':
        legs = [SurebetLeg(**leg) for leg in data.get('legs', [])]
        return cls(
            id=data['id'],
            event_name=data['event_name'],
            sport=data.get('sport', 'football'),
            market_type=data.get('market_type', '1x2'),
            is_live=data.get('is_live', True),
            profit_percent=data['profit_percent'],
            total_stake=data['total_stake'],
            estimated_profit=data['estimated_profit'],
            legs=legs,
            bookmakers=[l.bookmaker for l in legs],
            found_at=datetime.fromisoformat(data.get('found_at', datetime.utcnow().isoformat()))
        )
    
    def to_dict(self) -> Dict:
        return {
            'id': self.id,
            'event_name': self.event_name,
            'sport': self.sport,
            'market_type': self.market_type,
            'is_live': self.is_live,
            'profit_percent': self.profit_percent,
            'total_stake': self.total_stake,
            'estimated_profit': self.estimated_profit,
            'legs': [leg.__dict__ for leg in self.legs],
            'bookmakers': self.bookmakers,
            'found_at': self.found_at.isoformat(),
            'expires_at': self.expires_at.isoformat() if self.expires_at else None
        }


class CalculationCache:
    """LRU cache for repeated margin calculations"""
    
    def __init__(self, maxsize: int = 10000):
        self._cache: Dict[str, float] = {}
        self._maxsize = maxsize
        self._hits = 0
        self._misses = 0
    
    def _make_key(self, odds_tuple: tuple) -> str:
        return f"{','.join(f'{o:.4f}' for o in odds_tuple)}"
    
    def get(self, odds: List[float]) -> Optional[float]:
        key = self._make_key(tuple(odds))
        if key in self._cache:
            self._hits += 1
            return self._cache[key]
        self._misses += 1
        return None
    
    def set(self, odds: List[float], result: float):
        if len(self._cache) >= self._maxsize:
            oldest_key = next(iter(self._cache))
            del self._cache[oldest_key]
        key = self._make_key(tuple(odds))
        self._cache[key] = result
    
    def stats(self) -> Dict:
        total = self._hits + self._misses
        return {
            'hits': self._hits,
            'misses': self._misses,
            'hit_rate': round(self._hits / total * 100, 2) if total > 0 else 0,
        }


class SurebetCalculator:
    """Advanced surebet calculator with multiple strategies"""
    
    def __init__(self, min_profit: float = 0.5):
        self.min_profit = min_profit / 100
        self._calc_cache = CalculationCache()
        self._group_cache: Dict[str, List[Dict]] = {}
        self._last_group_hash: str = ""
    
    def _calculate_margin_cached(self, odds: List[float]) -> float:
        cached = self._calc_cache.get(odds)
        if cached is not None:
            return cached
        margin = sum(1 / o for o in odds)
        self._calc_cache.set(odds, margin)
        return margin
    
    def _group_events_optimized(self, events: List[Dict]) -> Dict[tuple, List[Dict]]:
        """Single-pass event grouping with team name normalization"""
        grouped: Dict[tuple, List[Dict]] = defaultdict(list)
        for event in events:
            home = event.get('home_team', '')
            away = event.get('away_team', '')
            if not home or not away:
                continue
            key = team_normalizer.get_key(home, away)
            grouped[key].append(event)
        return grouped
    
    def calculate_stakes(self, odds: List[float], total_stake: float = 10000) -> List[float]:
        inverses = [1 / odd for odd in odds]
        total_inverse = sum(inverses)
        return [(total_stake * inv / total_inverse) for inv in inverses]
    
    def find_2way_surebets(self, events: List[Dict]) -> List[Dict]:
        surebets = []
        events_by_key = self._group_events_optimized(events)
        
        for key, same_events in events_by_key.items():
            if len(same_events) < 2:
                continue
            
            home_best = None
            home_best_odds = 0
            away_best = None
            away_best_odds = 0
            
            for e in same_events:
                h_odds = e.get('home_odds') or 0
                a_odds = e.get('away_odds') or 0
                
                if h_odds > home_best_odds and h_odds >= 1.05:
                    home_best_odds = h_odds
                    home_best = e
                
                if a_odds > away_best_odds and a_odds >= 1.05:
                    away_best_odds = a_odds
                    away_best = e
            
            if not home_best or not away_best or home_best is away_best:
                continue
            
            if home_best.get('bookmaker') == away_best.get('bookmaker'):
                continue
            
            # Early-exit: impossible to be surebet if sum of odds too low
            if home_best_odds + away_best_odds < 2.0:
                continue
            
            # Early-exit: filter unrealistic odds diff BEFORE expensive margin calc
            odds_diff = abs(home_best_odds - away_best_odds)
            if odds_diff > 10:
                continue
            
            margin = self._calculate_margin_cached([home_best_odds, away_best_odds])
            
            if margin >= 1:
                continue
            
            profit = (1 / margin - 1) * 100
            if profit < self.min_profit * 100:
                continue
            
            total_stake = 10000
            stakes = self.calculate_stakes([home_best_odds, away_best_odds], total_stake)
            
            surebets.append({
                'id': str(uuid.uuid4())[:8],
                'event_name': f"{home_best.get('home_team')} vs {home_best.get('away_team')}",
                'sport': home_best.get('sport', 'football'),
                'market_type': '2-way',
                'is_live': home_best.get('is_live', True),
                'profit_percent': profit,
                'total_stake': total_stake,
                'estimated_profit': total_stake * (1/margin - 1),
                'legs': [
                    SurebetLeg(
                        bookmaker=home_best['bookmaker'],
                        market='1',
                        selection='П1',
                        odds=home_best_odds,
                        event_name=f"{home_best.get('home_team')} - {home_best.get('away_team')}",
                        calculated_stake=stakes[0],
                        stake_percent=stakes[0]/total_stake*100
                    ).__dict__,
                    SurebetLeg(
                        bookmaker=away_best['bookmaker'],
                        market='2',
                        selection='П2',
                        odds=away_best_odds,
                        event_name=f"{away_best.get('home_team')} - {away_best.get('away_team')}",
                        calculated_stake=stakes[1],
                        stake_percent=stakes[1]/total_stake*100
                    ).__dict__
                ],
                'bookmakers': [home_best['bookmaker'], away_best['bookmaker']],
                'found_at': datetime.utcnow().isoformat()
            })
        
        return surebets
    
    def find_3way_surebets(self, events: List[Dict]) -> List[Dict]:
        surebets = []
        events_by_key = self._group_events_optimized(events)
        
        for key, same_events in events_by_key.items():
            best_home_odds = 0
            best_home = None
            best_draw_odds = 0
            best_draw = None
            best_away_odds = 0
            best_away = None
            
            for e in same_events:
                h = e.get('home_odds', 0) or 0
                d = e.get('draw_odds') or 0
                a = e.get('away_odds', 0) or 0
                
                if h and h > best_home_odds and h > 1.01:
                    best_home_odds = h
                    best_home = e
                
                if d and d > best_draw_odds and d > 1.01:
                    best_draw_odds = d
                    best_draw = e
                
                if a and a > best_away_odds and a > 1.01:
                    best_away_odds = a
                    best_away = e
            
            if not best_home or not best_draw or not best_away:
                continue
            
            # REQUIRE different bookmakers (at least 2)
            bookmakers = {best_home.get('bookmaker'), best_draw.get('bookmaker'), best_away.get('bookmaker')}
            if len(bookmakers) < 2:
                continue
            
            margin = self._calculate_margin_cached([best_home_odds, best_draw_odds, best_away_odds])
            
            if margin >= 1:
                continue
            
            profit = (1 / margin - 1) * 100
            if profit < self.min_profit * 100:
                continue
            
            total_stake = 10000
            stakes = self.calculate_stakes([best_home_odds, best_draw_odds, best_away_odds], total_stake)
            
            surebets.append({
                'id': str(uuid.uuid4())[:8],
                'event_name': f"{best_home.get('home_team')} vs {best_home.get('away_team')}",
                'sport': best_home.get('sport', 'football'),
                'market_type': '3-way',
                'is_live': best_home.get('is_live', True),
                'profit_percent': profit,
                'total_stake': total_stake,
                'estimated_profit': total_stake * (1/margin - 1),
                'legs': [
                    SurebetLeg(
                        bookmaker=best_home['bookmaker'],
                        market='1',
                        selection='П1',
                        odds=best_home_odds,
                        event_name=f"{best_home.get('home_team')} - {best_home.get('away_team')}",
                        calculated_stake=stakes[0],
                        stake_percent=stakes[0]/total_stake*100
                    ).__dict__,
                    SurebetLeg(
                        bookmaker=best_draw['bookmaker'],
                        market='X',
                        selection='Ничья',
                        odds=best_draw_odds,
                        event_name=f"{best_home.get('home_team')} - {best_home.get('away_team')}",
                        calculated_stake=stakes[1],
                        stake_percent=stakes[1]/total_stake*100
                    ).__dict__,
                    SurebetLeg(
                        bookmaker=best_away['bookmaker'],
                        market='2',
                        selection='П2',
                        odds=best_away_odds,
                        event_name=f"{best_away.get('home_team')} - {best_away.get('away_team')}",
                        calculated_stake=stakes[2],
                        stake_percent=stakes[2]/total_stake*100
                    ).__dict__
                ],
                'bookmakers': list(set([
                    best_home['bookmaker'],
                    best_draw['bookmaker'],
                    best_away['bookmaker']
                ])),
                'found_at': datetime.utcnow().isoformat()
            })
        
        return surebets
    
    def find_total_surebets(self, events: List[Dict], total_line: float = 2.5) -> List[Dict]:
        """Find surebets in total (over/under) markets"""
        surebets = []
        events_by_key = self._group_events_optimized(events)
        
        for key, same_events in events_by_key.items():
            for event in same_events:
                over_odds = event.get(f'total_over_{total_line}')
                under_odds = event.get(f'total_under_{total_line}')
                
                if not over_odds or not under_odds:
                    over_odds = event.get('over_odds')
                    under_odds = event.get('under_odds')
                
                if not over_odds or not under_odds:
                    continue
                
                if over_odds <= 1.01 or under_odds <= 1.01:
                    continue
                
                margin = self._calculate_margin_cached([over_odds, under_odds])
                
                if margin >= 1:
                    continue
                
                profit = (1 / margin - 1) * 100
                if profit < self.min_profit * 100:
                    continue
                
                total_stake = 10000
                stakes = self.calculate_stakes([over_odds, under_odds], total_stake)
                
                surebets.append({
                    'id': str(uuid.uuid4())[:8],
                    'event_name': f"{event.get('home_team')} vs {event.get('away_team')}",
                    'sport': event.get('sport', 'football'),
                    'market_type': f'total_{total_line}',
                    'is_live': event.get('is_live', True),
                    'profit_percent': profit,
                    'total_stake': total_stake,
                    'estimated_profit': total_stake * (1/margin - 1),
                    'legs': [
                        SurebetLeg(
                            bookmaker=event['bookmaker'],
                            market=f'Total {total_line}',
                            selection=f'ТБ {total_line}',
                            odds=over_odds,
                            event_name=f"{event.get('home_team')} - {event.get('away_team')}",
                            calculated_stake=stakes[0],
                            stake_percent=stakes[0]/total_stake*100
                        ).__dict__,
                        SurebetLeg(
                            bookmaker=event['bookmaker'],
                            market=f'Total {total_line}',
                            selection=f'ТМ {total_line}',
                            odds=under_odds,
                            event_name=f"{event.get('home_team')} - {event.get('away_team')}",
                            calculated_stake=stakes[1],
                            stake_percent=stakes[1]/total_stake*100
                        ).__dict__
                    ],
                    'bookmakers': [event['bookmaker']],
                    'found_at': datetime.utcnow().isoformat()
                })
        
        return surebets
    
    def find_all_total_surebets(self, events: List[Dict], total_lines: Optional[List[float]] = None) -> List[Dict]:
        """Find cross-bookmaker total (over/under) surebets across multiple lines.
        
        Events should have 'total_over' and 'total_under' keys as dicts:
          {'total_over': {2.5: 1.95, 3.0: 2.10}, 'total_under': {2.5: 2.05, 3.0: 1.85}}
        """
        if total_lines is None:
            total_lines = [1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5]
        
        surebets = []
        events_by_key = self._group_events_optimized(events)
        
        for key, same_events in events_by_key.items():
            if len(same_events) < 2:
                continue
            
            for line in total_lines:
                best_over_odds = 0.0
                best_over_event = None
                best_under_odds = 0.0
                best_under_event = None
                
                for event in same_events:
                    totals_over = event.get('total_over')
                    totals_under = event.get('total_under')
                    
                    if not isinstance(totals_over, dict) or not isinstance(totals_under, dict):
                        continue
                    
                    over_odds = totals_over.get(line) or totals_over.get(str(line))
                    under_odds = totals_under.get(line) or totals_under.get(str(line))
                    
                    if over_odds and over_odds > best_over_odds and over_odds >= 1.05:
                        best_over_odds = over_odds
                        best_over_event = event
                    
                    if under_odds and under_odds > best_under_odds and under_odds >= 1.05:
                        best_under_odds = under_odds
                        best_under_event = event
                
                if not best_over_event or not best_under_event:
                    continue
                
                if best_over_event.get('bookmaker') == best_under_event.get('bookmaker'):
                    continue
                
                margin = self._calculate_margin_cached([best_over_odds, best_under_odds])
                
                if margin >= 1:
                    continue
                
                profit = (1 / margin - 1) * 100
                if profit < self.min_profit * 100:
                    continue
                
                total_stake = 10000
                stakes = self.calculate_stakes([best_over_odds, best_under_odds], total_stake)
                
                event_name = f"{best_over_event.get('home_team')} vs {best_over_event.get('away_team')}"
                
                surebets.append({
                    'id': str(uuid.uuid4())[:8],
                    'event_name': event_name,
                    'sport': best_over_event.get('sport', 'football'),
                    'market_type': f'total_{line}',
                    'is_live': best_over_event.get('is_live', True),
                    'profit_percent': profit,
                    'total_stake': total_stake,
                    'estimated_profit': total_stake * (1/margin - 1),
                    'legs': [
                        SurebetLeg(
                            bookmaker=best_over_event['bookmaker'],
                            market=f'Total {line}',
                            selection=f'ТБ {line}',
                            odds=best_over_odds,
                            event_name=event_name,
                            calculated_stake=stakes[0],
                            stake_percent=stakes[0]/total_stake*100
                        ).__dict__,
                        SurebetLeg(
                            bookmaker=best_under_event['bookmaker'],
                            market=f'Total {line}',
                            selection=f'ТМ {line}',
                            odds=best_under_odds,
                            event_name=event_name,
                            calculated_stake=stakes[1],
                            stake_percent=stakes[1]/total_stake*100
                        ).__dict__
                    ],
                    'bookmakers': list(set([
                        best_over_event['bookmaker'],
                        best_under_event['bookmaker']
                    ])),
                    'found_at': datetime.utcnow().isoformat()
                })
        
        return surebets
    
    def find_handicap_surebets(self, events: List[Dict], handicap_lines: Optional[List[float]] = None) -> List[Dict]:
        """Find cross-bookmaker handicap surebets across multiple lines.
        
        Events should have 'handicap_home' and 'handicap_away' keys as dicts:
          {'handicap_home': {-0.5: 1.95, -1.0: 2.10}, 'handicap_away': {+0.5: 2.05, +1.0: 1.85}}
        """
        if handicap_lines is None:
            handicap_lines = [-2.5, -2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5]
        
        surebets = []
        events_by_key = self._group_events_optimized(events)
        
        for key, same_events in events_by_key.items():
            if len(same_events) < 2:
                continue
            
            for line in handicap_lines:
                best_home_odds = 0.0
                best_home_event = None
                best_away_odds = 0.0
                best_away_event = None
                
                for event in same_events:
                    hc_home = event.get('handicap_home')
                    hc_away = event.get('handicap_away')
                    
                    if not isinstance(hc_home, dict) or not isinstance(hc_away, dict):
                        continue
                    
                    home_odds = hc_home.get(line) or hc_home.get(str(line))
                    away_line = -line
                    away_odds = hc_away.get(away_line) or hc_away.get(str(away_line))
                    
                    if home_odds and home_odds > best_home_odds and home_odds >= 1.05:
                        best_home_odds = home_odds
                        best_home_event = event
                    
                    if away_odds and away_odds > best_away_odds and away_odds >= 1.05:
                        best_away_odds = away_odds
                        best_away_event = event
                
                if not best_home_event or not best_away_event:
                    continue
                
                if best_home_event.get('bookmaker') == best_away_event.get('bookmaker'):
                    continue
                
                margin = self._calculate_margin_cached([best_home_odds, best_away_odds])
                
                if margin >= 1:
                    continue
                
                profit = (1 / margin - 1) * 100
                if profit < self.min_profit * 100:
                    continue
                
                total_stake = 10000
                stakes = self.calculate_stakes([best_home_odds, best_away_odds], total_stake)
                
                line_str = f"{'+' if line > 0 else ''}{line}"
                event_name = f"{best_home_event.get('home_team')} vs {best_home_event.get('away_team')}"
                
                surebets.append({
                    'id': str(uuid.uuid4())[:8],
                    'event_name': event_name,
                    'sport': best_home_event.get('sport', 'football'),
                    'market_type': f'handicap_{line_str}',
                    'is_live': best_home_event.get('is_live', True),
                    'profit_percent': profit,
                    'total_stake': total_stake,
                    'estimated_profit': total_stake * (1/margin - 1),
                    'legs': [
                        SurebetLeg(
                            bookmaker=best_home_event['bookmaker'],
                            market=f'Handicap {line_str}',
                            selection=f'Ф1 ({line_str})',
                            odds=best_home_odds,
                            event_name=event_name,
                            calculated_stake=stakes[0],
                            stake_percent=stakes[0]/total_stake*100
                        ).__dict__,
                        SurebetLeg(
                            bookmaker=best_away_event['bookmaker'],
                            market=f'Handicap {line_str}',
                            selection=f'Ф2 ({line_str})',
                            odds=best_away_odds,
                            event_name=event_name,
                            calculated_stake=stakes[1],
                            stake_percent=stakes[1]/total_stake*100
                        ).__dict__
                    ],
                    'bookmakers': list(set([
                        best_home_event['bookmaker'],
                        best_away_event['bookmaker']
                    ])),
                    'found_at': datetime.utcnow().isoformat()
                })
        
        return surebets
    
    def find_surebets(self, events: List[Dict]) -> List[Dict]:
        all_surebets = []
        
        all_surebets.extend(self.find_2way_surebets(events))
        all_surebets.extend(self.find_3way_surebets(events))
        
        return sorted(all_surebets, key=lambda x: x.get('profit_percent', 0), reverse=True)
    
    def get_cache_stats(self) -> Dict:
        return self._calc_cache.stats()


class OddsAnalyzer:
    """Analyze odds patterns and detect anomalies"""
    
    @staticmethod
    def calculate_margin(odds: List[float]) -> float:
        inverses = [1/o for o in odds if o > 1.01]
        return sum(inverses) - 1 if inverses else 0
    
    @staticmethod
    def is_arbitrage(odds: List[float]) -> bool:
        return OddsAnalyzer.calculate_margin(odds) < 0
    
    @staticmethod
    def get_best_odds(events: List[Dict], market: str) -> Tuple[float, str]:
        best_odds = 0
        best_bk = ''
        
        for event in events:
            odd = event.get(market, 0)
            if odd > best_odds:
                best_odds = odd
                best_bk = event.get('bookmaker', '')
        
        return best_odds, best_bk
    
    @staticmethod
    def detect_odds_movement(current: float, historical: List[float]) -> str:
        if not historical:
            return 'stable'
        
        avg = sum(historical) / len(historical)
        diff_percent = ((current - avg) / avg) * 100
        
        if diff_percent > 5:
            return 'sharp_up'
        elif diff_percent < -5:
            return 'sharp_down'
        return 'stable'
