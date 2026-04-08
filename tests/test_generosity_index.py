# tests/test_generosity_index.py
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.generosity_index import BookmakerGenerosityIndex


class TestGenerosityIndexCalculation:
    def test_basic_generosity_calculation(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        result = index.calculate_index(events)
        assert 'bk1' in result
        assert 'bk2' in result
        assert 'football' in result['bk1']
        assert 'football' in result['bk2']

    def test_generous_bookmaker_detected(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'generous_bk',
                'home_odds': 2.20,
                'away_odds': 2.10,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'stingy_bk',
                'home_odds': 1.90,
                'away_odds': 1.80,
                'sport': 'football',
            },
        ]
        result = index.calculate_index(events)
        assert result['generous_bk']['football'] > 0
        assert result['stingy_bk']['football'] < 0

    def test_single_bookmaker_no_index(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'only_bk',
                'home_odds': 2.00,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        result = index.calculate_index(events)
        assert 'only_bk' not in result

    def test_empty_events(self):
        index = BookmakerGenerosityIndex()
        result = index.calculate_index([])
        assert result == {}

    def test_invalid_odds_excluded(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 0,
                'away_odds': 2.00,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 2.00,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        result = index.calculate_index(events)
        assert len(result) == 0

    def test_multiple_sports(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
            {
                'home_team': 'Player X',
                'away_team': 'Player Y',
                'bookmaker': 'bk1',
                'home_odds': 1.80,
                'away_odds': 2.20,
                'sport': 'tennis',
            },
            {
                'home_team': 'Player X',
                'away_team': 'Player Y',
                'bookmaker': 'bk2',
                'home_odds': 1.70,
                'away_odds': 2.30,
                'sport': 'tennis',
            },
        ]
        result = index.calculate_index(events)
        assert 'football' in result['bk1']
        assert 'tennis' in result['bk1']


class TestGenerosityRanking:
    def test_ranking_order(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'best_bk',
                'home_odds': 2.30,
                'away_odds': 2.20,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'mid_bk',
                'home_odds': 2.00,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'worst_bk',
                'home_odds': 1.80,
                'away_odds': 1.70,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        ranking = index.get_ranking()
        assert len(ranking) == 3
        assert ranking[0][0] == 'best_bk'
        assert ranking[-1][0] == 'worst_bk'

    def test_ranking_by_sport(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        ranking = index.get_ranking(sport='football')
        assert len(ranking) == 2
        assert ranking[0][0] == 'bk1'

    def test_empty_ranking(self):
        index = BookmakerGenerosityIndex()
        ranking = index.get_ranking()
        assert ranking == []

    def test_sport_not_found(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        ranking = index.get_ranking(sport='basketball')
        assert ranking == []


class TestBestForSport:
    def test_get_best_for_sport(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'generous_bk',
                'home_odds': 2.20,
                'away_odds': 2.10,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'stingy_bk',
                'home_odds': 1.90,
                'away_odds': 1.80,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        best = index.get_best_for_sport('football')
        assert best == 'generous_bk'

    def test_no_data_for_sport(self):
        index = BookmakerGenerosityIndex()
        best = index.get_best_for_sport('hockey')
        assert best is None


class TestGenerositySummary:
    def test_summary_structure(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        summary = index.get_summary()
        assert 'ranking' in summary
        assert 'sport_best' in summary
        assert 'total_bookmakers' in summary
        assert 'total_samples' in summary
        assert 'sports_tracked' in summary
        assert summary['total_bookmakers'] == 2
        assert 'football' in summary['sports_tracked']


class TestGenerosityReset:
    def test_reset_clears_data(self):
        index = BookmakerGenerosityIndex()
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.00,
                'sport': 'football',
            },
        ]
        index.calculate_index(events)
        assert len(index.get_ranking()) > 0
        index.reset()
        assert len(index.get_ranking()) == 0
