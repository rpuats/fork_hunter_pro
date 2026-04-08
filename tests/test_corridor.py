# tests/test_corridor.py
"""
Tests for Corridor Finder module.
"""
import pytest
from core.corridor_finder import CorridorFinder, Corridor, CorridorScenario


class TestCorridorFinder:
    """Test suite for CorridorFinder."""

    @pytest.fixture
    def finder(self):
        """Create a CorridorFinder instance."""
        return CorridorFinder(min_ev=1.0)

    @pytest.fixture
    def sample_events_totals(self):
        """Create sample events with totals for corridor testing."""
        return [
            {
                'id': 'event_1',
                'bookmaker': 'winline',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'total_over_2.5': 1.95,
                'total_under_2.5': 1.90,
                'is_live': True,
            },
            {
                'id': 'event_2',
                'bookmaker': 'pari',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'total_over_2.5': 1.92,
                'total_under_3.5': 1.85,
                'is_live': True,
            },
            {
                'id': 'event_3',
                'bookmaker': 'fonbet',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'total_over_3.5': 2.10,
                'total_under_3.5': 1.75,
                'is_live': True,
            },
        ]

    @pytest.fixture
    def sample_events_handicaps(self):
        """Create sample events with handicaps for corridor testing."""
        return [
            {
                'id': 'event_1',
                'bookmaker': 'winline',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'f1_-1.5': 2.05,
                'f2_+2.5': 1.95,
                'is_live': True,
            },
            {
                'id': 'event_2',
                'bookmaker': 'pari',
                'home_team': 'Реал Мадрид',
                'away_team': 'Барселона',
                'sport': 'football',
                'f1_-1.0': 2.20,
                'f2_+1.5': 1.85,
                'is_live': True,
            },
        ]

    def test_find_totals_corridors(self, finder, sample_events_totals):
        """Test finding totals corridors."""
        corridors = finder.find_corridors(sample_events_totals, min_ev=0.5)
        assert isinstance(corridors, list)

    def test_find_handicap_corridors(self, finder, sample_events_handicaps):
        """Test finding handicap corridors."""
        corridors = finder.find_corridors(sample_events_handicaps, min_ev=0.5)
        assert isinstance(corridors, list)

    def test_find_corridors_empty_events(self, finder):
        """Test corridor finding with empty events."""
        corridors = finder.find_corridors([])
        assert corridors == []

    def test_find_corridors_filter_sport(self, finder):
        """Test corridor filtering by sport."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'basketball',
                'total_over_150.5': 1.95,
                'total_under_160.5': 1.90,
            }
        ]
        corridors = finder.find_corridors(events, sport='football')
        assert len(corridors) == 0

    def test_get_stats(self, finder, sample_events_totals):
        """Test getting finder stats."""
        finder.find_corridors(sample_events_totals)
        stats = finder.get_stats()
        assert 'total_found' in stats
        assert 'by_type' in stats
        assert 'min_ev_threshold' in stats

    def test_reset_stats(self, finder, sample_events_totals):
        """Test resetting stats."""
        finder.find_corridors(sample_events_totals)
        finder.reset_stats()
        stats = finder.get_stats()
        assert stats['total_found'] == 0


class TestCorridorScenario:
    """Test CorridorScenario dataclass."""

    def test_scenario_to_dict(self):
        """Test CorridorScenario to_dict method."""
        scenario = CorridorScenario(
            name="both_win",
            description="Both bets win",
            probability=0.25,
            profit_percent=15.0,
            both_win=True,
        )
        d = scenario.to_dict()
        assert d['name'] == 'both_win'
        assert d['probability'] == 0.25
        assert d['both_win'] is True


class TestCorridor:
    """Test Corridor dataclass."""

    def test_corridor_to_dict(self):
        """Test Corridor to_dict method."""
        scenario = CorridorScenario(
            name="both_win",
            description="Both win",
            probability=0.25,
            profit_percent=15.0,
        )
        corridor = Corridor(
            id='test_1',
            event_name='Team A vs Team B',
            sport='football',
            corridor_type='totals',
            markets=[{'selection': 'ТБ 2.5'}, {'selection': 'ТМ 3.5'}],
            odds=[1.95, 1.90],
            scenarios=[scenario],
            ev_percent=5.0,
        )
        d = corridor.to_dict()
        assert d['id'] == 'test_1'
        assert d['corridor_type'] == 'totals'
        assert len(d['scenarios']) == 1


class TestCorridorCalculation:
    """Test corridor calculation logic."""

    @pytest.fixture
    def finder(self):
        return CorridorFinder(min_ev=0)

    def test_calc_stakes(self, finder):
        """Test stake calculation."""
        odds = [2.00, 2.00]
        stakes = finder._calc_stakes(odds, 10000)
        assert len(stakes) == 2
        assert sum(stakes) == 10000

    def test_calc_stakes_different_odds(self, finder):
        """Test stake calculation with different odds."""
        odds = [1.90, 2.10]
        stakes = finder._calc_stakes(odds, 10000)
        assert len(stakes) == 2
        assert 4500 < stakes[0] < 5500
        assert 4500 < stakes[1] < 5500

    def test_calc_scenarios_total(self, finder):
        """Test total corridor scenario calculation."""
        scenarios, ev = finder._calc_scenarios_total(
            over_line=2.5,
            under_line=3.5,
            over_odds=1.95,
            under_odds=1.90,
            over_bk='bk1',
            under_bk='bk2',
        )
        assert len(scenarios) == 4
        assert any(s.name == 'both_win' for s in scenarios)
        assert any(s.name == 'over_only' for s in scenarios)
        assert any(s.name == 'under_only' for s in scenarios)
        assert any(s.name == 'both_lose' for s in scenarios)

    def test_calc_scenarios_handicap(self, finder):
        """Test handicap corridor scenario calculation."""
        scenarios, ev = finder._calc_scenarios_handicap(
            handicap1=-1.5,
            handicap2=2.5,
            odds1=2.05,
            odds2=1.95,
            bk1='bk1',
            bk2='bk2',
            selection1='Ф1 (-1.5)',
            selection2='Ф2 (+2.5)',
        )
        assert len(scenarios) == 4
        assert any(s.name == 'both_win' for s in scenarios)

    def test_make_id(self, finder):
        """Test corridor ID generation."""
        markets = [{'selection': 'ТБ 2.5'}, {'selection': 'ТМ 3.5'}]
        id1 = finder._make_id('Team A vs Team B', 'totals', markets)
        id2 = finder._make_id('Team A vs Team B', 'totals', markets)
        id3 = finder._make_id('Team A vs Team B', 'handicaps', markets)
        assert id1 == id2
        assert id1 != id3
        assert len(id1) == 8


class TestCorridorGrouping:
    """Test event grouping for corridors."""

    @pytest.fixture
    def finder(self):
        return CorridorFinder(min_ev=0)

    def test_group_events(self, finder):
        """Test event grouping by team names."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'total_over_2.5': 1.95,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'total_under_3.5': 1.90,
            },
            {
                'id': 'e3',
                'bookmaker': 'bk3',
                'home_team': 'Team C',
                'away_team': 'Team D',
                'sport': 'football',
                'total_over_2.5': 1.85,
            },
        ]
        grouped = finder._group_events(events)
        assert len(grouped) == 2
        assert any('team a' in key for key in grouped.keys())

    def test_group_events_normalizes_case(self, finder):
        """Test that event grouping normalizes case."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'TEAM A',
                'away_team': 'team b',
                'sport': 'football',
                'total_over_2.5': 1.95,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'team a',
                'away_team': 'Team B',
                'sport': 'football',
                'total_under_3.5': 1.90,
            },
        ]
        grouped = finder._group_events(events)
        assert len(grouped) == 1


class TestCorridorThresholds:
    """Test corridor EV thresholds."""

    @pytest.fixture
    def finder(self):
        return CorridorFinder(min_ev=1.0)

    def test_min_ev_filter(self, finder):
        """Test minimum EV filtering."""
        events = [
            {
                'id': 'e1',
                'bookmaker': 'bk1',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'total_over_2.5': 1.95,
            },
            {
                'id': 'e2',
                'bookmaker': 'bk2',
                'home_team': 'Team A',
                'away_team': 'Team B',
                'sport': 'football',
                'total_under_3.5': 1.90,
            },
        ]
        corridors = finder.find_corridors(events, min_ev=5.0)
        assert isinstance(corridors, list)
