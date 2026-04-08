# core/odds_verifier.py
"""
Odds Verifier — Idea #14
Re-checks surebet odds immediately before notification to filter expired opportunities.

Problem: 40-60% of surebets expire between detection and placement.
Solution: Re-validate each leg's odds against current event data with ±tolerance.
"""
import time
import logging
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from collections import defaultdict

logger = logging.getLogger(__name__)


@dataclass
class VerificationResult:
    surebet_id: str
    is_valid: bool
    original_profit: float
    verified_profit: float
    expired_legs: List[Dict] = field(default_factory=list)
    reason: str = ""
    verified_at: float = 0.0


@dataclass
class VerifierStats:
    total_checked: int = 0
    total_valid: int = 0
    total_expired: int = 0
    total_legs_checked: int = 0
    total_legs_expired: int = 0
    avg_verification_time_ms: float = 0.0
    _times: list = field(default_factory=list)

    def record(self, duration_ms: float):
        self._times.append(duration_ms)
        if len(self._times) > 1000:
            self._times = self._times[-500:]
        self.avg_verification_time_ms = sum(self._times) / len(self._times)

    def summary(self) -> Dict:
        return {
            "total_checked": self.total_checked,
            "total_valid": self.total_valid,
            "total_expired": self.total_expired,
            "total_legs_checked": self.total_legs_checked,
            "total_legs_expired": self.total_legs_expired,
            "validation_rate": round(
                self.total_valid / self.total_checked * 100, 2
            ) if self.total_checked > 0 else 0.0,
            "avg_verification_time_ms": round(self.avg_verification_time_ms, 2),
        }


class OddsVerifier:
    """
    Verifies surebet odds against fresh event data before notification.

    Tolerance: ±0.02 odds change is acceptable (configurable).
    If any leg's odds moved beyond tolerance, the surebet is marked expired.
    """

    def __init__(self, tolerance: float = 0.02, min_profit: float = 0.0):
        self.tolerance = tolerance
        self.min_profit = min_profit
        self.stats = VerifierStats()
        self._odds_history: Dict[str, Dict] = {}

    def verify_surebet(self, surebet: Dict, events: List[Dict]) -> VerificationResult:
        """
        Verify a single surebet against current events.
        Returns VerificationResult with validity status.
        """
        start = time.monotonic()
        self.stats.total_checked += 1

        sb_id = surebet.get("id", "unknown")
        legs = surebet.get("legs", [])
        original_profit = surebet.get("profit_percent", 0.0)

        if not legs:
            result = VerificationResult(
                surebet_id=sb_id,
                is_valid=False,
                original_profit=original_profit,
                verified_profit=0.0,
                reason="No legs in surebet",
                verified_at=time.time(),
            )
            self.stats.total_expired += 1
            self._record_time(start)
            return result

        event_map = self._build_event_map(events)
        verified_legs = []
        expired_legs = []
        verified_odds = []

        for leg in legs:
            bk = leg.get("bookmaker", "")
            market = leg.get("market", "")
            selection = leg.get("selection", "")
            original_odds = leg.get("odds", 0.0)
            event_name = leg.get("event_name", "")

            current_odds = self._find_current_odds(
                event_name, bk, market, selection, event_map
            )

            self.stats.total_legs_checked += 1

            if current_odds is None:
                expired_legs.append({
                    **leg,
                    "reason": "Event not found",
                    "original_odds": original_odds,
                    "current_odds": None,
                })
                self.stats.total_legs_expired += 1
                continue

            if self._odds_changed(original_odds, current_odds):
                expired_legs.append({
                    **leg,
                    "reason": "Odds changed beyond tolerance",
                    "original_odds": original_odds,
                    "current_odds": current_odds,
                })
                self.stats.total_legs_expired += 1
            else:
                verified_legs.append(leg)
                verified_odds.append(current_odds)

        if expired_legs:
            verified_profit = self._calculate_profit(verified_odds) if verified_odds else 0.0
            is_valid = (
                len(verified_legs) == len(legs)
                and verified_profit >= self.min_profit
            )
            result = VerificationResult(
                surebet_id=sb_id,
                is_valid=is_valid,
                original_profit=original_profit,
                verified_profit=verified_profit,
                expired_legs=expired_legs,
                reason=f"{len(expired_legs)} leg(s) expired",
                verified_at=time.time(),
            )
        else:
            verified_profit = self._calculate_profit(verified_odds)
            is_valid = verified_profit >= self.min_profit
            result = VerificationResult(
                surebet_id=sb_id,
                is_valid=is_valid,
                original_profit=original_profit,
                verified_profit=verified_profit,
                verified_at=time.time(),
            )

        if result.is_valid:
            self.stats.total_valid += 1
        else:
            self.stats.total_expired += 1

        self._record_time(start)
        return result

    def verify_odds(self, event: Dict, tolerance: Optional[float] = None) -> bool:
        """
        Verify that an event's odds are still valid (not stale).
        Checks home_odds and away_odds against cached values.
        """
        tol = tolerance if tolerance is not None else self.tolerance
        event_key = self._event_key(event)

        if not event_key:
            return False

        home_odds = event.get("home_odds", 0.0)
        away_odds = event.get("away_odds", 0.0)

        if home_odds <= 1.0 or away_odds <= 1.0:
            return False

        cached = self._odds_history.get(event_key)
        if cached:
            home_changed = abs(home_odds - cached.get("home_odds", 0.0)) > tol
            away_changed = abs(away_odds - cached.get("away_odds", 0.0)) > tol
            if home_changed or away_changed:
                logger.debug(
                    f"Odds drift detected for {event_key}: "
                    f"home {cached.get('home_odds')}->{home_odds}, "
                    f"away {cached.get('away_odds')}->{away_odds}"
                )

        self._odds_history[event_key] = {
            "home_odds": home_odds,
            "away_odds": away_odds,
            "ts": time.time(),
        }

        return True

    def get_expired_surebets(self, surebets: List[Dict], events: List[Dict]) -> List[Dict]:
        """
        Return list of surebets that have expired (odds no longer valid).
        """
        expired = []
        for sb in surebets:
            result = self.verify_surebet(sb, events)
            if not result.is_valid:
                expired.append({
                    **sb,
                    "verification": {
                        "reason": result.reason,
                        "verified_profit": result.verified_profit,
                        "expired_legs": result.expired_legs,
                        "verified_at": result.verified_at,
                    },
                })
        return expired

    def get_valid_surebets(self, surebets: List[Dict], events: List[Dict]) -> List[Dict]:
        """
        Return list of surebets that are still valid (odds confirmed).
        """
        valid = []
        for sb in surebets:
            result = self.verify_surebet(sb, events)
            if result.is_valid:
                valid.append({
                    **sb,
                    "verified_profit": result.verified_profit,
                    "verified_at": result.verified_at,
                })
        return valid

    def verify_batch(self, surebets: List[Dict], events: List[Dict]) -> Tuple[List[Dict], List[Dict]]:
        """
        Verify a batch of surebets. Returns (valid, expired) lists.
        """
        valid = []
        expired = []

        for sb in surebets:
            result = self.verify_surebet(sb, events)
            if result.is_valid:
                valid.append({
                    **sb,
                    "verified_profit": result.verified_profit,
                    "verified_at": result.verified_at,
                })
            else:
                expired.append({
                    **sb,
                    "verification": {
                        "reason": result.reason,
                        "verified_profit": result.verified_profit,
                        "expired_legs": result.expired_legs,
                        "verified_at": result.verified_at,
                    },
                })

        return valid, expired

    def clear_history(self):
        """Clear odds history cache."""
        self._odds_history.clear()

    def prune_history(self, max_age: float = 300.0):
        """Remove stale entries from odds history."""
        now = time.time()
        stale = [k for k, v in self._odds_history.items() if now - v.get("ts", 0) > max_age]
        for k in stale:
            del self._odds_history[k]
        return len(stale)

    def get_stats(self) -> Dict:
        return self.stats.summary()

    def _build_event_map(self, events: List[Dict]) -> Dict[str, Dict]:
        event_map = {}
        for event in events:
            key = self._event_key(event)
            if key:
                event_map[key] = event
        return event_map

    def _event_key(self, event: Dict) -> str:
        home = event.get("home_team", "").lower().strip()
        away = event.get("away_team", "").lower().strip()
        bk = event.get("bookmaker", "").lower().strip()
        if home and away and bk:
            return f"{home}|{away}|{bk}"
        return ""

    def _find_current_odds(
        self,
        event_name: str,
        bookmaker: str,
        market: str,
        selection: str,
        event_map: Dict[str, Dict],
    ) -> Optional[float]:
        for key, event in event_map.items():
            if bookmaker.lower() != event.get("bookmaker", "").lower():
                continue

            if not self._name_matches(event_name, event):
                continue

            odds = self._extract_odds(event, market, selection)
            if odds is not None:
                return odds

        return None

    def _name_matches(self, event_name: str, event: Dict) -> bool:
        home = event.get("home_team", "").lower()
        away = event.get("away_team", "").lower()
        name_lower = event_name.lower()
        return home in name_lower and away in name_lower

    def _extract_odds(self, event: Dict, market: str, selection: str) -> Optional[float]:
        if market in ("1", "П1", "P1", "1X2_1"):
            return event.get("home_odds")
        elif market in ("X", "Ничья", "Draw", "1X2_X"):
            return event.get("draw_odds")
        elif market in ("2", "П2", "P2", "1X2_2"):
            return event.get("away_odds")
        elif market in ("ТБ", "Over", "Total Over"):
            return event.get("over_odds") or event.get("total_over_odds")
        elif market in ("ТМ", "Under", "Total Under"):
            return event.get("under_odds") or event.get("total_under_odds")
        elif market.startswith("Total"):
            parts = market.split()
            if len(parts) >= 2:
                try:
                    line = float(parts[1])
                    totals_over = event.get("total_over", {})
                    totals_under = event.get("total_under", {})
                    if selection.startswith("ТБ") or selection.startswith("Over"):
                        return totals_over.get(line) or totals_over.get(str(line))
                    else:
                        return totals_under.get(line) or totals_under.get(str(line))
                except (ValueError, TypeError):
                    pass
        elif market.startswith("Handicap"):
            parts = market.split()
            if len(parts) >= 2:
                try:
                    line = float(parts[1].replace("+", "").replace("−", "-"))
                    hc_home = event.get("handicap_home", {})
                    hc_away = event.get("handicap_away", {})
                    if selection.startswith("Ф1"):
                        return hc_home.get(line) or hc_home.get(str(line))
                    else:
                        return hc_away.get(-line) or hc_away.get(str(-line))
                except (ValueError, TypeError):
                    pass

        return None

    def _odds_changed(self, original: float, current: float) -> bool:
        if current is None or original <= 0:
            return True
        return abs(current - original) > self.tolerance

    def _calculate_profit(self, odds: List[float]) -> float:
        if len(odds) < 2:
            return 0.0
        margin = sum(1.0 / o for o in odds if o > 1.0)
        if margin <= 0 or margin >= 1:
            return 0.0
        return (1.0 / margin - 1.0) * 100.0

    def _record_time(self, start: float):
        elapsed_ms = (time.monotonic() - start) * 1000
        self.stats.record(elapsed_ms)
