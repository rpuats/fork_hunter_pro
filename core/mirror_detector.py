# core/mirror_detector.py
"""
Mirror Line Detector — detects which bookmakers copy lines from each other.

Classifies BK pairs as:
  - "mirror"      (Pearson r > threshold, default 0.95)  → skip surebet search
  - "independent" (r < 0.80)                              → priority for search
  - "unknown"     (0.80 ≤ r ≤ 0.95)                       → normal processing
"""

from __future__ import annotations

import logging
import math
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

from core.team_normalizer import team_normalizer

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _pearson(x: List[float], y: List[float]) -> float:
    """Pearson correlation coefficient for two equally-sized lists."""
    n = len(x)
    if n < 3:
        return 0.0
    mean_x = sum(x) / n
    mean_y = sum(y) / n
    cov = sum((xi - mean_x) * (yi - mean_y) for xi, yi in zip(x, y))
    std_x = math.sqrt(sum((xi - mean_x) ** 2 for xi in x))
    std_y = math.sqrt(sum((yi - mean_y) ** 2 for yi in y))
    if std_x == 0 or std_y == 0:
        return 0.0
    return cov / (std_x * std_y)


def _make_pair(bk1: str, bk2: str) -> Tuple[str, str]:
    """Canonical (sorted) pair key."""
    return tuple(sorted((bk1, bk2)))  # type: ignore[return-value]


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class MirrorStats:
    """Aggregated statistics for one BK pair."""
    correlation: float = 0.0
    common_events: int = 0
    classification: str = "unknown"
    avg_odds_diff: float = 0.0


# ---------------------------------------------------------------------------
# Main class
# ---------------------------------------------------------------------------

class MirrorLineDetector:
    """Detects mirror-line relationships between bookmakers."""

    def __init__(
        self,
        mirror_threshold: float = 0.95,
        independent_threshold: float = 0.80,
        min_common_events: int = 3,
    ):
        self.mirror_threshold = mirror_threshold
        self.independent_threshold = independent_threshold
        self.min_common_events = min_common_events

        self._correlation_matrix: Dict[Tuple[str, str], float] = {}
        self._pair_stats: Dict[Tuple[str, str], MirrorStats] = {}
        self._last_cycle_pairs: int = 0

    # -- public API ----------------------------------------------------------

    def compute_correlation_matrix(self, events: List[Dict]) -> Dict[Tuple[str, str], float]:
        """
        Build a correlation matrix from a list of event dicts.

        Each event must contain at least:
            - bookmaker (str)
            - home_team, away_team (str)
            - home_odds, away_odds (float)

        Returns a dict {(bk_a, bk_b): correlation_float}.
        """
        # 1. Group events by normalised match key
        match_groups: Dict[str, Dict[str, Dict]] = defaultdict(dict)  # match_key -> {bk -> event}
        for ev in events:
            home = ev.get("home_team", "")
            away = ev.get("away_team", "")
            bk = ev.get("bookmaker", "")
            if not home or not away or not bk:
                continue
            match_key = team_normalizer.get_key(home, away)
            match_groups[match_key][bk] = ev

        # 2. Collect per-BK odds vectors
        bk_names: set[str] = set()
        for match_bks in match_groups.values():
            bk_names.update(match_bks.keys())

        if len(bk_names) < 2:
            self._correlation_matrix = {}
            return self._correlation_matrix

        # 3. For each pair, build aligned odds vectors and compute Pearson r
        sorted_bks = sorted(bk_names)
        corr_matrix: Dict[Tuple[str, str], float] = {}
        pair_stats: Dict[Tuple[str, str], MirrorStats] = {}

        for i in range(len(sorted_bks)):
            for j in range(i + 1, len(sorted_bks)):
                bk_a, bk_b = sorted_bks[i], sorted_bks[j]
                pair = _make_pair(bk_a, bk_b)

                home_odds_a: List[float] = []
                home_odds_b: List[float] = []
                away_odds_a: List[float] = []
                away_odds_b: List[float] = []
                diffs: List[float] = []

                for match_bks in match_groups.values():
                    ev_a = match_bks.get(bk_a)
                    ev_b = match_bks.get(bk_b)
                    if ev_a is None or ev_b is None:
                        continue
                    ha = ev_a.get("home_odds", 0)
                    hb = ev_b.get("home_odds", 0)
                    aa = ev_a.get("away_odds", 0)
                    ab = ev_b.get("away_odds", 0)
                    if ha > 1 and hb > 1 and aa > 1 and ab > 1:
                        home_odds_a.append(ha)
                        home_odds_b.append(hb)
                        away_odds_a.append(aa)
                        away_odds_b.append(ab)
                        diffs.append(abs(ha - hb))
                        diffs.append(abs(aa - ab))

                common = len(home_odds_a)
                if common < self.min_common_events:
                    corr_matrix[pair] = 0.0
                    pair_stats[pair] = MirrorStats(
                        correlation=0.0,
                        common_events=common,
                        classification="unknown",
                    )
                    continue

                r_home = _pearson(home_odds_a, home_odds_b)
                r_away = _pearson(away_odds_a, away_odds_b)
                r_combined = (r_home + r_away) / 2

                avg_diff = sum(diffs) / len(diffs) if diffs else 0.0

                corr_matrix[pair] = r_combined
                pair_stats[pair] = MirrorStats(
                    correlation=round(r_combined, 4),
                    common_events=common,
                    avg_odds_diff=round(avg_diff, 4),
                )

        self._correlation_matrix = corr_matrix
        self._pair_stats = pair_stats
        self._last_cycle_pairs = len(corr_matrix)

        # Classify
        self._classify_all()

        logger.info(
            f"[MirrorDetector] Computed {len(corr_matrix)} pairs, "
            f"{sum(1 for s in pair_stats.values() if s.classification == 'mirror')} mirrors, "
            f"{sum(1 for s in pair_stats.values() if s.classification == 'independent')} independent"
        )

        return corr_matrix

    def classify_pairs(
        self,
        mirror_threshold: Optional[float] = None,
        independent_threshold: Optional[float] = None,
    ) -> Dict[Tuple[str, str], str]:
        """Return {pair: classification} using current or overridden thresholds."""
        mt = mirror_threshold if mirror_threshold is not None else self.mirror_threshold
        it = independent_threshold if independent_threshold is not None else self.independent_threshold

        result: Dict[Tuple[str, str], str] = {}
        for pair, stats in self._pair_stats.items():
            r = stats.correlation
            if stats.common_events < self.min_common_events:
                result[pair] = "unknown"
            elif r >= mt:
                result[pair] = "mirror"
            elif r <= it:
                result[pair] = "independent"
            else:
                result[pair] = "unknown"
        return result

    def get_independent_pairs(self) -> List[Tuple[str, str]]:
        """Return list of pairs classified as 'independent'."""
        return [p for p, c in self.classify_pairs().items() if c == "independent"]

    def get_mirror_pairs(self) -> List[Tuple[str, str]]:
        """Return list of pairs classified as 'mirror'."""
        return [p for p, c in self.classify_pairs().items() if c == "mirror"]

    def get_dependency_map(self) -> Dict[str, List[str]]:
        """Return {bk: [correlated_bks]} for all mirror relationships."""
        dep_map: Dict[str, List[str]] = defaultdict(list)
        for (bk_a, bk_b), stats in self._pair_stats.items():
            if stats.classification == "mirror":
                dep_map[bk_a].append(bk_b)
                dep_map[bk_b].append(bk_a)
        return dict(dep_map)

    def is_mirror_pair(self, bk1: str, bk2: str) -> bool:
        """Quick check: are these two bookmakers mirrors?"""
        pair = _make_pair(bk1, bk2)
        stats = self._pair_stats.get(pair)
        return stats is not None and stats.classification == "mirror"

    def should_skip_pair(self, bk1: str, bk2: str) -> bool:
        """Return True if pair should be skipped during surebet search."""
        return self.is_mirror_pair(bk1, bk2)

    def get_pair_stats(self, bk1: str, bk2: str) -> Optional[MirrorStats]:
        """Get detailed stats for a specific pair."""
        return self._pair_stats.get(_make_pair(bk1, bk2))

    def get_summary(self) -> Dict[str, Any]:
        """Return a human-readable summary dict."""
        classifications = self.classify_pairs()
        mirror_count = sum(1 for c in classifications.values() if c == "mirror")
        indep_count = sum(1 for c in classifications.values() if c == "independent")
        unknown_count = sum(1 for c in classifications.values() if c == "unknown")

        top_mirrors = sorted(
            [(p, s) for p, s in self._pair_stats.items() if s.classification == "mirror"],
            key=lambda x: x[1].correlation,
            reverse=True,
        )[:5]

        return {
            "total_pairs": len(self._pair_stats),
            "mirror_pairs": mirror_count,
            "independent_pairs": indep_count,
            "unknown_pairs": unknown_count,
            "top_mirrors": [
                {"pair": list(p), "correlation": s.correlation, "common_events": s.common_events}
                for p, s in top_mirrors
            ],
            "dependency_map": self.get_dependency_map(),
        }

    def reset(self) -> None:
        """Clear all cached data."""
        self._correlation_matrix.clear()
        self._pair_stats.clear()
        self._last_cycle_pairs = 0

    # -- internal ------------------------------------------------------------

    def _classify_all(self) -> None:
        """Update classification field on all pair stats."""
        for pair, stats in self._pair_stats.items():
            if stats.common_events < self.min_common_events:
                stats.classification = "unknown"
            elif stats.correlation >= self.mirror_threshold:
                stats.classification = "mirror"
            elif stats.correlation <= self.independent_threshold:
                stats.classification = "independent"
            else:
                stats.classification = "unknown"
