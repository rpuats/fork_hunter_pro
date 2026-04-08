# tests/test_odds_verifier.py
import pytest
import sys
import os
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.odds_verifier import OddsVerifier, VerificationResult, VerifierStats


def make_event(home, away, bk, home_odds, away_odds, draw_odds=None, **kwargs):
    return {
        'home_team': home,
        'away_team': away,
        'bookmaker': bk,
        'home_odds': home_odds,
        'away_odds': away_odds,
        'draw_odds': draw_odds or 0,
        'sport': 'football',
        'is_live': True,
        **kwargs,
    }


def make_surebet(sb_id, legs, profit=1.5):
    return {
        'id': sb_id,
        'event_name': legs[0].get('event_name', 'Team A vs Team B'),
        'sport': 'football',
        'market_type': '2-way',
        'is_live': True,
        'profit_percent': profit,
        'total_stake': 10000,
        'estimated_profit': 150,
        'legs': legs,
        'bookmakers': [leg['bookmaker'] for leg in legs],
        'found_at': '2026-04-01T00:00:00',
    }


def make_leg(bk, selection, odds, event_name='Team A vs Team B', market='1'):
    return {
        'bookmaker': bk,
        'market': market,
        'selection': selection,
        'odds': odds,
        'event_name': event_name,
        'calculated_stake': 5000,
        'stake_percent': 50.0,
    }


class TestOddsVerifierInit:
    def test_default_tolerance(self):
        v = OddsVerifier()
        assert v.tolerance == 0.02

    def test_custom_tolerance(self):
        v = OddsVerifier(tolerance=0.05)
        assert v.tolerance == 0.05

    def test_stats_initially_zero(self):
        v = OddsVerifier()
        s = v.get_stats()
        assert s['total_checked'] == 0
        assert s['total_valid'] == 0
        assert s['total_expired'] == 0


class TestVerifySurebet:
    def test_valid_surebet_unchanged_odds(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        sb = make_surebet('sb1', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is True
        assert len(result.expired_legs) == 0

    def test_expired_surebet_odds_dropped(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 1.90, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 1.95),
        ]
        sb = make_surebet('sb2', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is False
        assert len(result.expired_legs) == 2

    def test_partial_expiry_one_leg_expired(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 1.90),
        ]
        sb = make_surebet('sb3', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is False
        assert len(result.expired_legs) == 1

    def test_surebet_event_not_found(self):
        v = OddsVerifier()
        events = [
            make_event('Team X', 'Team Y', 'bk1', 2.10, 1.90),
        ]
        sb = make_surebet('sb4', [
            make_leg('bk1', 'П1', 2.10, event_name='Team A vs Team B', market='1'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is False
        assert len(result.expired_legs) == 1
        assert 'not found' in result.expired_legs[0]['reason'].lower()

    def test_surebet_no_legs(self):
        v = OddsVerifier()
        sb = {
            'id': 'sb5',
            'event_name': 'Team A vs Team B',
            'sport': 'football',
            'market_type': '2-way',
            'is_live': True,
            'profit_percent': 1.5,
            'total_stake': 10000,
            'estimated_profit': 150,
            'legs': [],
            'bookmakers': [],
            'found_at': '2026-04-01T00:00:00',
        }
        result = v.verify_surebet(sb, [])
        assert result.is_valid is False
        assert 'no legs' in result.reason.lower()

    def test_tolerance_boundary_exact(self):
        v = OddsVerifier(tolerance=0.03)
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.12, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        sb = make_surebet('sb6', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is True

    def test_tolerance_boundary_exceeded(self):
        v = OddsVerifier(tolerance=0.02)
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.13, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        sb = make_surebet('sb7', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is False


class TestVerifyOdds:
    def test_valid_odds(self):
        v = OddsVerifier()
        event = make_event('Team A', 'Team B', 'bk1', 2.10, 1.90)
        assert v.verify_odds(event) is True

    def test_invalid_zero_odds(self):
        v = OddsVerifier()
        event = make_event('Team A', 'Team B', 'bk1', 0, 1.90)
        assert v.verify_odds(event) is False

    def test_invalid_negative_odds(self):
        v = OddsVerifier()
        event = make_event('Team A', 'Team B', 'bk1', -1.0, 1.90)
        assert v.verify_odds(event) is False

    def test_empty_event(self):
        v = OddsVerifier()
        assert v.verify_odds({}) is False


class TestGetExpiredSurebets:
    def test_all_expired(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 1.50, 1.50),
            make_event('Team A', 'Team B', 'bk2', 1.50, 1.50),
        ]
        surebets = [
            make_surebet('sb1', [
                make_leg('bk1', 'П1', 2.10, market='1'),
                make_leg('bk2', 'П2', 2.15, market='2'),
            ]),
        ]
        expired = v.get_expired_surebets(surebets, events)
        assert len(expired) == 1
        assert 'verification' in expired[0]

    def test_none_expired(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        surebets = [
            make_surebet('sb1', [
                make_leg('bk1', 'П1', 2.10, market='1'),
                make_leg('bk2', 'П2', 2.15, market='2'),
            ]),
        ]
        expired = v.get_expired_surebets(surebets, events)
        assert len(expired) == 0


class TestGetValidSurebets:
    def test_all_valid(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        surebets = [
            make_surebet('sb1', [
                make_leg('bk1', 'П1', 2.10, market='1'),
                make_leg('bk2', 'П2', 2.15, market='2'),
            ]),
        ]
        valid = v.get_valid_surebets(surebets, events)
        assert len(valid) == 1
        assert 'verified_profit' in valid[0]

    def test_none_valid(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 1.50, 1.50),
        ]
        surebets = [
            make_surebet('sb1', [
                make_leg('bk1', 'П1', 2.10, market='1'),
            ]),
        ]
        valid = v.get_valid_surebets(surebets, events)
        assert len(valid) == 0


class TestVerifyBatch:
    def test_mixed_batch(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
            make_event('Team C', 'Team D', 'bk1', 1.50, 1.50),
        ]
        surebets = [
            make_surebet('sb_valid', [
                make_leg('bk1', 'П1', 2.10, market='1'),
                make_leg('bk2', 'П2', 2.15, market='2'),
            ]),
            make_surebet('sb_expired', [
                make_leg('bk1', 'П1', 2.10, event_name='Team C vs Team D', market='1'),
            ]),
        ]
        valid, expired = v.verify_batch(surebets, events)
        assert len(valid) == 1
        assert len(expired) == 1
        assert valid[0]['id'] == 'sb_valid'
        assert expired[0]['id'] == 'sb_expired'

    def test_empty_batch(self):
        v = OddsVerifier()
        valid, expired = v.verify_batch([], [])
        assert len(valid) == 0
        assert len(expired) == 0


class TestStats:
    def test_stats_tracking(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 2.10, 1.90),
            make_event('Team A', 'Team B', 'bk2', 1.95, 2.15),
        ]
        sb = make_surebet('sb1', [
            make_leg('bk1', 'П1', 2.10, market='1'),
            make_leg('bk2', 'П2', 2.15, market='2'),
        ])
        v.verify_surebet(sb, events)
        s = v.get_stats()
        assert s['total_checked'] == 1
        assert s['total_valid'] == 1
        assert s['total_expired'] == 0
        assert s['total_legs_checked'] == 2

    def test_stats_after_batch(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 1.50, 1.50),
        ]
        surebets = [
            make_surebet('sb1', [make_leg('bk1', 'П1', 2.10, market='1')]),
            make_surebet('sb2', [make_leg('bk1', 'П1', 2.10, market='1')]),
        ]
        v.verify_batch(surebets, events)
        s = v.get_stats()
        assert s['total_checked'] == 2
        assert s['total_valid'] == 0
        assert s['total_expired'] == 2


class TestOddsHistory:
    def test_clear_history(self):
        v = OddsVerifier()
        event = make_event('Team A', 'Team B', 'bk1', 2.10, 1.90)
        v.verify_odds(event)
        assert len(v._odds_history) > 0
        v.clear_history()
        assert len(v._odds_history) == 0

    def test_prune_history(self):
        v = OddsVerifier()
        event = make_event('Team A', 'Team B', 'bk1', 2.10, 1.90)
        v.verify_odds(event)
        v._odds_history['old_key'] = {'home_odds': 1.0, 'away_odds': 1.0, 'ts': 0}
        pruned = v.prune_history(max_age=1.0)
        assert pruned >= 1


class TestProfitCalculation:
    def test_profitable_margin(self):
        v = OddsVerifier()
        profit = v._calculate_profit([2.10, 2.15])
        assert profit > 0

    def test_unprofitable_margin(self):
        v = OddsVerifier()
        profit = v._calculate_profit([1.50, 1.50])
        assert profit == 0

    def test_single_odds(self):
        v = OddsVerifier()
        profit = v._calculate_profit([2.00])
        assert profit == 0

    def test_empty_odds(self):
        v = OddsVerifier()
        profit = v._calculate_profit([])
        assert profit == 0


class TestVerifierStats:
    def test_initial_summary(self):
        s = VerifierStats()
        summary = s.summary()
        assert summary['total_checked'] == 0
        assert summary['validation_rate'] == 0.0

    def test_record_updates_avg(self):
        s = VerifierStats()
        s.record(10.0)
        s.record(20.0)
        assert s.avg_verification_time_ms == 15.0


class TestEdgeCases:
    def test_3way_surebet_verification(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'bk1', 3.50, 2.00, draw_odds=3.60),
            make_event('Team A', 'Team B', 'bk2', 2.80, 2.50, draw_odds=3.80),
            make_event('Team A', 'Team B', 'bk3', 2.90, 3.00, draw_odds=3.40),
        ]
        sb = make_surebet('sb_3way', [
            make_leg('bk1', 'П1', 3.50, market='1'),
            make_leg('bk2', 'Ничья', 3.80, market='X'),
            make_leg('bk3', 'П2', 3.00, market='2'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is True
        assert len(result.expired_legs) == 0

    def test_total_market_verification(self):
        v = OddsVerifier()
        event = {
            'home_team': 'Team A',
            'away_team': 'Team B',
            'bookmaker': 'bk1',
            'over_odds': 2.10,
            'under_odds': 1.90,
            'total_over': {2.5: 2.10},
            'total_under': {2.5: 1.90},
            'sport': 'football',
            'is_live': True,
        }
        sb = make_surebet('sb_total', [
            {'bookmaker': 'bk1', 'market': 'ТБ', 'selection': 'ТБ 2.5', 'odds': 2.10, 'event_name': 'Team A vs Team B'},
        ])
        result = v.verify_surebet(sb, [event])
        assert result.is_valid is True

    def test_handicap_market_verification(self):
        v = OddsVerifier()
        event = {
            'home_team': 'Team A',
            'away_team': 'Team B',
            'bookmaker': 'bk1',
            'handicap_home': {-0.5: 2.10},
            'handicap_away': {0.5: 1.90},
            'sport': 'football',
            'is_live': True,
        }
        sb = make_surebet('sb_hc', [
            {'bookmaker': 'bk1', 'market': 'Handicap -0.5', 'selection': 'Ф1 (-0.5)', 'odds': 2.10, 'event_name': 'Team A vs Team B'},
        ])
        result = v.verify_surebet(sb, [event])
        assert result.is_valid is True

    def test_case_insensitive_bookmaker_match(self):
        v = OddsVerifier()
        events = [
            make_event('Team A', 'Team B', 'BK1', 2.10, 1.90),
        ]
        sb = make_surebet('sb_case', [
            make_leg('bk1', 'П1', 2.10, market='1'),
        ])
        result = v.verify_surebet(sb, events)
        assert result.is_valid is True

    def test_verification_result_fields(self):
        v = OddsVerifier()
        result = VerificationResult(
            surebet_id='test',
            is_valid=True,
            original_profit=1.5,
            verified_profit=1.4,
            expired_legs=[],
            reason='',
            verified_at=time.time(),
        )
        assert result.surebet_id == 'test'
        assert result.is_valid is True
        assert result.original_profit == 1.5
        assert result.verified_profit == 1.4
