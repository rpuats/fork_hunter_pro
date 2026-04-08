# tests/test_value_detector.py
"""
Tests for Value Bet Detector module.
"""
import pytest
from core.value_detector import ValueBetDetector, ValueBet


class TestValueBetDetector:
    """Test suite for ValueBetDetector."""

    @pytest.fixture
    def detector(self):
        """Create a ValueBetDetector instance."""
        return ValueBetDetector(min_edge=2.0)

    @pytest.fixture
    def sample_events(self):
        """Create sample events for testing."""
        return [
            {
                'id': 'event_1',
                'bookmaker': 'winline',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'home_odds': 2.10,
                'away_odds': 3.50,
                'draw_odds': 3.20,
                'is_live': True,
            },
            {
                'id': 'event_2',
                'bookmaker': 'pari',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'home_odds': 2.05,
                'away_odds': 3.60,
                'draw_odds': 3.10,
                'is_live': True,
            },
            {
                'id': 'event_3',
                'bookmaker': 'fonbet',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'home_odds': 2.15,
                'away_odds': 3.40,
                'draw_odds': 3.25,
                'is_live': True,
            },
        ]

    def test_calculate_fair_odds(self, detector):
        """Test fair odds calculation."""
        odds = [2.0, 2.1, 2.05]
        fair = detector.calculate_fair_odds(odds)
        assert fair > 0
        assert 2.0 < fair < 2.2

    def test_calculate_fair_odds_empty(self, detector):
        """Test fair odds with empty list."""
        fair = detector.calculate_fair_odds([])
        assert fair == 0.0

    def test_calculate_fair_odds_single(self, detector):
        """Test fair odds with single value."""
        fair = detector.calculate_fair_odds([2.0])
        assert fair == 0.0

    def test_calculate_fair_odds_below_threshold(self, detector):
        """Test fair odds with values below threshold."""
        fair = detector.calculate_fair_odds([1.01, 1.0])
        assert fair == 0.0

    def test_calculate_edge_positive(self, detector):
        """Test edge calculation with positive edge."""
        edge = detector.calculate_edge(bookmaker_odds=2.20, fair_odds=2.00)
        assert edge == pytest.approx(0.10)

    def test_calculate_edge_negative(self, detector):
        """Test edge calculation with negative edge."""
        edge = detector.calculate_edge(bookmaker_odds=1.90, fair_odds=2.00)
        assert edge == pytest.approx(-0.05)

    def test_calculate_edge_zero(self, detector):
        """Test edge calculation with zero values."""
        edge = detector.calculate_edge(bookmaker_odds=0, fair_odds=2.00)
        assert edge == 0.0

    def test_find_value_bets(self, detector, sample_events):
        """Test finding value bets."""
        value_bets = detector.find_value_bets(sample_events, min_edge=2.0)
        assert isinstance(value_bets, list)

    def test_find_value_bets_filter_sport(self, detector, sample_events):
        """Test value bet filtering by sport."""
        value_bets = detector.find_value_bets(sample_events, sport='basketball')
        assert len(value_bets) == 0

    def test_find_value_bets_filter_bookmaker(self, detector, sample_events):
        """Test value bet filtering by bookmaker."""
        value_bets = detector.find_value_bets(sample_events, bookmaker='winline')
        assert all(vb['bookmaker'] == 'winline' for vb in value_bets)

    def test_find_value_bets_min_edge(self, detector):
        """Test value bet filtering by minimum edge."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.50,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.00,
            },
            {
                'id': 'e3',
                'bookmaker': 'bk3',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 1.95,
            },
        ]
        value_bets = detector.find_value_bets(events, min_edge=10.0)
        assert all(vb['edge_percent'] >= 10 for vb in value_bets)

    def test_value_bet_to_dict(self):
        """Test ValueBet to_dict method."""
        vb = ValueBet(
            id='test_1',
            event_name='Team A vs Team B',
            sport='football',
            bookmaker='winline',
            market='2-way',
            selection='П1',
            bookmaker_odds=2.20,
            fair_odds=2.00,
            edge_percent=10.0,
            implied_probability=0.4545,
            fair_probability=0.50,
        )
        d = vb.to_dict()
        assert d['id'] == 'test_1'
        assert d['bookmaker_odds'] == 2.20
        assert d['edge_percent'] == 10.0

    def test_get_stats(self, detector, sample_events):
        """Test getting detector stats."""
        detector.find_value_bets(sample_events)
        stats = detector.get_stats()
        assert 'total_events_scanned' in stats
        assert 'min_edge_threshold' in stats

    def test_reset_stats(self, detector, sample_events):
        """Test resetting stats."""
        detector.find_value_bets(sample_events)
        detector.reset_stats()
        stats = detector.get_stats()
        assert stats['total_events_scanned'] == 0


class TestValueBetEdgeCalculation:
    """Test edge calculation edge cases."""

    @pytest.fixture
    def detector(self):
        return ValueBetDetector(min_edge=0)

    def test_edge_5_percent(self, detector):
        """Test 5% edge."""
        edge = detector.calculate_edge(2.10, 2.00)
        assert 0.049 < edge < 0.051

    def test_edge_10_percent(self, detector):
        """Test 10% edge."""
        edge = detector.calculate_edge(2.20, 2.00)
        assert edge == pytest.approx(0.10)

    def test_edge_2_percent(self, detector):
        """Test 2% edge."""
        edge = detector.calculate_edge(2.04, 2.00)
        assert 0.019 < edge < 0.021

    def test_fair_odds_multiple_bookmakers(self, detector):
        """Test fair odds with multiple bookmakers."""
        odds = [2.00, 2.10, 2.05, 2.15, 2.00]
        fair = detector.calculate_fair_odds(odds)
        assert 1.9 < fair < 2.2


class TestValueBetMarketExtraction:
    """Test market outcome extraction."""

    @pytest.fixture
    def detector(self):
        return ValueBetDetector(min_edge=0)

    def test_extract_2way_outcomes(self, detector):
        """Test 2-way outcome extraction."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.00,
                'away_odds': 1.90,
            }
        ]
        outcomes = detector._extract_outcomes_2way(events)
        assert len(outcomes) == 2
        assert any(o['outcome_key'] == 'home' for o in outcomes)
        assert any(o['outcome_key'] == 'away' for o in outcomes)

    def test_extract_3way_outcomes(self, detector):
        """Test 3-way outcome extraction."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.00,
                'draw_odds': 3.20,
                'away_odds': 3.50,
            }
        ]
        outcomes = detector._extract_outcomes_3way(events)
        assert len(outcomes) == 3
        assert any(o['outcome_key'] == 'home' for o in outcomes)
        assert any(o['outcome_key'] == 'draw' for o in outcomes)
        assert any(o['outcome_key'] == 'away' for o in outcomes)

    def test_extract_total_outcomes(self, detector):
        """Test total outcome extraction."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'total_over_2.5': 1.95,
                'total_under_2.5': 1.90,
            }
        ]
        outcomes = detector._extract_total_outcomes(events)
        assert len(outcomes) >= 2


class TestValueBetGrouping:
    """Test event grouping."""

    @pytest.fixture
    def detector(self):
        return ValueBetDetector(min_edge=0)

    def test_group_events(self, detector):
        """Test event grouping by team names."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.00,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.10,
            },
            {
                'id': 'e3',
                'bookmaker': 'bk3',
                'home_team': 'Team C',
                'away_team': 'Team D',
                'sport': 'football',
                'home_odds': 1.90,
            },
        ]
        grouped = detector._group_events(events)
        assert len(grouped) == 2
        assert any('team a' in key for key in grouped.keys())
        assert any('team c' in key for key in grouped.keys())

    def test_group_events_normalizes_case(self, detector):
        """Test that event grouping normalizes case."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'TEAM A',
                'away_team': 'team b',
                'sport': 'football',
                'home_odds': 2.00,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'team a',
                'away_team': 'Team B',
                'sport': 'football',
                'home_odds': 2.10,
            },
        ]
        grouped = detector._group_events(events)
        assert len(grouped) == 1
