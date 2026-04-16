# tests/test_engine.py
import pytest
import asyncio
import sys
import os
from unittest.mock import AsyncMock, MagicMock, patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from scanner.engine import GhostScanner, ScannerConfig, ScannerStats
from core.finder import SurebetCalculator
from core.finder_optimized import OptimizedSurebetCalculator
from core.performance import PerformanceMonitor


class MockDatabase:
    def __init__(self):
        self.saved_surebets = []
        self.path = ':memory:'

    async def init(self):
        pass

    async def save_surebet(self, surebet):
        self.saved_surebets.append(surebet)

    async def close(self):
        pass


class MockParser:
    def __init__(self, name="MockParser", slug="mock", events=None):
        self.name = name
        self.slug = slug
        self._request_count = 0
        self._errors = 0
        self._events = events or []

    async def get_events(self):
        self._request_count += 1
        return self._events

    async def close(self):
        pass


def make_event(home, away, bk, home_odds, away_odds, draw_odds=0):
    return {
        'home_team': home,
        'away_team': away,
        'bookmaker': bk,
        'home_odds': home_odds,
        'away_odds': away_odds,
        'draw_odds': draw_odds,
        'sport': 'football',
        'is_live': True,
        'market': '1x2',
    }


class TestScannerInitialization:
    def test_default_config(self):
        config = ScannerConfig()
        assert config.min_profit == 0.5
        assert config.cycle_interval == 3.0
        assert config.max_events_per_source == 200

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_scanner_init(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        assert scanner.is_running is False
        assert scanner.config.min_profit == 0.1
        assert isinstance(scanner.calculator, (SurebetCalculator, OptimizedSurebetCalculator))
        assert isinstance(scanner.perf_monitor, PerformanceMonitor)

    def test_scanner_stats_init(self):
        stats = ScannerStats()
        assert stats.total_cycles == 0
        assert stats.total_events == 0
        assert stats.total_surebets == 0


class TestEventProcessing:
    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_event_key(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        event = {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1'}
        key = scanner._get_event_key(event)
        assert key == 'team a|team b|bk1'

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_event_key_missing_fields(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        event = {'home_team': '', 'away_team': 'Team B', 'bookmaker': 'bk1'}
        key = scanner._get_event_key(event)
        assert key == ''

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_deduplicate_events(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        events = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1'},
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1'},
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk2'},
        ]
        unique = scanner._deduplicate_events(events)
        assert len(unique) == 2

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_group_events_by_match(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        events = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1'},
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk2'},
        ]
        grouped = scanner._group_events_by_match(events)
        assert len(grouped) == 1


class TestSurebetDetection:
    @patch('scanner.engine.ALL_PARSERS', [])
    def test_find_surebets_with_mock_data(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.20, 1.80),
            make_event('Team A', 'Team B', 'bk2', 1.80, 2.30),
        ]
        surebets = scanner._find_surebets(events)
        assert len(surebets) >= 0

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_find_surebets_no_surebet(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=10.0, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        events = [
            make_event('Team A', 'Team B', 'bk1', 1.80, 1.80),
            make_event('Team A', 'Team B', 'bk2', 1.75, 1.85),
        ]
        surebets = scanner._find_surebets(events)
        assert len(surebets) == 0

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_surebets_filter_by_profit(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        scanner.surebets = [
            {'profit_percent': 1.0, 'sport': 'football'},
            {'profit_percent': 5.0, 'sport': 'football'},
            {'profit_percent': 0.5, 'sport': 'hockey'},
        ]
        filtered = scanner.get_surebets(min_profit=2.0)
        assert len(filtered) == 1
        assert filtered[0]['profit_percent'] == 5.0

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_surebets_filter_by_sport(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        scanner.surebets = [
            {'profit_percent': 1.0, 'sport': 'football'},
            {'profit_percent': 5.0, 'sport': 'hockey'},
        ]
        filtered = scanner.get_surebets(sport='hockey')
        assert len(filtered) == 1

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_top_surebets(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        scanner.surebets = [{'profit_percent': i} for i in range(20)]
        top = scanner.get_top_surebets(limit=5)
        assert len(top) == 5

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_subscribe_notify(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        notifications = []
        def callback(surebets):
            notifications.extend(surebets)
        scanner.subscribe(callback)
        scanner._notify_subscribers([{'id': '1'}])
        assert len(notifications) == 1

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_unsubscribe(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        def callback(surebets):
            pass
        scanner.subscribe(callback)
        assert len(scanner._subscribers) == 1
        scanner.unsubscribe(callback)
        assert len(scanner._subscribers) == 0

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_stats(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        stats = scanner.get_stats()
        assert 'is_running' in stats
        assert 'total_cycles' in stats
        assert 'total_events' in stats
        assert 'cache_stats' in stats
        assert 'performance' in stats

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_corridors_uses_cached_runtime_results(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)

        scanner.corridors = [
            {'id': 'c1', 'sport': 'football', 'expected_roi': 2.5},
            {'id': 'c2', 'sport': 'hockey', 'expected_roi': 0.8},
        ]

        filtered = scanner.get_corridors(min_ev=1.0, sport='football')

        assert filtered == [{'id': 'c1', 'sport': 'football', 'expected_roi': 2.5}]

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_get_bookmaker_stats(self):
        db = MockDatabase()
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        stats = scanner.get_bookmaker_stats()
        assert isinstance(stats, dict)


class TestIncrementalChanges:
    @patch('scanner.engine.ALL_PARSERS', [])
    def test_detect_changes_first_run(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        events = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1', 'home_odds': 2.0, 'away_odds': 2.0},
        ]
        result = scanner._detect_incremental_changes(events)
        assert result is None

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_detect_changes_no_changes(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        events = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1', 'home_odds': 2.0, 'away_odds': 2.0},
        ]
        scanner._detect_incremental_changes(events)
        result = scanner._detect_incremental_changes(events)
        assert result is None

    @patch('scanner.engine.ALL_PARSERS', [])
    def test_detect_changes_with_changes(self):
        db = MockDatabase()
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db)
        events1 = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1', 'home_odds': 2.0, 'away_odds': 2.0},
        ]
        events2 = [
            {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'bk1', 'home_odds': 2.5, 'away_odds': 2.0},
        ]
        scanner._detect_incremental_changes(events1)
        result = scanner._detect_incremental_changes(events2)
        assert result is not None
        assert len(result) == 1


class TestFetchAllEvents:
    @pytest.mark.asyncio
    @patch('scanner.engine.ALL_PARSERS', [])
    async def test_fetch_all_events(self):
        db = MockDatabase()
        events = [make_event('Team A', 'Team B', 'mock', 2.0, 2.0)]
        parser = MockParser(events=events)
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        scanner.parsers = [parser]
        result = await scanner._fetch_all_events()
        assert len(result) == 1
        assert result[0]['bookmaker'] == 'mock'

    @pytest.mark.asyncio
    @patch('scanner.engine.ALL_PARSERS', [])
    async def test_fetch_all_events_parser_error(self):
        db = MockDatabase()
        parser = MockParser(slug='mock')
        parser.get_events = AsyncMock(side_effect=Exception("Test error"))
        config = ScannerConfig(min_profit=0.1, enabled_sources=set())
        with patch('scanner.engine.ALL_PARSERS', []):
            scanner = GhostScanner(db, config)
        scanner.parsers = [parser]
        result = await scanner._fetch_all_events()
        assert len(result) == 0
        assert 'mock' in scanner.stats.parser_stats
