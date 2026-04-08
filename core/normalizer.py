# core/normalizer.py
import re
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
from collections import defaultdict


@dataclass
class NormalizedTeam:
    name: str
    aliases: List[str]
    country: Optional[str] = None
    city: Optional[str] = None


class TeamNormalizer:
    """Smart team name normalizer with fuzzy matching"""
    
    def __init__(self):
        self._teams: Dict[str, NormalizedTeam] = {}
        self._aliases: Dict[str, str] = {}
        self._common_words = self._load_common_words()
        self._init_known_teams()
    
    def _load_common_words(self) -> set:
        return {
            'fc', 'fk', 'sc', 'cf', 'ac', 'rc', 'ud', 'sd', 'as', 'us',
            'sk', 'ss', 'ks', 'gs', 'rg', 'of', 'if', 'kf', 'af', 'gf',
            'team', 'club', ' Atlético', 'athletic', 'racing', 'sporting',
            'real', 'sport', 'city', 'united', 'manager', 'academy',
            'young', 'reserves', 'ii', 'iii', 'u21', 'u23', 'u19',
            'women', 'lg', 'rt', 'rt'
        }
    
    def _init_known_teams(self):
        known_teams = {
            'Манчестер Юнайтед': ['manchester united', 'man utd', 'mu', 'manchester utd'],
            'Манчестер Сити': ['manchester city', 'man city', 'mc', 'mancity'],
            'Ливерпуль': ['liverpool', 'lfc', 'liverpool fc'],
            'Челси': ['chelsea', 'cfc', 'chelsea fc'],
            'Арсенал': ['arsenal', 'afc', 'arsenal fc'],
            'Тоттенхэм': ['tottenham', 'spurs', 'tottenham hotspur', 'thfc'],
            'Барселона': ['barcelona', 'fcb', 'barca', 'fc barcelona'],
            'Реал Мадрид': ['real madrid', 'rm', 'real', 'real madrid cf'],
            'Атлетико Мадрид': ['atletico madrid', 'atm', 'atletico'],
            'Бавария': ['bayern', 'bayern munchen', 'fc bayern', 'bayern munich'],
            'Боруссия Дортмунд': ['borussia dortmund', 'bvb', 'dortmund', 'bvb09'],
            'ПСЖ': ['psg', 'paris saint germain', 'paris sg', 'paris'],
            'Ювентус': ['juventus', 'juve', 'juventus fc', ' Old Lady'],
            'Интер': ['inter', 'inter milan', 'fc inter', 'inter milano'],
            'Милан': ['ac milan', 'milan', 'acmilan', 'milan ac'],
            'Рома': ['roma', 'as roma', 'asr', 'roma fc'],
            'Наполи': ['napoli', 'sc napoli', 'ssc napoli'],
            'Спартак Москва': ['spartak moscow', 'spartak', 'spartak moskva'],
            'ЦСКА Москва': ['cska moscow', 'cska', 'cska moskva', 'pfk cska'],
            'Зенит': ['zenit', 'zenit spb', 'zenit saint petersburg'],
            'Динамо Москва': ['dinamo moscow', 'dinamo', 'dinamo moskva'],
            'Локомотив Москва': ['lokomotiv moscow', 'lokomotiv', 'loko'],
            'Ростов': ['rostov', 'fk rostov', 'rostov on don'],
            'Краснодар': ['krasnodar', 'fk krasnodar', 'fc krasnodar'],
            'Урал': ['ural', 'ural ekaterinburg', 'fk ural'],
        }
        
        for canonical, aliases in known_teams.items():
            self._teams[canonical.lower()] = NormalizedTeam(
                name=canonical,
                aliases=aliases
            )
            for alias in aliases:
                self._aliases[alias] = canonical.lower()
    
    def normalize(self, team_name: str) -> str:
        if not team_name:
            return team_name
        
        original = team_name.strip()
        normalized = self._clean_name(original)
        
        if normalized in self._aliases:
            return self._teams[self._aliases[normalized]].name
        
        for canonical, team in self._teams.items():
            for alias in team.aliases:
                if self._fuzzy_match(normalized, alias):
                    return team.name
        
        return original
    
    def _clean_name(self, name: str) -> str:
        name = name.lower().strip()
        
        name = re.sub(r'[^\w\s\u0400-\u04FF]', ' ', name)
        
        words = name.split()
        words = [w for w in words if w not in self._common_words]
        
        return ' '.join(words).strip()
    
    def _fuzzy_match(self, s1: str, s2: str, threshold: float = 0.8) -> bool:
        s1 = s1.lower().strip()
        s2 = s2.lower().strip()
        
        if s1 == s2:
            return True
        
        if s1 in s2 or s2 in s1:
            return True
        
        if self._levenshtein_ratio(s1, s2) >= threshold:
            return True
        
        return False
    
    def _levenshtein_ratio(self, s1: str, s2: str) -> float:
        if len(s1) == 0 and len(s2) == 0:
            return 1.0
        
        if len(s1) == 0 or len(s2) == 0:
            return 0.0
        
        m, n = len(s1), len(s2)
        
        if m > n:
            s1, s2 = s2, s1
            m, n = n, m
        
        dp = list(range(m + 1))
        
        for j in range(1, n + 1):
            prev = dp[0]
            dp[0] = j
            for i in range(1, m + 1):
                temp = dp[i]
                cost = 0 if s1[i-1] == s2[j-1] else 1
                dp[i] = min(
                    dp[i] + 1,
                    dp[i-1] + 1,
                    prev + cost
                )
                prev = temp
        
        distance = dp[m]
        max_len = max(len(s1), len(s2))
        return 1 - (distance / max_len)
    
    def are_same(self, team1: str, team2: str) -> bool:
        n1 = self.normalize(team1)
        n2 = self.normalize(team2)
        return n1 == n2


class EventNormalizer:
    """Normalizes events across different bookmakers"""
    
    def __init__(self):
        self.team_normalizer = TeamNormalizer()
        self._cache: Dict[str, str] = {}
    
    def normalize_event(self, home_team: str, away_team: str) -> Tuple[str, str]:
        home = self.team_normalizer.normalize(home_team)
        away = self.team_normalizer.normalize(away_team)
        
        if home > away:
            return away, home
        
        return home, away
    
    def get_event_key(self, home_team: str, away_team: str, market: str = '1x2') -> str:
        home, away = self.normalize_event(home_team, away_team)
        return f"{home}|{away}|{market}"
    
    def are_same_event(
        self,
        home1: str, away1: str,
        home2: str, away2: str
    ) -> bool:
        n1_home, n1_away = self.normalize_event(home1, away1)
        n2_home, n2_away = self.normalize_event(home2, away2)
        
        return n1_home == n2_home and n1_away == n2_away


class SportsNormalizer:
    """Normalizes sport names and types"""
    
    SPORT_ALIASES = {
        'football': ['football', 'soccer', 'футбол', 'фубол', 'footbal'],
        'hockey': ['hockey', 'хоккей', 'hokey'],
        'basketball': ['basketball', 'баскетбол', 'bascetball', 'basketbal'],
        'tennis': ['tennis', 'теннис', 'tenis'],
        'volleyball': ['volleyball', 'волейбол', 'valleyball'],
        'esports': ['esports', 'киберспорт', 'csgo', 'dota2', 'lol'],
    }
    
    @classmethod
    def normalize_sport(cls, sport: str) -> str:
        if not sport:
            return 'other'
        
        sport_lower = sport.lower().strip()
        
        for canonical, aliases in cls.SPORT_ALIASES.items():
            if sport_lower in aliases or sport_lower == canonical:
                return canonical
        
        return 'other'


event_normalizer = EventNormalizer()
