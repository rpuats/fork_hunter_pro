# services/reliability.py
"""
Bookmaker Reliability Scorer — tracks per-bookmaker metrics and calculates
a reliability score from 0-100 based on odds stability, uptime, error rate,
and bet acceptance rate.
"""
from typing import List, Dict, Optional
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from collections import defaultdict
import logging
import time

logger = logging.getLogger(__name__)


@dataclass
class ReliabilityMetrics:
    """Per-bookmaker reliability metrics."""
    bookmaker: str
    odds_stability: float = 100.0
    uptime: float = 100.0
    error_rate: float = 0.0
    bet_acceptance: float = 100.0
    total_checks: int = 0
    total_errors: int = 0
    odds_changes: int = 0
    odds_displayed: int = 0
    bets_placed: int = 0
    bets_accepted: int = 0
    last_updated: str = field(default_factory=lambda: datetime.utcnow().isoformat())

    def to_dict(self) -> Dict:
        return {
            'bookmaker': self.bookmaker,
            'odds_stability': round(self.odds_stability, 2),
            'uptime': round(self.uptime, 2),
            'error_rate': round(self.error_rate, 2),
            'bet_acceptance': round(self.bet_acceptance, 2),
            'total_checks': self.total_checks,
            'total_errors': self.total_errors,
            'odds_changes': self.odds_changes,
            'odds_displayed': self.odds_displayed,
            'bets_placed': self.bets_placed,
            'bets_accepted': self.bets_accepted,
            'last_updated': self.last_updated,
        }


@dataclass
class ReliabilityScore:
    """Composite reliability score for a bookmaker."""
    bookmaker: str
    score: float
    grade: str
    metrics: ReliabilityMetrics
    breakdown: Dict[str, float]
    recommendation: str

    def to_dict(self) -> Dict:
        return {
            'bookmaker': self.bookmaker,
            'score': round(self.score, 2),
            'grade': self.grade,
            'recommendation': self.recommendation,
            'breakdown': {k: round(v, 2) for k, v in self.breakdown.items()},
            'metrics': self.metrics.to_dict(),
        }


class ReliabilityScorer:
    """
    Calculates and tracks reliability scores for bookmakers.

    Score components (weighted):
    - odds_stability (30%): How often odds remain unchanged after display
    - uptime (25%): Parser/API availability
    - error_rate (25%): Inverse of error frequency
    - bet_acceptance (20%): Percentage of bets accepted without issues

    Score range: 0-100
    """

    WEIGHTS = {
        'odds_stability': 0.30,
        'uptime': 0.25,
        'error_rate': 0.25,
        'bet_acceptance': 0.20,
    }

    GRADES = [
        (90, "A+", "Excellent — highly reliable"),
        (80, "A", "Very good — reliable"),
        (70, "B+", "Good — mostly reliable"),
        (60, "B", "Acceptable — use with caution"),
        (50, "C+", "Below average — monitor closely"),
        (40, "C", "Poor — consider alternatives"),
        (0, "D", "Unreliable — avoid"),
    ]

    def __init__(self):
        self._metrics: Dict[str, ReliabilityMetrics] = {}
        self._uptime_tracker: Dict[str, List[bool]] = defaultdict(list)
        self._max_uptime_entries = 1000

    def _get_or_create(self, bookmaker: str) -> ReliabilityMetrics:
        if bookmaker not in self._metrics:
            self._metrics[bookmaker] = ReliabilityMetrics(bookmaker=bookmaker)
        return self._metrics[bookmaker]

    def record_odds_display(self, bookmaker: str):
        """Record that odds were displayed for a bookmaker."""
        metrics = self._get_or_create(bookmaker)
        metrics.odds_displayed += 1
        metrics.total_checks += 1
        metrics.last_updated = datetime.utcnow().isoformat()

    def record_odds_change(self, bookmaker: str):
        """Record that odds changed after initial display (instability)."""
        metrics = self._get_or_create(bookmaker)
        metrics.odds_changes += 1

    def record_error(self, bookmaker: str):
        """Record a parser/API error for a bookmaker."""
        metrics = self._get_or_create(bookmaker)
        metrics.total_errors += 1
        metrics.total_checks += 1
        metrics.last_updated = datetime.utcnow().isoformat()

    def record_check(self, bookmaker: str, success: bool):
        """Record a health check result."""
        metrics = self._get_or_create(bookmaker)
        metrics.total_checks += 1
        if not success:
            metrics.total_errors += 1

        tracker = self._uptime_tracker[bookmaker]
        tracker.append(success)
        if len(tracker) > self._max_uptime_entries:
            tracker.pop(0)

        metrics.last_updated = datetime.utcnow().isoformat()

    def record_bet(self, bookmaker: str, accepted: bool):
        """Record a bet placement result."""
        metrics = self._get_or_create(bookmaker)
        metrics.bets_placed += 1
        if accepted:
            metrics.bets_accepted += 1
        metrics.last_updated = datetime.utcnow().isoformat()

    def _calculate_odds_stability(self, metrics: ReliabilityMetrics) -> float:
        """Calculate odds stability percentage."""
        if metrics.odds_displayed == 0:
            return 100.0
        stable = metrics.odds_displayed - metrics.odds_changes
        return max(0.0, (stable / metrics.odds_displayed) * 100)

    def _calculate_uptime(self, metrics: ReliabilityMetrics) -> float:
        """Calculate uptime percentage from tracker."""
        tracker = self._uptime_tracker.get(metrics.bookmaker, [])
        if not tracker:
            if metrics.total_checks == 0:
                return 100.0
            return max(0.0, ((metrics.total_checks - metrics.total_errors) / metrics.total_checks) * 100)
        successful = sum(1 for s in tracker if s)
        return (successful / len(tracker)) * 100

    def _calculate_error_rate(self, metrics: ReliabilityMetrics) -> float:
        """Calculate inverse error rate (higher is better)."""
        if metrics.total_checks == 0:
            return 100.0
        error_rate = metrics.total_errors / metrics.total_checks
        return max(0.0, (1.0 - error_rate) * 100)

    def _calculate_bet_acceptance(self, metrics: ReliabilityMetrics) -> float:
        """Calculate bet acceptance rate."""
        if metrics.bets_placed == 0:
            return 100.0
        return (metrics.bets_accepted / metrics.bets_placed) * 100

    def calculate_score(self, bookmaker: str) -> ReliabilityScore:
        """
        Calculate composite reliability score for a bookmaker.

        Args:
            bookmaker: Bookmaker slug.

        Returns:
            ReliabilityScore with breakdown and grade.
        """
        metrics = self._get_or_create(bookmaker)

        odds_stability = self._calculate_odds_stability(metrics)
        uptime = self._calculate_uptime(metrics)
        error_rate_score = self._calculate_error_rate(metrics)
        bet_acceptance = self._calculate_bet_acceptance(metrics)

        metrics.odds_stability = odds_stability
        metrics.uptime = uptime
        metrics.error_rate = round((100 - error_rate_score), 2)
        metrics.bet_acceptance = bet_acceptance

        score = (
            odds_stability * self.WEIGHTS['odds_stability']
            + uptime * self.WEIGHTS['uptime']
            + error_rate_score * self.WEIGHTS['error_rate']
            + bet_acceptance * self.WEIGHTS['bet_acceptance']
        )

        score = max(0.0, min(100.0, score))

        grade = "D"
        recommendation = "Unreliable — avoid"
        for threshold, g, rec in self.GRADES:
            if score >= threshold:
                grade = g
                recommendation = rec
                break

        breakdown = {
            'odds_stability': odds_stability * self.WEIGHTS['odds_stability'],
            'uptime': uptime * self.WEIGHTS['uptime'],
            'error_rate': error_rate_score * self.WEIGHTS['error_rate'],
            'bet_acceptance': bet_acceptance * self.WEIGHTS['bet_acceptance'],
        }

        return ReliabilityScore(
            bookmaker=bookmaker,
            score=score,
            grade=grade,
            metrics=metrics,
            breakdown=breakdown,
            recommendation=recommendation,
        )

    def get_all_scores(self) -> List[ReliabilityScore]:
        """Get reliability scores for all tracked bookmakers."""
        return [self.calculate_score(bk) for bk in self._metrics]

    def get_summary(self) -> Dict:
        """Get summary of all bookmaker reliability scores."""
        scores = self.get_all_scores()
        if not scores:
            return {'bookmakers': [], 'average_score': 0}

        avg = sum(s.score for s in scores) / len(scores)
        scores.sort(key=lambda x: x.score, reverse=True)

        return {
            'bookmakers': [s.to_dict() for s in scores],
            'average_score': round(avg, 2),
            'total_tracked': len(scores),
        }

    def apply_from_parser_stats(self, bookmaker: str, parser_stats: Dict) -> None:
        """
        Apply parser statistics to reliability metrics.

        Args:
            bookmaker: Bookmaker slug.
            parser_stats: Stats dict from parser (events, requests, errors).
        """
        metrics = self._get_or_create(bookmaker)
        errors = parser_stats.get('errors', 0)
        events = parser_stats.get('events', 0)

        if errors > 0:
            self.record_error(bookmaker)

        if events > 0:
            for _ in range(min(events, 10)):
                self.record_odds_display(bookmaker)

    def reset(self, bookmaker: Optional[str] = None):
        """Reset metrics for a specific bookmaker or all."""
        if bookmaker:
            self._metrics.pop(bookmaker, None)
            self._uptime_tracker.pop(bookmaker, None)
        else:
            self._metrics.clear()
            self._uptime_tracker.clear()
