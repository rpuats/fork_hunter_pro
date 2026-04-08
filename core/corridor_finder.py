# core/corridor_finder.py
"""
Corridor Scanner — detects corridor (middle) opportunities where two bets
on different totals or handicaps create a window where both can win,
or one wins with minimal loss on the other.
"""
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime
from collections import defaultdict
import logging
import hashlib

logger = logging.getLogger(__name__)


@dataclass
class CorridorScenario:
    """Represents one possible outcome scenario for a corridor."""
    name: str
    description: str
    probability: float
    profit_percent: float
    both_win: bool = False
    one_wins: bool = False
    both_lose: bool = False

    def to_dict(self) -> Dict:
        return {
            'name': self.name,
            'description': self.description,
            'probability': round(self.probability, 4),
            'profit_percent': round(self.profit_percent, 4),
            'both_win': self.both_win,
            'one_wins': self.one_wins,
            'both_lose': self.both_lose,
        }


@dataclass
class Corridor:
    """Represents a detected corridor opportunity."""
    id: str
    event_name: str
    sport: str
    corridor_type: str
    markets: List[Dict]
    odds: List[float]
    scenarios: List[CorridorScenario]
    ev_percent: float
    total_stake: float = 10000.0
    found_at: str = field(default_factory=lambda: datetime.utcnow().isoformat())

    def to_dict(self) -> Dict:
        return {
            'id': self.id,
            'event_name': self.event_name,
            'sport': self.sport,
            'corridor_type': self.corridor_type,
            'markets': self.markets,
            'odds': [round(o, 4) for o in self.odds],
            'scenarios': [s.to_dict() for s in self.scenarios],
            'ev_percent': round(self.ev_percent, 4),
            'total_stake': self.total_stake,
            'found_at': self.found_at,
        }


class CorridorFinder:
    """
    Detects corridor (middle) opportunities in totals and handicaps.

    Corridor types:
    - Totals: ТБ X + ТМ Y where X < Y (corridor on goals between X and Y)
    - Handicaps: Ф1(-X) + Ф2(+Y) where there's overlap
    """

    def __init__(self, min_ev: float = 1.0):
        """
        Args:
            min_ev: Minimum expected value percentage (default 1%).
        """
        self.min_ev = min_ev / 100
        self._stats: Dict[str, int] = defaultdict(int)

    def _make_id(self, event_name: str, corridor_type: str, markets: List[Dict]) -> str:
        raw = f"{event_name}|{corridor_type}|{str(markets)}"
        return hashlib.md5(raw.encode()).hexdigest()[:8]

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

    def _calc_stakes(self, odds: List[float], total_stake: float) -> List[float]:
        """Calculate proportional stakes for given odds."""
        inverses = [1.0 / o for o in odds]
        total_inv = sum(inverses)
        return [(total_stake * inv / total_inv) for inv in inverses]

    def _calc_scenarios_total(
        self,
        over_line: float,
        under_line: float,
        over_odds: float,
        under_odds: float,
        over_bk: str,
        under_bk: str,
        total_stake: float = 10000.0,
    ) -> Tuple[List[CorridorScenario], float]:
        """
        Calculate scenarios for a total corridor: ТБ(over_line) + ТМ(under_line).

        Since over_line < under_line, there's a corridor between them.
        E.g., ТБ 2.5 + ТМ 3.5 → corridor on exactly 3 goals.
        """
        stakes = self._calc_stakes([over_odds, under_odds], total_stake)

        over_return = stakes[0] * over_odds
        under_return = stakes[1] * under_odds
        total_staked = stakes[0] + stakes[1]

        over_prob = 1.0 / over_odds
        under_prob = 1.0 / under_odds

        corridor_gap = under_line - over_line

        scenarios = []

        both_win_prob = min(over_prob, under_prob) * corridor_gap / max(corridor_gap, 1.0)
        both_win_profit = (over_return + under_return - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="both_win",
            description=f"Both bets win (score falls in corridor {over_line}-{under_line})",
            probability=both_win_prob,
            profit_percent=both_win_profit,
            both_win=True,
        ))

        over_only_prob = over_prob - both_win_prob
        over_only_profit = (over_return - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="over_only",
            description=f"Only ТБ {over_line} wins (score >= {under_line})",
            probability=max(over_only_prob, 0),
            profit_percent=over_only_profit,
            one_wins=True,
        ))

        under_only_prob = under_prob - both_win_prob
        under_only_profit = (under_return - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="under_only",
            description=f"Only ТМ {under_line} wins (score <= {over_line})",
            probability=max(under_only_prob, 0),
            profit_percent=under_only_profit,
            one_wins=True,
        ))

        lose_prob = max(1.0 - over_prob - under_prob + both_win_prob, 0)
        lose_profit = -100.0

        scenarios.append(CorridorScenario(
            name="both_lose",
            description="Both bets lose (should not happen with proper corridor)",
            probability=lose_prob,
            profit_percent=lose_profit,
            both_lose=True,
        ))

        ev = sum(s.probability * s.profit_percent / 100 for s in scenarios) * 100

        return scenarios, ev

    def _calc_scenarios_handicap(
        self,
        handicap1: float,
        handicap2: float,
        odds1: float,
        odds2: float,
        bk1: str,
        bk2: str,
        selection1: str,
        selection2: str,
        total_stake: float = 10000.0,
    ) -> Tuple[List[CorridorScenario], float]:
        """
        Calculate scenarios for a handicap corridor.

        E.g., Ф1(-1.5) + Ф2(+2.5) → corridor if team 1 wins by exactly 2.
        """
        stakes = self._calc_stakes([odds1, odds2], total_stake)

        ret1 = stakes[0] * odds1
        ret2 = stakes[1] * odds2
        total_staked = stakes[0] + stakes[1]

        prob1 = 1.0 / odds1
        prob2 = 1.0 / odds2

        overlap = max(0, handicap2 - handicap1)

        scenarios = []

        both_win_prob = min(prob1, prob2) * min(overlap / 3.0, 1.0)
        both_win_profit = (ret1 + ret2 - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="both_win",
            description=f"Both bets win (handicap corridor: {selection1} + {selection2})",
            probability=both_win_prob,
            profit_percent=both_win_profit,
            both_win=True,
        ))

        one_only_prob = prob1 - both_win_prob
        one_only_profit = (ret1 - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="first_only",
            description=f"Only {selection1} wins",
            probability=max(one_only_prob, 0),
            profit_percent=one_only_profit,
            one_wins=True,
        ))

        two_only_prob = prob2 - both_win_prob
        two_only_profit = (ret2 - total_staked) / total_staked * 100

        scenarios.append(CorridorScenario(
            name="second_only",
            description=f"Only {selection2} wins",
            probability=max(two_only_prob, 0),
            profit_percent=two_only_profit,
            one_wins=True,
        ))

        lose_prob = max(1.0 - prob1 - prob2 + both_win_prob, 0)

        scenarios.append(CorridorScenario(
            name="both_lose",
            description="Both bets lose",
            probability=lose_prob,
            profit_percent=-100.0,
            both_lose=True,
        ))

        ev = sum(s.probability * s.profit_percent / 100 for s in scenarios) * 100

        return scenarios, ev

    def _find_total_corridors(self, events: List[Dict]) -> List[Corridor]:
        """Find total corridors: ТБ X + ТМ Y where X < Y."""
        corridors: List[Corridor] = []
        grouped = self._group_events(events)

        total_lines = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5]

        for event_key, bk_events in grouped.items():
            sport = bk_events[0].get('sport', 'unknown') if bk_events else 'unknown'
            event_name = f"{bk_events[0].get('home_team', '')} vs {bk_events[0].get('away_team', '')}" if bk_events else ''

            over_by_line: Dict[float, List[Dict]] = defaultdict(list)
            under_by_line: Dict[float, List[Dict]] = defaultdict(list)

            for event in bk_events:
                bk = event.get('bookmaker', 'unknown')
                for line in total_lines:
                    over_odds = event.get(f'total_over_{line}') or event.get(f'over_{line}')
                    under_odds = event.get(f'total_under_{line}') or event.get(f'under_{line}')

                    if over_odds and over_odds > 1.01:
                        over_by_line[line].append({
                            'odds': over_odds,
                            'bookmaker': bk,
                            'selection': f'ТБ {line}',
                            'line': line,
                        })
                    if under_odds and under_odds > 1.01:
                        under_by_line[line].append({
                            'odds': under_odds,
                            'bookmaker': bk,
                            'selection': f'ТМ {line}',
                            'line': line,
                        })

            over_lines_sorted = sorted(over_by_line.keys())
            under_lines_sorted = sorted(under_by_line.keys())

            for over_line in over_lines_sorted:
                for under_line in under_lines_sorted:
                    if over_line >= under_line:
                        continue

                    corridor_gap = under_line - over_line
                    if corridor_gap < 1.0:
                        continue

                    for over_entry in over_by_line[over_line]:
                        for under_entry in under_by_line[under_line]:
                            if over_entry['bookmaker'] == under_entry['bookmaker']:
                                continue

                            scenarios, ev = self._calc_scenarios_total(
                                over_line=over_line,
                                under_line=under_line,
                                over_odds=over_entry['odds'],
                                under_odds=under_entry['odds'],
                                over_bk=over_entry['bookmaker'],
                                under_bk=under_entry['bookmaker'],
                            )

                            if ev > self.min_ev * 100:
                                corridor_id = self._make_id(
                                    event_name, 'totals',
                                    [over_entry, under_entry]
                                )
                                corridor = Corridor(
                                    id=corridor_id,
                                    event_name=event_name,
                                    sport=sport,
                                    corridor_type='totals',
                                    markets=[
                                        {
                                            'bookmaker': over_entry['bookmaker'],
                                            'selection': over_entry['selection'],
                                            'line': over_line,
                                        },
                                        {
                                            'bookmaker': under_entry['bookmaker'],
                                            'selection': under_entry['selection'],
                                            'line': under_line,
                                        },
                                    ],
                                    odds=[over_entry['odds'], under_entry['odds']],
                                    scenarios=scenarios,
                                    ev_percent=ev,
                                )
                                corridors.append(corridor)
                                self._stats['totals'] += 1

        return corridors

    def _find_handicap_corridors(self, events: List[Dict]) -> List[Corridor]:
        """Find handicap corridors: Ф1(-X) + Ф2(+Y) with overlap."""
        corridors: List[Corridor] = []
        grouped = self._group_events(events)

        handicap_lines = [-3.5, -3.0, -2.5, -2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5]

        for event_key, bk_events in grouped.items():
            sport = bk_events[0].get('sport', 'unknown') if bk_events else 'unknown'
            event_name = f"{bk_events[0].get('home_team', '')} vs {bk_events[0].get('away_team', '')}" if bk_events else ''

            f1_entries: Dict[float, List[Dict]] = defaultdict(list)
            f2_entries: Dict[float, List[Dict]] = defaultdict(list)

            for event in bk_events:
                bk = event.get('bookmaker', 'unknown')
                for line in handicap_lines:
                    f1_key = f'f1_{line}'
                    f2_key = f'f2_{line}'
                    f1_odds = event.get(f1_key) or event.get(f'handicap1_{line}')
                    f2_odds = event.get(f2_key) or event.get(f'handicap2_{line}')

                    if f1_odds and f1_odds > 1.01:
                        f1_entries[line].append({
                            'odds': f1_odds,
                            'bookmaker': bk,
                            'selection': f'Ф1 ({line:+.1f})',
                            'line': line,
                        })
                    if f2_odds and f2_odds > 1.01:
                        f2_entries[line].append({
                            'odds': f2_odds,
                            'bookmaker': bk,
                            'selection': f'Ф2 ({line:+.1f})',
                            'line': line,
                        })

            for f1_line, f1_list in f1_entries.items():
                for f2_line, f2_list in f2_entries.items():
                    if f1_line >= f2_line:
                        continue

                    for f1_entry in f1_list:
                        for f2_entry in f2_list:
                            if f1_entry['bookmaker'] == f2_entry['bookmaker']:
                                continue

                            scenarios, ev = self._calc_scenarios_handicap(
                                handicap1=f1_line,
                                handicap2=f2_line,
                                odds1=f1_entry['odds'],
                                odds2=f2_entry['odds'],
                                bk1=f1_entry['bookmaker'],
                                bk2=f2_entry['bookmaker'],
                                selection1=f1_entry['selection'],
                                selection2=f2_entry['selection'],
                            )

                            if ev > self.min_ev * 100:
                                corridor_id = self._make_id(
                                    event_name, 'handicaps',
                                    [f1_entry, f2_entry]
                                )
                                corridor = Corridor(
                                    id=corridor_id,
                                    event_name=event_name,
                                    sport=sport,
                                    corridor_type='handicaps',
                                    markets=[
                                        {
                                            'bookmaker': f1_entry['bookmaker'],
                                            'selection': f1_entry['selection'],
                                            'line': f1_line,
                                        },
                                        {
                                            'bookmaker': f2_entry['bookmaker'],
                                            'selection': f2_entry['selection'],
                                            'line': f2_line,
                                        },
                                    ],
                                    odds=[f1_entry['odds'], f2_entry['odds']],
                                    scenarios=scenarios,
                                    ev_percent=ev,
                                )
                                corridors.append(corridor)
                                self._stats['handicaps'] += 1

        return corridors

    def find_corridors(
        self,
        events: List[Dict],
        min_ev: Optional[float] = None,
        sport: Optional[str] = None,
    ) -> List[Dict]:
        """
        Find all corridor opportunities.

        Args:
            events: List of event dicts from parsers.
            min_ev: Override minimum EV percentage.
            sport: Filter by sport.

        Returns:
            List of corridor dicts sorted by EV descending.
        """
        threshold = (min_ev / 100) if min_ev is not None else self.min_ev
        old_threshold = self.min_ev
        self.min_ev = threshold

        corridors: List[Corridor] = []
        corridors.extend(self._find_total_corridors(events))
        corridors.extend(self._find_handicap_corridors(events))

        if sport:
            corridors = [c for c in corridors if c.sport.lower() == sport.lower()]

        corridors.sort(key=lambda x: x.ev_percent, reverse=True)

        self.min_ev = old_threshold

        return [c.to_dict() for c in corridors]

    def get_stats(self) -> Dict:
        """Get finder statistics."""
        return {
            'total_found': sum(self._stats.values()),
            'by_type': dict(self._stats),
            'min_ev_threshold': round(self.min_ev * 100, 2),
        }

    def reset_stats(self):
        """Reset internal statistics."""
        self._stats.clear()
