# tests/test_mirror_detector.py
"""Tests for MirrorLineDetector module."""

import pytest
from core.mirror_detector import MirrorLineDetector, _pearson, _make_pair


# -- Helpers -----------------------------------------------------------------

def _make_event(home, away, bk, home_odds, away_odds, is_live=True):
    return {
        "home_team": home,
        "away_team": away,
        "bookmaker": bk,
        "home_odds": home_odds,
        "away_odds": away_odds,
        "is_live": is_live,
    }


# -- _pearson helper tests ---------------------------------------------------

class TestPearsonHelper:
    def test_perfect_positive_correlation(self):
        x = [1.0, 2.0, 3.0, 4.0, 5.0]
        y = [1.0, 2.0, 3.0, 4.0, 5.0]
        assert _pearson(x, y) == pytest.approx(1.0, rel=1e-9)

    def test_perfect_negative_correlation(self):
        x = [1.0, 2.0, 3.0, 4.0, 5.0]
        y = [5.0, 4.0, 3.0, 2.0, 1.0]
        assert _pearson(x, y) == pytest.approx(-1.0, rel=1e-9)

    def test_no_correlation(self):
        x = [1.0, 2.0, 3.0, 4.0, 5.0]
        y = [5.0, 1.0, 4.0, 2.0, 3.0]
        r = _pearson(x, y)
        assert abs(r) < 0.5

    def test_too_few_points(self):
        assert _pearson([1.0], [2.0]) == 0.0
        assert _pearson([1.0, 2.0], [3.0, 4.0]) == 0.0

    def test_zero_std(self):
        assert _pearson([2.0, 2.0, 2.0], [1.0, 2.0, 3.0]) == 0.0


# -- _make_pair tests --------------------------------------------------------

class TestMakePair:
    def test_sorted_order(self):
        assert _make_pair("zenit", "pari") == ("pari", "zenit")

    def test_same_order_invariant(self):
        assert _make_pair("a", "b") == _make_pair("b", "a")

    def test_identical(self):
        assert _make_pair("x", "x") == ("x", "x")


# -- MirrorLineDetector tests ------------------------------------------------

class TestMirrorLineDetector:
    def test_init_defaults(self):
        detector = MirrorLineDetector()
        assert detector.mirror_threshold == 0.95
        assert detector.independent_threshold == 0.80
        assert detector.min_common_events == 3

    def test_init_custom_thresholds(self):
        detector = MirrorLineDetector(
            mirror_threshold=0.90,
            independent_threshold=0.70,
            min_common_events=5,
        )
        assert detector.mirror_threshold == 0.90
        assert detector.independent_threshold == 0.70
        assert detector.min_common_events == 5

    def test_empty_events(self):
        detector = MirrorLineDetector()
        result = detector.compute_correlation_matrix([])
        assert result == {}

    def test_single_bookmaker(self):
        events = [
            _make_event("Team A", "Team B", "winline", 1.95, 2.05),
            _make_event("Team C", "Team D", "winline", 2.10, 1.85),
        ]
        detector = MirrorLineDetector()
        result = detector.compute_correlation_matrix(events)
        assert result == {}

    def test_mirror_pair_detection(self):
        """Two BKs with nearly identical odds should be classified as mirror."""
        events = []
        for i in range(10):
            base_home = 1.80 + i * 0.05
            base_away = 2.20 - i * 0.03
            events.append(_make_event(f"Team{i}", f"Team{i+1}", "bk_a", base_home, base_away))
            events.append(_make_event(f"Team{i}", f"Team{i+1}", "bk_b", base_home + 0.001, base_away + 0.001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        classifications = detector.classify_pairs()
        assert classifications[("bk_a", "bk_b")] == "mirror"

    def test_independent_pair_detection(self):
        """Two BKs with very different odds patterns should be independent."""
        events = []
        for i in range(10):
            events.append(_make_event(f"Team{i}", f"Team{i+1}", "bk_x", 1.50 + i * 0.1, 2.50 - i * 0.05))
            events.append(_make_event(f"Team{i}", f"Team{i+1}", "bk_y", 3.00 - i * 0.1, 1.30 + i * 0.08))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        classifications = detector.classify_pairs()
        assert classifications[("bk_x", "bk_y")] == "independent"

    def test_get_independent_pairs(self):
        events = []
        for i in range(10):
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_a", 1.5 + i * 0.1, 2.5 - i * 0.05))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_b", 3.0 - i * 0.1, 1.3 + i * 0.08))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        indep = detector.get_independent_pairs()
        assert ("bk_a", "bk_b") in indep

    def test_get_mirror_pairs(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_c", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_d", base + 0.001, 2.20 - i * 0.03 + 0.001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        mirrors = detector.get_mirror_pairs()
        assert ("bk_c", "bk_d") in mirrors

    def test_get_dependency_map(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_a", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_b", base + 0.001, 2.20 - i * 0.03 + 0.001))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_c", base + 0.002, 2.20 - i * 0.03 + 0.002))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        dep_map = detector.get_dependency_map()
        assert "bk_a" in dep_map
        assert "bk_b" in dep_map["bk_a"] or "bk_c" in dep_map["bk_a"]

    def test_is_mirror_pair(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_m", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_n", base + 0.0001, 2.20 - i * 0.03 + 0.0001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        assert detector.is_mirror_pair("bk_m", "bk_n") is True
        assert detector.is_mirror_pair("bk_m", "nonexistent") is False

    def test_should_skip_pair(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_p", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_q", base + 0.0001, 2.20 - i * 0.03 + 0.0001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        assert detector.should_skip_pair("bk_p", "bk_q") is True

    def test_get_pair_stats(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_r", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_s", base + 0.001, 2.20 - i * 0.03 + 0.001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        stats = detector.get_pair_stats("bk_r", "bk_s")
        assert stats is not None
        assert stats.correlation > 0.95
        assert stats.common_events == 10

    def test_get_summary(self):
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_u", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_v", base + 0.001, 2.20 - i * 0.03 + 0.001))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        summary = detector.get_summary()
        assert "total_pairs" in summary
        assert "mirror_pairs" in summary
        assert "independent_pairs" in summary
        assert "dependency_map" in summary
        assert summary["total_pairs"] >= 1

    def test_reset(self):
        events = [
            _make_event("A", "B", "bk_x", 1.9, 2.1),
            _make_event("A", "B", "bk_y", 1.9, 2.1),
        ] * 10
        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        assert len(detector._pair_stats) > 0
        detector.reset()
        assert len(detector._pair_stats) == 0
        assert len(detector._correlation_matrix) == 0

    def test_min_common_events_filter(self):
        """Pairs with fewer than min_common_events should be 'unknown'."""
        events = [
            _make_event("A", "B", "bk_a", 1.9, 2.1),
            _make_event("A", "B", "bk_b", 1.9, 2.1),
        ]
        detector = MirrorLineDetector(min_common_events=3)
        detector.compute_correlation_matrix(events)
        classifications = detector.classify_pairs()
        assert classifications[("bk_a", "bk_b")] == "unknown"

    def test_three_bookmakers_mixed(self):
        """Three BKs: two mirrors, one independent."""
        events = []
        for i in range(10):
            base = 1.80 + i * 0.05
            events.append(_make_event(f"T{i}", f"T{i+1}", "mirror_a", base, 2.20 - i * 0.03))
            events.append(_make_event(f"T{i}", f"T{i+1}", "mirror_b", base + 0.001, 2.20 - i * 0.03 + 0.001))
            events.append(_make_event(f"T{i}", f"T{i+1}", "indep_c", 3.0 - i * 0.1, 1.3 + i * 0.08))

        detector = MirrorLineDetector()
        detector.compute_correlation_matrix(events)
        classifications = detector.classify_pairs()

        assert classifications[("mirror_a", "mirror_b")] == "mirror"
        assert classifications[("indep_c", "mirror_a")] == "independent"
        assert classifications[("indep_c", "mirror_b")] == "independent"

    def test_classification_with_custom_thresholds(self):
        """classify_pairs should respect overridden thresholds."""
        events = []
        home_odds_a = [1.80, 1.85, 1.90, 1.95, 2.00, 2.05, 2.10, 2.15, 2.20, 2.25]
        home_odds_b = [1.82, 1.83, 1.95, 1.92, 2.01, 2.03, 2.08, 2.18, 2.19, 2.27]
        away_odds_a = [2.20, 2.15, 2.10, 2.05, 2.00, 1.95, 1.90, 1.85, 1.80, 1.75]
        away_odds_b = [2.18, 2.17, 2.05, 2.08, 1.99, 1.97, 1.92, 1.82, 1.81, 1.73]

        for i in range(10):
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_a", home_odds_a[i], away_odds_a[i]))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_b", home_odds_b[i], away_odds_b[i]))

        detector = MirrorLineDetector(mirror_threshold=0.999, independent_threshold=0.90)
        detector.compute_correlation_matrix(events)

        classifications = detector.classify_pairs()
        assert classifications[("bk_a", "bk_b")] == "unknown"

        classifications_strict = detector.classify_pairs(mirror_threshold=0.90)
        assert classifications_strict[("bk_a", "bk_b")] == "mirror"

    def test_correlation_matrix_return_type(self):
        events = []
        for i in range(5):
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_a", 1.5 + i * 0.1, 2.5 - i * 0.05))
            events.append(_make_event(f"T{i}", f"T{i+1}", "bk_b", 1.5 + i * 0.1 + 0.01, 2.5 - i * 0.05 + 0.01))

        detector = MirrorLineDetector(min_common_events=3)
        matrix = detector.compute_correlation_matrix(events)
        assert isinstance(matrix, dict)
        for key, value in matrix.items():
            assert isinstance(key, tuple)
            assert isinstance(value, float)
            assert -1.0 <= value <= 1.0
