# core/generosity_index.py
"""
Bookmaker Generosity Index (Идея #12)

Compares each bookmaker's odds vs market average to show which BK
consistently offers higher odds. Helps focus surebet scanning on
"generous" bookmakers.
"""
import logging
from collections import defaultdict
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class BookmakerGenerosityIndex:
    """
    Tracks how generous each bookmaker is compared to market average.
    
    Generosity = (bk_odds / market_avg_odds) - 1
    
    Positive value = BK offers better odds than market average
    Negative value = BK offers worse odds than market average
    """
    
    _historical_data: Dict[str, List[float]] = field(default_factory=lambda: defaultdict(list))
    _sport_data: Dict[str, Dict[str, List[float]]] = field(default_factory=lambda: defaultdict(lambda: defaultdict(list)))
    _max_history: int = 1000
    
    def calculate_index(self, events: List[Dict]) -> Dict[str, Dict[str, float]]:
        """
        Calculate generosity index for each bookmaker by sport.
        
        Args:
            events: List of event dicts with keys: bookmaker, sport, home_odds, away_odds, etc.
        
        Returns:
            Dict[bookmaker_slug, Dict[sport, avg_generosity]]
        """
        odds_by_match: Dict[str, Dict[str, Dict[str, float]]] = defaultdict(lambda: defaultdict(dict))
        
        for event in events:
            bookmaker = event.get('bookmaker', '')
            sport = event.get('sport', 'unknown')
            home_odds = event.get('home_odds') or 0
            away_odds = event.get('away_odds') or 0
            draw_odds = event.get('draw_odds') or 0
            
            if not bookmaker or home_odds <= 1 or away_odds <= 1:
                continue
            
            match_key = self._get_match_key(event)
            if not match_key:
                continue
            
            odds_by_match[match_key][bookmaker]['home'] = home_odds
            odds_by_match[match_key][bookmaker]['away'] = away_odds
            if draw_odds and draw_odds > 1:
                odds_by_match[match_key][bookmaker]['draw'] = draw_odds
        
        generosity_by_bk_sport: Dict[str, Dict[str, List[float]]] = defaultdict(lambda: defaultdict(list))
        
        for match_key, bk_odds in odds_by_match.items():
            if len(bk_odds) < 2:
                continue
            
            all_home = [data.get('home', 0) for data in bk_odds.values() if data.get('home', 0) > 1]
            all_away = [data.get('away', 0) for data in bk_odds.values() if data.get('away', 0) > 1]
            all_draw = [data.get('draw', 0) for data in bk_odds.values() if data.get('draw', 0) > 1]
            
            avg_home = sum(all_home) / len(all_home) if all_home else 0
            avg_away = sum(all_away) / len(all_away) if all_away else 0
            avg_draw = sum(all_draw) / len(all_draw) if all_draw else 0
            
            sport = self._get_sport_for_match(match_key, events)
            
            for bookmaker, data in bk_odds.items():
                diffs = []
                if 'home' in data and avg_home > 0:
                    diffs.append((data['home'] / avg_home) - 1)
                if 'away' in data and avg_away > 0:
                    diffs.append((data['away'] / avg_away) - 1)
                if 'draw' in data and avg_draw > 0:
                    diffs.append((data['draw'] / avg_draw) - 1)
                
                if diffs:
                    avg_diff = sum(diffs) / len(diffs)
                    generosity_by_bk_sport[bookmaker][sport].append(avg_diff)
        
        result: Dict[str, Dict[str, float]] = {}
        for bookmaker, sports in generosity_by_bk_sport.items():
            result[bookmaker] = {}
            for sport, values in sports.items():
                if values:
                    result[bookmaker][sport] = round(sum(values) / len(values), 4)
        
        self._update_historical_data(result)
        
        return result
    
    def get_ranking(self, sport: Optional[str] = None) -> List[Tuple[str, float]]:
        """
        Get bookmakers ranked by generosity (highest first).
        
        Args:
            sport: Optional sport filter. If None, returns overall ranking.
        
        Returns:
            List of (bookmaker_slug, avg_generosity) tuples sorted by generosity desc.
        """
        if not self._historical_data:
            return []
        
        if sport:
            rankings = []
            for bk, sports in self._sport_data.items():
                if sport in sports:
                    values = sports[sport]
                    if values:
                        avg = sum(values) / len(values)
                        rankings.append((bk, round(avg, 4)))
        else:
            rankings = []
            for bk, values in self._historical_data.items():
                if values:
                    avg = sum(values) / len(values)
                    rankings.append((bk, round(avg, 4)))
        
        rankings.sort(key=lambda x: x[1], reverse=True)
        return rankings
    
    def get_best_for_sport(self, sport: str) -> Optional[str]:
        """
        Get the most generous bookmaker for a specific sport.
        
        Args:
            sport: Sport name (e.g., 'football', 'tennis', 'basketball')
        
        Returns:
            Bookmaker slug with highest generosity for this sport, or None if no data.
        """
        ranking = self.get_ranking(sport=sport)
        if ranking:
            return ranking[0][0]
        return None
    
    def get_summary(self) -> Dict:
        """
        Get a comprehensive summary of generosity data.
        
        Returns:
            Dict with ranking, sport_best, and overall stats.
        """
        ranking = self.get_ranking()
        
        sports = set()
        for sport_data in self._sport_data.values():
            sports.update(sport_data.keys())
        
        sport_best = {}
        for sport in sports:
            best = self.get_best_for_sport(sport)
            if best:
                sport_best[sport] = best
        
        return {
            'ranking': ranking,
            'sport_best': sport_best,
            'total_bookmakers': len(self._historical_data),
            'total_samples': sum(len(v) for v in self._historical_data.values()),
            'sports_tracked': list(sports),
        }
    
    def _update_historical_data(self, current_index: Dict[str, Dict[str, float]]):
        """Update internal historical data with new calculations."""
        for bookmaker, sports in current_index.items():
            for sport, value in sports.items():
                self._sport_data[bookmaker][sport].append(value)
                if len(self._sport_data[bookmaker][sport]) > self._max_history:
                    self._sport_data[bookmaker][sport] = self._sport_data[bookmaker][sport][-self._max_history:]
                
                self._historical_data[bookmaker].append(value)
                if len(self._historical_data[bookmaker]) > self._max_history:
                    self._historical_data[bookmaker] = self._historical_data[bookmaker][-self._max_history:]
    
    def _get_match_key(self, event: Dict) -> str:
        """Create a normalized match key for grouping events."""
        home = event.get('home_team', '').lower().strip()
        away = event.get('away_team', '').lower().strip()
        if home and away:
            teams = sorted([home, away])
            return f"{teams[0]}|{teams[1]}"
        return ""
    
    def _get_sport_for_match(self, match_key: str, events: List[Dict]) -> str:
        """Get sport for a match key from events."""
        for event in events:
            if self._get_match_key(event) == match_key:
                return event.get('sport', 'unknown')
        return 'unknown'
    
    def reset(self):
        """Clear all historical data."""
        self._historical_data.clear()
        self._sport_data.clear()
        logger.info("Generosity index reset")
