# scanner/core/normalizer.py
"""
Event Normalizer - matches events across different bookmakers.
Uses fuzzy matching on team names + league.
"""
import re
from typing import List, Dict, Tuple
from difflib import SequenceMatcher


class EventNormalizer:
    """Normalizes and matches events across bookmakers."""
    
    TEAM_SYNONYMS = {
        'реал мадрид': ['реал', 'реал м', 'real madrid'],
        'манчестер сити': ['ман сити', 'манчестер с', 'man city'],
        'манчестер юнайтед': ['ман юн', 'манчестер ю', 'man utd'],
        'пари нн': ['пари нн', 'нижний новгород'],
    }
    
    @classmethod
    def normalize_team(cls, name: str) -> str:
        if not name:
            return ''
        name = name.lower().strip()
        name = re.sub(r'\(.*?\)', '', name).strip()
        name = re.sub(r'\s+', ' ', name).strip()
        
        for canonical, aliases in cls.TEAM_SYNONYMS.items():
            if name == canonical or name in aliases:
                return canonical
            for alias in aliases:
                if alias in name or name in alias:
                    return canonical
        return name
    
    @classmethod
    def similarity(cls, a: str, b: str) -> float:
        na, nb = cls.normalize_team(a), cls.normalize_team(b)
        if na == nb:
            return 1.0
        if na in nb or nb in na:
            return 0.85
        return SequenceMatcher(None, na, nb).ratio()
    
    @classmethod
    def match_events(
        cls,
        events_a: List[Dict],
        events_b: List[Dict],
        min_confidence: float = 0.75
    ) -> List[Tuple[Dict, Dict, float]]:
        """Match events between two bookmakers."""
        matches = []
        
        for ea in events_a:
            home_a = ea.get('home_team', '')
            away_a = ea.get('away_team', '')
            league_a = ea.get('league', '').lower()
            
            best_match = None
            best_conf = 0.0
            
            for eb in events_b:
                home_b = eb.get('home_team', '')
                away_b = eb.get('away_team', '')
                league_b = eb.get('league', '').lower()
                
                # Direct match
                h_sim = cls.similarity(home_a, home_b)
                a_sim = cls.similarity(away_a, away_b)
                direct = (h_sim + a_sim) / 2
                
                # Reversed match
                h_sim_r = cls.similarity(home_a, away_b)
                a_sim_r = cls.similarity(away_a, home_b)
                reversed_ = (h_sim_r + a_sim_r) / 2
                
                team_score = max(direct, reversed_)
                league_score = 1.0 if league_a == league_b and league_a else 0.5
                
                confidence = team_score * 0.7 + league_score * 0.3
                
                if confidence > best_conf:
                    best_conf = confidence
                    best_match = eb
            
            if best_conf >= min_confidence and best_match:
                matches.append((ea, best_match, best_conf))
        
        return matches
