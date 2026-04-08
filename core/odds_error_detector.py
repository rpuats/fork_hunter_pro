# core/odds_error_detector.py
"""
Odds Error Detector (Идея #23)
Detects anomalous odds that deviate significantly from market average.
Bookmaker mistakes can yield 10-50% profit vs 0.5-3% for normal surebets.

Different from surebets: single-bookmaker anomaly detection.
"""
import math
import logging
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime
from collections import defaultdict
import statistics

logger = logging.getLogger(__name__)


@dataclass
class Anomaly:
    """Represents a detected odds anomaly."""
    event_name: str
    sport: str
    bookmaker: str
    selection: str
    anomalous_odds: float
    market_average: float
    deviation_percent: float
    num_bookmakers: int
    is_live: bool
    detected_at: str = ""
    score: float = 0.0

    def to_dict(self) -> Dict:
        return {
            "event_name": self.event_name,
            "sport": self.sport,
            "bookmaker": self.bookmaker,
            "selection": self.selection,
            "anomalous_odds": round(self.anomalous_odds, 4),
            "market_average": round(self.market_average, 4),
            "deviation_percent": round(self.deviation_percent, 2),
            "num_bookmakers": self.num_bookmakers,
            "is_live": self.is_live,
            "detected_at": self.detected_at,
            "score": round(self.score, 2),
        }


class OddsErrorDetector:
    """
    Detects bookmaker errors by finding odds that significantly deviate
    from the market consensus across multiple bookmakers.

    Strategy:
    1. Group events by match (home_team vs away_team)
    2. For each selection (home/draw/away), compute market average
    3. Flag outliers that deviate > threshold from market average
    4. Score anomalies by likelihood of being a real error
    """

    def __init__(
        self,
        default_threshold: float = 0.25,
        min_bookmakers: int = 3,
        max_single_share: float = 0.7,
    ):
        self.default_threshold = default_threshold
        self.min_bookmakers = min_bookmakers
        self.max_single_share = max_single_share
        self._detection_count = 0
        self._false_positive_filter = True

    def compute_market_average(
        self, events: List[Dict], match_key: str
    ) -> Dict[str, float]:
        """
        Compute market average odds for each selection of a given match.

        Args:
            events: List of event dicts with bookmaker odds
            match_key: Normalized match key (e.g. "team_a|team_b")

        Returns:
            Dict mapping selection -> average odds
            e.g. {"home": 2.15, "draw": 3.40, "away": 3.10}
        """
        match_events = [e for e in events if self._get_match_key(e) == match_key]

        if not match_events:
            return {}

        home_odds = []
        draw_odds = []
        away_odds = []

        for e in match_events:
            h = e.get("home_odds") or e.get("p1") or e.get("1")
            d = e.get("draw_odds") or e.get("x") or e.get("draw")
            a = e.get("away_odds") or e.get("p2") or e.get("2")

            if h and h > 1.0:
                home_odds.append(float(h))
            if d and d > 1.0:
                draw_odds.append(float(d))
            if a and a > 1.0:
                away_odds.append(float(a))

        result = {}
        if home_odds:
            result["home"] = statistics.mean(home_odds)
        if draw_odds:
            result["draw"] = statistics.mean(draw_odds)
        if away_odds:
            result["away"] = statistics.mean(away_odds)

        return result

    def compute_market_stats(
        self, events: List[Dict], match_key: str
    ) -> Dict[str, Dict]:
        """
        Compute detailed market statistics for each selection.

        Returns:
            Dict mapping selection -> {mean, median, stdev, min, max, values, count}
        """
        match_events = [e for e in events if self._get_match_key(e) == match_key]

        if not match_events:
            return {}

        selections = {"home": [], "draw": [], "away": []}

        for e in match_events:
            h = e.get("home_odds") or e.get("p1") or e.get("1")
            d = e.get("draw_odds") or e.get("x") or e.get("draw")
            a = e.get("away_odds") or e.get("p2") or e.get("2")

            if h and h > 1.0:
                selections["home"].append(float(h))
            if d and d > 1.0:
                selections["draw"].append(float(d))
            if a and a > 1.0:
                selections["away"].append(float(a))

        result = {}
        for sel, values in selections.items():
            if len(values) < 2:
                continue
            result[sel] = {
                "mean": statistics.mean(values),
                "median": statistics.median(values),
                "stdev": statistics.stdev(values) if len(values) >= 2 else 0.0,
                "min": min(values),
                "max": max(values),
                "values": sorted(values),
                "count": len(values),
            }

        return result

    def detect_anomalies(
        self,
        events: List[Dict],
        threshold: Optional[float] = None,
    ) -> List[Anomaly]:
        """
        Detect all odds anomalies across all matches in events.

        Args:
            events: List of event dicts
            threshold: Deviation threshold (default: self.default_threshold)
                       0.25 means 25% deviation from market average

        Returns:
            List of Anomaly objects
        """
        if threshold is None:
            threshold = self.default_threshold

        grouped = self._group_by_match(events)
        anomalies = []

        for match_key, match_events in grouped.items():
            unique_bks = set(e.get("bookmaker", "") for e in match_events if e.get("bookmaker"))
            if len(unique_bks) < self.min_bookmakers:
                continue

            stats = self.compute_market_stats(match_events, match_key)
            if not stats:
                continue

            for event in match_events:
                bk = event.get("bookmaker", "")
                if not bk:
                    continue

                event_anomalies = self._check_event_anomalies(
                    event, stats, match_key, match_events, threshold
                )
                anomalies.extend(event_anomalies)

        anomalies.sort(key=lambda a: a.deviation_percent, reverse=True)
        self._detection_count += len(anomalies)

        return anomalies

    def score_anomaly(self, anomaly: Anomaly) -> float:
        """
        Score an anomaly from 0-100. Higher = more likely a real bookmaker error.

        Scoring factors:
        - Deviation magnitude (0-40 points)
        - Number of bookmakers in market (0-20 points)
        - Consensus strength (low stdev among others) (0-20 points)
        - Live vs pre-match (live errors are more valuable) (0-10 points)
        - Odds reasonableness (0-10 points)
        """
        score = 0.0

        deviation = abs(anomaly.deviation_percent)

        if deviation >= 50:
            score += 40
        elif deviation >= 35:
            score += 35
        elif deviation >= 25:
            score += 30
        elif deviation >= 20:
            score += 25
        elif deviation >= 15:
            score += 15
        elif deviation >= 10:
            score += 10
        else:
            score += 0

        bk_count = anomaly.num_bookmakers
        if bk_count >= 7:
            score += 20
        elif bk_count >= 5:
            score += 15
        elif bk_count >= 4:
            score += 10
        elif bk_count >= 3:
            score += 5

        if anomaly.market_average > 0:
            cv = abs(anomaly.anomalous_odds - anomaly.market_average) / anomaly.market_average
            if cv < 0.1:
                score += 10
            elif cv < 0.2:
                score += 10
            elif cv < 0.3:
                score += 5
            else:
                score += 0

        if anomaly.is_live:
            score += 10

        odds = anomaly.anomalous_odds
        if 1.2 <= odds <= 10.0:
            score += 10
        elif 1.05 <= odds <= 20.0:
            score += 5

        anomaly.score = min(score, 100.0)
        return anomaly.score

    def get_errors(
        self,
        events: List[Dict],
        threshold: Optional[float] = None,
        min_score: float = 30.0,
    ) -> List[Dict]:
        """
        Main entry point: find bookmaker errors worth betting on.

        Args:
            events: List of event dicts
            threshold: Deviation threshold
            min_score: Minimum anomaly score to return

        Returns:
            List of error dicts ready for consumption
        """
        anomalies = self.detect_anomalies(events, threshold)

        errors = []
        for anomaly in anomalies:
            score = self.score_anomaly(anomaly)
            if score < min_score:
                continue

            error_dict = anomaly.to_dict()
            error_dict["recommended_stake_pct"] = self._calc_recommended_stake(anomaly)
            error_dict["expected_value"] = self._calc_expected_value(anomaly)
            error_dict["confidence"] = self._confidence_label(score)
            error_dict["action"] = self._recommend_action(anomaly)

            errors.append(error_dict)

        errors.sort(key=lambda e: e["score"], reverse=True)
        return errors

    def get_stats(self) -> Dict:
        return {
            "total_detections": self._detection_count,
            "default_threshold": self.default_threshold,
            "min_bookmakers": self.min_bookmakers,
        }

    def _check_event_anomalies(
        self,
        event: Dict,
        stats: Dict[str, Dict],
        match_key: str,
        all_match_events: List[Dict],
        threshold: float,
    ) -> List[Anomaly]:
        """Check a single event for anomalies across all selections."""
        anomalies = []
        bk = event.get("bookmaker", "")
        is_live = event.get("is_live", False)
        sport = event.get("sport", "football")
        home = event.get("home_team", "")
        away = event.get("away_team", "")
        event_name = f"{home} vs {away}"
        now = datetime.utcnow().isoformat()

        selection_map = {
            "home": event.get("home_odds") or event.get("p1") or event.get("1"),
            "draw": event.get("draw_odds") or event.get("x") or event.get("draw"),
            "away": event.get("away_odds") or event.get("p2") or event.get("2"),
        }

        for selection, odds in selection_map.items():
            if not odds or selection not in stats:
                continue

            odds = float(odds)
            sel_stats = stats[selection]
            market_avg = sel_stats["mean"]
            market_stdev = sel_stats["stdev"]
            other_values = sel_stats["values"]

            deviation = (odds - market_avg) / market_avg if market_avg > 0 else 0

            if abs(deviation) < threshold:
                continue

            if self._false_positive_filter and not self._is_real_anomaly(
                odds, other_values, bk, all_match_events, selection
            ):
                continue

            num_bks = sel_stats["count"]

            anomaly = Anomaly(
                event_name=event_name,
                sport=sport,
                bookmaker=bk,
                selection=selection,
                anomalous_odds=odds,
                market_average=market_avg,
                deviation_percent=deviation * 100,
                num_bookmakers=num_bks,
                is_live=is_live,
                detected_at=now,
            )
            anomalies.append(anomaly)

        return anomalies

    def _is_real_anomaly(
        self,
        odds: float,
        other_values: List[float],
        bk: str,
        all_events: List[Dict],
        selection: str,
    ) -> bool:
        """Filter out false positives."""
        if len(other_values) < 3:
            return True

        sorted_vals = sorted(other_values)
        median_val = statistics.median(sorted_vals)

        if abs(odds - median_val) / median_val < 0.1:
            return False

        same_bk_events = [e for e in all_events if e.get("bookmaker") == bk]
        if len(same_bk_events) >= 2:
            bk_selections = []
            for e in same_bk_events:
                val = e.get(f"{selection}_odds") or e.get(selection[:1])
                if val:
                    bk_selections.append(float(val))
            if bk_selections:
                bk_avg = statistics.mean(bk_selections)
                market_avg = statistics.mean(other_values)
                if abs(bk_avg - market_avg) / market_avg < 0.05:
                    return False

        return True

    def _calc_recommended_stake(self, anomaly: Anomaly) -> float:
        """Calculate recommended stake percentage based on anomaly score."""
        score = anomaly.score
        if score >= 80:
            return 5.0
        elif score >= 60:
            return 3.0
        elif score >= 40:
            return 2.0
        else:
            return 1.0

    def _calc_expected_value(self, anomaly: Anomaly) -> float:
        """Calculate expected value of betting on the anomalous odds."""
        market_avg = anomaly.market_average
        if market_avg <= 0:
            return 0.0

        market_prob = 1.0 / market_avg
        implied_prob = 1.0 / anomaly.anomalous_odds if anomaly.anomalous_odds > 0 else 0

        if implied_prob <= 0:
            return 0.0

        ev = (market_prob * anomaly.anomalous_odds) - 1.0
        return round(ev * 100, 2)

    def _confidence_label(self, score: float) -> str:
        if score >= 80:
            return "VERY HIGH"
        elif score >= 60:
            return "HIGH"
        elif score >= 40:
            return "MEDIUM"
        elif score >= 20:
            return "LOW"
        return "VERY LOW"

    def _recommend_action(self, anomaly: Anomaly) -> str:
        if anomaly.anomalous_odds > anomaly.market_average:
            return f"BET {anomaly.selection.upper()} @ {anomaly.anomalous_odds:.2f} (market avg: {anomaly.market_average:.2f})"
        else:
            return f"AVOID {anomaly.selection.upper()} @ {anomaly.anomalous_odds:.2f} (market avg: {anomaly.market_average:.2f})"

    @staticmethod
    def _get_match_key(event: Dict) -> str:
        home = event.get("home_team", "").lower().strip()
        away = event.get("away_team", "").lower().strip()
        if home and away:
            return f"{home}|{away}"
        return ""

    @staticmethod
    def _group_by_match(events: List[Dict]) -> Dict[str, List[Dict]]:
        grouped = defaultdict(list)
        for e in events:
            key = OddsErrorDetector._get_match_key(e)
            if key:
                grouped[key].append(e)
        return dict(grouped)
