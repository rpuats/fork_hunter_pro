# core/value_detector.py
"""
Value Bet Detector — identifies mispriced odds by comparing bookmaker odds
against fair odds calculated from the market consensus across all bookmakers.
"""
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime
from collections import defaultdict
import logging

logger = logging.getLogger(__name__)


@dataclass
class ValueBet:
    """Represents a detected value bet."""
    id: str
    event_name: str
    sport: str
    bookmaker: str
    market: str
    selection: str
    bookmaker_odds: float
    fair_odds: float
    edge_percent: float
    implied_probability: float
    fair_probability: float
    found_at: str = field(default_factory=lambda: datetime.utcnow().isoformat())

    def to_dict(self) -> Dict:
        return {
            'id': self.id,
            'event_name': self.event_name,
            'sport': self.sport,
            'bookmaker': self.bookmaker,
            'market': self.market,
            'selection': self.selection,
            'bookmaker_odds': round(self.bookmaker_odds, 4),
            'fair_odds': round(self.fair_odds, 4),
            'edge_percent': round(self.edge_percent, 4),
            'implied_probability': round(self.implied_probability, 4),
            'fair_probability': round(self.fair_probability, 4),
            'found_at': self.found_at,
        }


class ValueBetDetector:
    """
    Detects value bets by comparing bookmaker odds against fair odds.

    Fair odds calculation:
        fair_odds = 1 / (sum(1/Ki) / N)
    where Ki are odds from all bookmakers for the same outcome,
    and N is the number of bookmakers offering that outcome.

    Edge calculation:
        edge = (bookmaker_odds / fair_odds) - 1
    """

    def __init__(self, min_edge: float = 2.0):
        """
        Args:
            min_edge: Minimum edge percentage to consider a value bet (default 2%).
        """
        self.min_edge = min_edge / 100
        self._stats: Dict[str, int] = defaultdict(int)
        self._total_scanned = 0

    def calculate_fair_odds(self, odds_list: List[float]) -> float:
        """
        Calculate fair odds by removing bookmaker margin.

        Formula: fair = 1 / (sum(1/Ki) / N)

        Args:
            odds_list: List of odds for the same outcome from different bookmakers.

        Returns:
            Fair odds without margin.
        """
        if not odds_list or len(odds_list) < 2:
            return 0.0

        valid_odds = [o for o in odds_list if o > 1.01]
        if len(valid_odds) < 2:
            return 0.0

        n = len(valid_odds)
        avg_implied_prob = sum(1.0 / o for o in valid_odds) / n
        if avg_implied_prob <= 0:
            return 0.0

        return 1.0 / avg_implied_prob

    def calculate_edge(self, bookmaker_odds: float, fair_odds: float) -> float:
        """
        Calculate the edge percentage.

        Args:
            bookmaker_odds: The odds offered by a specific bookmaker.
            fair_odds: The calculated fair odds.

        Returns:
            Edge as a decimal (e.g., 0.05 = 5% edge).
        """
        if fair_odds <= 0 or bookmaker_odds <= 0:
            return 0.0
        return (bookmaker_odds / fair_odds) - 1.0

    def _group_events(self, events: List[Dict]) -> Dict[str, List[Dict]]:
        """Group events by normalized event name."""
        grouped: Dict[str, List[Dict]] = defaultdict(list)
        for event in events:
            home = event.get('home_team', '').lower().strip()
            away = event.get('away_team', '').lower().strip()
            sport = event.get('sport', 'unknown')
            if home and away:
                key = f"{sport}|{home}|{away}"
                grouped[key].append(event)
        return grouped

    def _extract_outcomes_2way(self, events: List[Dict]) -> List[Dict]:
        """Extract 2-way market outcomes (home/away) from events."""
        outcomes = []
        for event in events:
            home_odds = event.get('home_odds')
            away_odds = event.get('away_odds')
            bookmaker = event.get('bookmaker', 'unknown')
            sport = event.get('sport', 'unknown')
            event_name = f"{event.get('home_team', '')} vs {event.get('away_team', '')}"

            if home_odds and home_odds > 1.01:
                outcomes.append({
                    'outcome_key': 'home',
                    'odds': home_odds,
                    'bookmaker': bookmaker,
                    'sport': sport,
                    'event_name': event_name,
                    'selection': 'П1',
                    'market': '2-way',
                })
            if away_odds and away_odds > 1.01:
                outcomes.append({
                    'outcome_key': 'away',
                    'odds': away_odds,
                    'bookmaker': bookmaker,
                    'sport': sport,
                    'event_name': event_name,
                    'selection': 'П2',
                    'market': '2-way',
                })
        return outcomes

    def _extract_outcomes_3way(self, events: List[Dict]) -> List[Dict]:
        """Extract 3-way market outcomes (home/draw/away) from events."""
        outcomes = []
        for event in events:
            home_odds = event.get('home_odds')
            draw_odds = event.get('draw_odds')
            away_odds = event.get('away_odds')
            bookmaker = event.get('bookmaker', 'unknown')
            sport = event.get('sport', 'unknown')
            event_name = f"{event.get('home_team', '')} vs {event.get('away_team', '')}"

            if home_odds and home_odds > 1.01:
                outcomes.append({
                    'outcome_key': 'home',
                    'odds': home_odds,
                    'bookmaker': bookmaker,
                    'sport': sport,
                    'event_name': event_name,
                    'selection': 'П1',
                    'market': '3-way',
                })
            if draw_odds and draw_odds > 1.01:
                outcomes.append({
                    'outcome_key': 'draw',
                    'odds': draw_odds,
                    'bookmaker': bookmaker,
                    'sport': sport,
                    'event_name': event_name,
                    'selection': 'Ничья',
                    'market': '3-way',
                })
            if away_odds and away_odds > 1.01:
                outcomes.append({
                    'outcome_key': 'away',
                    'odds': away_odds,
                    'bookmaker': bookmaker,
                    'sport': sport,
                    'event_name': event_name,
                    'selection': 'П2',
                    'market': '3-way',
                })
        return outcomes

    def _extract_total_outcomes(self, events: List[Dict]) -> List[Dict]:
        """Extract total (over/under) market outcomes from events."""
        outcomes = []
        for event in events:
            bookmaker = event.get('bookmaker', 'unknown')
            sport = event.get('sport', 'unknown')
            event_name = f"{event.get('home_team', '')} vs {event.get('away_team', '')}"

            for line in [1.5, 2.0, 2.5, 3.0, 3.5, 4.5]:
                over_key = f'total_over_{line}'
                under_key = f'total_under_{line}'
                over_odds = event.get(over_key) or event.get(f'over_{line}')
                under_odds = event.get(under_key) or event.get(f'under_{line}')

                if over_odds and over_odds > 1.01:
                    outcomes.append({
                        'outcome_key': f'over_{line}',
                        'odds': over_odds,
                        'bookmaker': bookmaker,
                        'sport': sport,
                        'event_name': event_name,
                        'selection': f'ТБ {line}',
                        'market': f'total_{line}',
                    })
                if under_odds and under_odds > 1.01:
                    outcomes.append({
                        'outcome_key': f'under_{line}',
                        'odds': under_odds,
                        'bookmaker': bookmaker,
                        'sport': sport,
                        'event_name': event_name,
                        'selection': f'ТМ {line}',
                        'market': f'total_{line}',
                    })
        return outcomes

    def find_value_bets(
        self,
        events: List[Dict],
        min_edge: Optional[float] = None,
        sport: Optional[str] = None,
        bookmaker: Optional[str] = None,
    ) -> List[Dict]:
        """
        Find all value bets across all markets.

        Args:
            events: List of event dicts from parsers.
            min_edge: Override minimum edge percentage.
            sport: Filter by sport.
            bookmaker: Filter by bookmaker slug.

        Returns:
            List of value bet dicts sorted by edge descending.
        """
        threshold = (min_edge / 100) if min_edge is not None else self.min_edge
        self._total_scanned += len(events)

        grouped = self._group_events(events)
        value_bets: List[ValueBet] = []
        bet_id_counter = 0

        for event_key, bk_events in grouped.items():
            sport_name = bk_events[0].get('sport', 'unknown') if bk_events else 'unknown'

            if sport and sport_name.lower() != sport.lower():
                continue

            for market_extractor in [
                self._extract_outcomes_2way,
                self._extract_outcomes_3way,
                self._extract_total_outcomes,
            ]:
                outcomes = market_extractor(bk_events)

                by_outcome: Dict[str, List[Dict]] = defaultdict(list)
                for outcome in outcomes:
                    by_outcome[outcome['outcome_key']].append(outcome)

                for outcome_key, outcome_list in by_outcome.items():
                    if len(outcome_list) < 2:
                        continue

                    odds_for_outcome = [o['odds'] for o in outcome_list]
                    fair_odds = self.calculate_fair_odds(odds_for_outcome)

                    if fair_odds <= 0:
                        continue

                    for outcome in outcome_list:
                        if bookmaker and outcome['bookmaker'] != bookmaker:
                            continue

                        edge = self.calculate_edge(outcome['odds'], fair_odds)

                        if edge > threshold:
                            bet_id_counter += 1
                            implied_prob = 1.0 / outcome['odds']
                            fair_prob = 1.0 / fair_odds

                            vb = ValueBet(
                                id=f"vb_{event_key}_{outcome_key}_{outcome['bookmaker']}_{bet_id_counter}",
                                event_name=outcome['event_name'],
                                sport=outcome['sport'],
                                bookmaker=outcome['bookmaker'],
                                market=outcome['market'],
                                selection=outcome['selection'],
                                bookmaker_odds=outcome['odds'],
                                fair_odds=fair_odds,
                                edge_percent=edge * 100,
                                implied_probability=implied_prob,
                                fair_probability=fair_prob,
                            )
                            value_bets.append(vb)
                            self._stats[outcome['bookmaker']] += 1

        value_bets.sort(key=lambda x: x.edge_percent, reverse=True)
        return [vb.to_dict() for vb in value_bets]

    def get_stats(self) -> Dict:
        """Get detector statistics."""
        return {
            'total_events_scanned': self._total_scanned,
            'value_bets_per_bookmaker': dict(self._stats),
            'min_edge_threshold': round(self.min_edge * 100, 2),
        }

    def reset_stats(self):
        """Reset internal statistics."""
        self._stats.clear()
        self._total_scanned = 0
