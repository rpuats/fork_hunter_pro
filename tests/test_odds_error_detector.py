# tests/test_odds_error_detector.py
"""
Tests for OddsErrorDetector (Идея #23)
Tests anomalous odds detection, market average computation, scoring, and error reporting.
"""
import pytest
from core.odds_error_detector import OddsErrorDetector, Anomaly


def _make_event(home, away, home_odds, draw_odds, away_odds, bk, sport="football", is_live=False):
    return {
        "home_team": home,
        "away_team": away,
        "home_odds": home_odds,
        "draw_odds": draw_odds,
        "away_odds": away_odds,
        "bookmaker": bk,
        "sport": sport,
        "is_live": is_live,
    }


class TestComputeMarketAverage:
    def test_basic_market_average(self):
        detector = OddsErrorDetector()
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.2, 3.2, 3.8, "bk2"),
            _make_event("A", "B", 1.8, 2.8, 4.2, "bk3"),
        ]
        result = detector.compute_market_average(events, "a|b")
        assert result["home"] == pytest.approx(2.0)
        assert result["draw"] == pytest.approx(3.0)
        assert result["away"] == pytest.approx(4.0)

    def test_market_average_with_alternate_keys(self):
        detector = OddsErrorDetector()
        events = [
            {"home_team": "A", "away_team": "B", "p1": 2.0, "x": 3.0, "p2": 4.0, "bookmaker": "bk1"},
            {"home_team": "A", "away_team": "B", "p1": 2.2, "x": 3.2, "p2": 3.8, "bookmaker": "bk2"},
        ]
        result = detector.compute_market_average(events, "a|b")
        assert result["home"] == pytest.approx(2.1)
        assert result["draw"] == pytest.approx(3.1)
        assert result["away"] == pytest.approx(3.9)

    def test_market_average_empty_match(self):
        detector = OddsErrorDetector()
        events = [_make_event("A", "B", 2.0, 3.0, 4.0, "bk1")]
        result = detector.compute_market_average(events, "x|y")
        assert result == {}

    def test_market_average_missing_draw(self):
        detector = OddsErrorDetector()
        events = [
            {"home_team": "A", "away_team": "B", "home_odds": 2.0, "away_odds": 4.0, "bookmaker": "bk1"},
            {"home_team": "A", "away_team": "B", "home_odds": 2.2, "away_odds": 3.8, "bookmaker": "bk2"},
        ]
        result = detector.compute_market_average(events, "a|b")
        assert "home" in result
        assert "away" in result
        assert "draw" not in result

    def test_market_average_ignores_invalid_odds(self):
        detector = OddsErrorDetector()
        events = [
            {"home_team": "A", "away_team": "B", "home_odds": 2.0, "away_odds": 4.0, "bookmaker": "bk1"},
            {"home_team": "A", "away_team": "B", "home_odds": 0.5, "away_odds": -1.0, "bookmaker": "bk2"},
            {"home_team": "A", "away_team": "B", "home_odds": 2.2, "away_odds": 3.8, "bookmaker": "bk3"},
        ]
        result = detector.compute_market_average(events, "a|b")
        assert result["home"] == pytest.approx(2.1)
        assert result["away"] == pytest.approx(3.9)


class TestDetectAnomalies:
    def test_detects_clear_anomaly(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.1, 3.1, 3.9, "bk2"),
            _make_event("A", "B", 1.9, 2.9, 4.1, "bk3"),
            _make_event("A", "B", 3.5, 3.0, 4.0, "bk4"),
        ]
        anomalies = detector.detect_anomalies(events, threshold=0.25)
        home_anomalies = [a for a in anomalies if a.selection == "home" and a.bookmaker == "bk4"]
        assert len(home_anomalies) >= 1

    def test_no_anomalies_when_all_similar(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.05, 3.05, 3.95, "bk2"),
            _make_event("A", "B", 1.95, 2.95, 4.05, "bk3"),
        ]
        anomalies = detector.detect_anomalies(events, threshold=0.25)
        assert len(anomalies) == 0

    def test_respects_threshold(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 2.3, 3.0, 4.0, "bk4"),
        ]
        anomalies_strict = detector.detect_anomalies(events, threshold=0.10)
        anomalies_loose = detector.detect_anomalies(events, threshold=0.25)
        assert len(anomalies_strict) >= len(anomalies_loose)

    def test_requires_min_bookmakers(self):
        detector = OddsErrorDetector(min_bookmakers=5)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 5.0, 3.0, 4.0, "bk3"),
        ]
        anomalies = detector.detect_anomalies(events)
        assert len(anomalies) == 0

    def test_detects_multiple_anomalies_same_match(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 3.5, 5.0, 6.0, "bk4"),
        ]
        anomalies = detector.detect_anomalies(events, threshold=0.20)
        assert len(anomalies) >= 1

    def test_anomaly_sorted_by_deviation(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 3.0, 3.0, 4.0, "bk4"),
            _make_event("A", "B", 5.0, 3.0, 4.0, "bk5"),
        ]
        anomalies = detector.detect_anomalies(events, threshold=0.15)
        if len(anomalies) >= 2:
            assert anomalies[0].deviation_percent >= anomalies[1].deviation_percent

    def test_handles_empty_events(self):
        detector = OddsErrorDetector()
        anomalies = detector.detect_anomalies([])
        assert anomalies == []

    def test_handles_single_event(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [_make_event("A", "B", 2.0, 3.0, 4.0, "bk1")]
        anomalies = detector.detect_anomalies(events)
        assert anomalies == []


class TestScoreAnomaly:
    def test_high_deviation_high_score(self):
        detector = OddsErrorDetector()
        anomaly = Anomaly(
            event_name="A vs B",
            sport="football",
            bookmaker="bk1",
            selection="home",
            anomalous_odds=5.0,
            market_average=2.0,
            deviation_percent=150.0,
            num_bookmakers=7,
            is_live=True,
        )
        score = detector.score_anomaly(anomaly)
        assert score >= 70

    def test_low_deviation_low_score(self):
        detector = OddsErrorDetector()
        anomaly = Anomaly(
            event_name="A vs B",
            sport="football",
            bookmaker="bk1",
            selection="home",
            anomalous_odds=2.1,
            market_average=2.0,
            deviation_percent=5.0,
            num_bookmakers=3,
            is_live=False,
        )
        score = detector.score_anomaly(anomaly)
        assert score <= 30

    def test_score_capped_at_100(self):
        detector = OddsErrorDetector()
        anomaly = Anomaly(
            event_name="A vs B",
            sport="football",
            bookmaker="bk1",
            selection="home",
            anomalous_odds=10.0,
            market_average=1.5,
            deviation_percent=500.0,
            num_bookmakers=7,
            is_live=True,
        )
        score = detector.score_anomaly(anomaly)
        assert score <= 100.0

    def test_live_bonus(self):
        detector = OddsErrorDetector()
        live_anomaly = Anomaly(
            event_name="A vs B", sport="football", bookmaker="bk1",
            selection="home", anomalous_odds=3.0, market_average=2.0,
            deviation_percent=50.0, num_bookmakers=5, is_live=True,
        )
        prematch_anomaly = Anomaly(
            event_name="A vs B", sport="football", bookmaker="bk1",
            selection="home", anomalous_odds=3.0, market_average=2.0,
            deviation_percent=50.0, num_bookmakers=5, is_live=False,
        )
        live_score = detector.score_anomaly(live_anomaly)
        prematch_score = detector.score_anomaly(prematch_anomaly)
        assert live_score > prematch_score

    def test_more_bookmakers_higher_score(self):
        detector = OddsErrorDetector()
        anomaly_3 = Anomaly(
            event_name="A vs B", sport="football", bookmaker="bk1",
            selection="home", anomalous_odds=3.0, market_average=2.0,
            deviation_percent=50.0, num_bookmakers=3, is_live=False,
        )
        anomaly_7 = Anomaly(
            event_name="A vs B", sport="football", bookmaker="bk1",
            selection="home", anomalous_odds=3.0, market_average=2.0,
            deviation_percent=50.0, num_bookmakers=7, is_live=False,
        )
        score_3 = detector.score_anomaly(anomaly_3)
        score_7 = detector.score_anomaly(anomaly_7)
        assert score_7 > score_3


class TestGetErrors:
    def test_returns_errors_above_min_score(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 4.0, 3.0, 4.0, "bk4"),
        ]
        errors = detector.get_errors(events, threshold=0.20, min_score=0)
        assert len(errors) >= 1

    def test_error_has_required_fields(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 4.0, 3.0, 4.0, "bk4"),
        ]
        errors = detector.get_errors(events, threshold=0.20, min_score=0)
        if errors:
            err = errors[0]
            assert "event_name" in err
            assert "bookmaker" in err
            assert "anomalous_odds" in err
            assert "market_average" in err
            assert "score" in err
            assert "recommended_stake_pct" in err
            assert "expected_value" in err
            assert "confidence" in err
            assert "action" in err

    def test_errors_sorted_by_score(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 3.0, 5.0, 6.0, "bk4"),
        ]
        errors = detector.get_errors(events, threshold=0.15, min_score=0)
        if len(errors) >= 2:
            assert errors[0]["score"] >= errors[1]["score"]

    def test_filters_by_min_score(self):
        detector = OddsErrorDetector(min_bookmakers=3)
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk2"),
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk3"),
            _make_event("A", "B", 2.3, 3.0, 4.0, "bk4"),
        ]
        errors_low = detector.get_errors(events, threshold=0.10, min_score=0)
        errors_high = detector.get_errors(events, threshold=0.10, min_score=80)
        assert len(errors_low) >= len(errors_high)

    def test_empty_events_returns_empty(self):
        detector = OddsErrorDetector()
        errors = detector.get_errors([])
        assert errors == []


class TestStats:
    def test_get_stats(self):
        detector = OddsErrorDetector(default_threshold=0.30, min_bookmakers=4)
        stats = detector.get_stats()
        assert stats["default_threshold"] == 0.30
        assert stats["min_bookmakers"] == 4
        assert "total_detections" in stats


class TestMarketStats:
    def test_compute_market_stats(self):
        detector = OddsErrorDetector()
        events = [
            _make_event("A", "B", 2.0, 3.0, 4.0, "bk1"),
            _make_event("A", "B", 2.2, 3.2, 3.8, "bk2"),
            _make_event("A", "B", 1.8, 2.8, 4.2, "bk3"),
        ]
        stats = detector.compute_market_stats(events, "a|b")
        assert "home" in stats
        assert stats["home"]["mean"] == pytest.approx(2.0)
        assert stats["home"]["count"] == 3
        assert stats["home"]["min"] == 1.8
        assert stats["home"]["max"] == 2.2
