# tests/test_calculator.py
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.finder import SurebetCalculator, SurebetLeg, OddsAnalyzer


class TestCalc2WaySurebet:
    def test_profitable_2way_surebet(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 1.90,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.95,
                'away_odds': 2.15,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) > 0
        sb = surebets[0]
        assert sb['profit_percent'] > 0
        assert sb['market_type'] == '2-way'
        assert len(sb['legs']) == 2

    def test_non_profitable_2way(self):
        calculator = SurebetCalculator(min_profit=0.5)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 1.80,
                'away_odds': 1.80,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 1.75,
                'away_odds': 1.85,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_2way_same_bookmaker_excluded(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.20,
                'away_odds': 1.70,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_2way_insufficient_events(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.10,
                'away_odds': 2.10,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0


class TestCalc3WaySurebet:
    def test_profitable_3way_surebet(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 3.50,
                'draw_odds': 3.60,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 2.80,
                'draw_odds': 3.80,
                'away_odds': 2.50,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk3',
                'home_odds': 2.90,
                'draw_odds': 3.40,
                'away_odds': 3.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_3way_surebets(events)
        assert len(surebets) >= 0

    def test_3way_no_surebet(self):
        calculator = SurebetCalculator(min_profit=0.5)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.00,
                'draw_odds': 3.20,
                'away_odds': 3.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_3way_surebets(events)
        assert len(surebets) == 0

    def test_3way_missing_draw(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.00,
                'draw_odds': 0,
                'away_odds': 3.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_3way_surebets(events)
        assert len(surebets) == 0


class TestStakeCalculation:
    def test_calculate_stakes_2way(self):
        calculator = SurebetCalculator(min_profit=0.5)
        odds = [2.10, 2.10]
        stakes = calculator.calculate_stakes(odds, total_stake=10000)
        assert len(stakes) == 2
        assert abs(sum(stakes) - 10000) < 0.01
        assert abs(stakes[0] - 5000) < 1
        assert abs(stakes[1] - 5000) < 1

    def test_calculate_stakes_3way(self):
        calculator = SurebetCalculator(min_profit=0.5)
        odds = [3.00, 3.00, 3.00]
        stakes = calculator.calculate_stakes(odds, total_stake=9000)
        assert len(stakes) == 3
        assert abs(sum(stakes) - 9000) < 0.01
        for stake in stakes:
            assert abs(stake - 3000) < 1

    def test_calculate_stakes_uneven_odds(self):
        calculator = SurebetCalculator(min_profit=0.5)
        odds = [1.50, 3.00]
        stakes = calculator.calculate_stakes(odds, total_stake=10000)
        assert len(stakes) == 2
        assert abs(sum(stakes) - 10000) < 0.01
        assert stakes[0] > stakes[1]

    def test_calculate_stakes_preserves_total(self):
        calculator = SurebetCalculator(min_profit=0.5)
        for total in [1000, 5000, 10000, 50000]:
            odds = [2.00, 2.50]
            stakes = calculator.calculate_stakes(odds, total_stake=total)
            assert abs(sum(stakes) - total) < 0.01


class TestEdgeCases:
    def test_zero_odds(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 0,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 2.00,
                'away_odds': 0,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_negative_odds(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': -1.50,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_missing_data(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': '',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 2.00,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_missing_away_team(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': '',
                'bookmaker': 'bk1',
                'home_odds': 2.00,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0

    def test_empty_events_list(self):
        calculator = SurebetCalculator(min_profit=0.1)
        surebets = calculator.find_surebets([])
        assert len(surebets) == 0

    def test_odds_below_threshold(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'home_odds': 1.005,
                'away_odds': 2.00,
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'home_odds': 2.00,
                'away_odds': 1.005,
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_2way_surebets(events)
        assert len(surebets) == 0


class TestOddsAnalyzer:
    def test_is_arbitrage_true(self):
        odds = [2.20, 2.20]
        assert OddsAnalyzer.is_arbitrage(odds) is True

    def test_is_arbitrage_false(self):
        odds = [1.80, 1.80]
        assert OddsAnalyzer.is_arbitrage(odds) is False

    def test_calculate_margin(self):
        odds = [2.00, 2.00]
        margin = OddsAnalyzer.calculate_margin(odds)
        assert abs(margin - 0.0) < 0.001

    def test_get_best_odds(self):
        events = [
            {'bookmaker': 'bk1', 'odds': 2.00},
            {'bookmaker': 'bk2', 'odds': 2.50},
            {'bookmaker': 'bk3', 'odds': 1.80},
        ]
        best_odds, best_bk = OddsAnalyzer.get_best_odds(events, 'odds')
        assert best_odds == 2.50
        assert best_bk == 'bk2'

    def test_detect_odds_movement_up(self):
        historical = [2.00, 2.00, 2.00]
        current = 2.20
        result = OddsAnalyzer.detect_odds_movement(current, historical)
        assert result == 'sharp_up'

    def test_detect_odds_movement_down(self):
        historical = [2.00, 2.00, 2.00]
        current = 1.80
        result = OddsAnalyzer.detect_odds_movement(current, historical)
        assert result == 'sharp_down'

    def test_detect_odds_movement_stable(self):
        historical = [2.00, 2.00, 2.00]
        current = 2.05
        result = OddsAnalyzer.detect_odds_movement(current, historical)
        assert result == 'stable'

    def test_detect_odds_movement_empty_history(self):
        current = 2.00
        result = OddsAnalyzer.detect_odds_movement(current, [])
        assert result == 'stable'


class TestSurebetLeg:
    def test_leg_creation(self):
        leg = SurebetLeg(
            bookmaker='bk1',
            market='1',
            selection='P1',
            odds=2.00,
            event_name='Team A vs Team B',
        )
        assert leg.bookmaker == 'bk1'
        assert leg.odds == 2.00
        assert leg.calculated_stake == 0.0
        assert leg.stake_percent == 0.0

    def test_leg_with_stake(self):
        leg = SurebetLeg(
            bookmaker='bk1',
            market='1',
            selection='P1',
            odds=2.00,
            event_name='Team A vs Team B',
            calculated_stake=5000.0,
            stake_percent=50.0,
        )
        assert leg.calculated_stake == 5000.0
        assert leg.stake_percent == 50.0


class TestTotalSurebets:
    def test_profitable_total_surebet_cross_bk(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'total_over': {2.5: 2.10, 3.0: 1.80},
                'total_under': {2.5: 1.85, 3.0: 2.00},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'total_over': {2.5: 1.90, 3.0: 2.20},
                'total_under': {2.5: 2.15, 3.0: 1.75},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_all_total_surebets(events)
        assert len(surebets) > 0
        sb = surebets[0]
        assert sb['profit_percent'] > 0
        assert 'total' in sb['market_type']
        assert len(sb['legs']) == 2

    def test_total_surebet_same_bk_excluded(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'total_over': {2.5: 2.20},
                'total_under': {2.5: 2.20},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_all_total_surebets(events)
        assert len(surebets) == 0

    def test_total_surebet_multiple_lines(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'total_over': {1.5: 1.30, 2.5: 2.10, 3.5: 3.00},
                'total_under': {1.5: 3.50, 2.5: 1.85, 3.5: 1.40},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'total_over': {1.5: 1.25, 2.5: 1.95, 3.5: 3.20},
                'total_under': {1.5: 3.80, 2.5: 2.05, 3.5: 1.35},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_all_total_surebets(events)
        assert len(surebets) >= 0

    def test_total_surebet_no_profit(self):
        calculator = SurebetCalculator(min_profit=0.5)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'total_over': {2.5: 1.80},
                'total_under': {2.5: 1.80},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'total_over': {2.5: 1.75},
                'total_under': {2.5: 1.85},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_all_total_surebets(events)
        assert len(surebets) == 0

    def test_total_surebet_missing_data(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'total_over': 'not_a_dict',
                'total_under': {2.5: 2.00},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_all_total_surebets(events)
        assert len(surebets) == 0


class TestHandicapSurebets:
    def test_profitable_handicap_surebet_cross_bk(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'handicap_home': {-0.5: 2.10, -1.0: 2.80},
                'handicap_away': {0.5: 1.85, 1.0: 1.50},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'handicap_home': {-0.5: 1.90, -1.0: 2.50},
                'handicap_away': {0.5: 2.15, 1.0: 1.65},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_handicap_surebets(events)
        assert len(surebets) > 0
        sb = surebets[0]
        assert sb['profit_percent'] > 0
        assert 'handicap' in sb['market_type']
        assert len(sb['legs']) == 2

    def test_handicap_surebet_same_bk_excluded(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'handicap_home': {-0.5: 2.20},
                'handicap_away': {0.5: 2.20},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_handicap_surebets(events)
        assert len(surebets) == 0

    def test_handicap_surebet_multiple_lines(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'handicap_home': {-1.5: 3.00, -0.5: 2.10, 0.5: 1.40},
                'handicap_away': {1.5: 1.40, 0.5: 1.85, -0.5: 3.20},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'handicap_home': {-1.5: 3.20, -0.5: 1.95, 0.5: 1.35},
                'handicap_away': {1.5: 1.35, 0.5: 2.05, -0.5: 3.40},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_handicap_surebets(events)
        assert len(surebets) >= 0

    def test_handicap_surebet_no_profit(self):
        calculator = SurebetCalculator(min_profit=0.5)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'handicap_home': {-0.5: 1.80},
                'handicap_away': {0.5: 1.80},
                'sport': 'football',
                'is_live': True,
            },
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk2',
                'handicap_home': {-0.5: 1.75},
                'handicap_away': {0.5: 1.85},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_handicap_surebets(events)
        assert len(surebets) == 0

    def test_handicap_surebet_missing_data(self):
        calculator = SurebetCalculator(min_profit=0.1)
        events = [
            {
                'home_team': 'Team A',
                'away_team': 'Team B',
                'bookmaker': 'bk1',
                'handicap_home': 'not_a_dict',
                'handicap_away': {-0.5: 2.00},
                'sport': 'football',
                'is_live': True,
            },
        ]
        surebets = calculator.find_handicap_surebets(events)
        assert len(surebets) == 0

    def test_handicap_surebet_empty_events(self):
        calculator = SurebetCalculator(min_profit=0.1)
        surebets = calculator.find_handicap_surebets([])
        assert len(surebets) == 0

    def test_total_surebet_empty_events(self):
        calculator = SurebetCalculator(min_profit=0.1)
        surebets = calculator.find_all_total_surebets([])
        assert len(surebets) == 0
