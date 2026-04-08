# tests/test_normalizer.py
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.normalizer import TeamNormalizer, EventNormalizer, SportsNormalizer


class TestTeamNameNormalization:
    def test_exact_alias_match(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('manchester united') == 'Манчестер Юнайтед'
        assert normalizer.normalize('man utd') == 'Манчестер Юнайтед'
        assert normalizer.normalize('liverpool') == 'Ливерпуль'
        assert normalizer.normalize('barcelona') == 'Барселона'

    def test_russian_teams(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('spartak moscow') == 'Спартак Москва'
        assert normalizer.normalize('cska') == 'ЦСКА Москва'
        assert normalizer.normalize('zenit') == 'Зенит'

    def test_unknown_team_returns_original(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('Unknown Team') == 'Unknown Team'

    def test_empty_string(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('') == ''

    def test_none_handling(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize(None) is None

    def test_whitespace_handling(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('  liverpool  ') == 'Ливерпуль'

    def test_case_insensitive(self):
        normalizer = TeamNormalizer()
        assert normalizer.normalize('LIVERPOOL') == 'Ливерпуль'
        assert normalizer.normalize('Liverpool') == 'Ливерпуль'
        assert normalizer.normalize('liverpool') == 'Ливерпуль'


class TestFuzzyMatching:
    def test_fuzzy_match_similar_names(self):
        normalizer = TeamNormalizer()
        assert normalizer._fuzzy_match('liverpool', 'liverpool') is True

    def test_fuzzy_match_substring(self):
        normalizer = TeamNormalizer()
        assert normalizer._fuzzy_match('liver', 'liverpool') is True

    def test_fuzzy_match_different_names(self):
        normalizer = TeamNormalizer()
        assert normalizer._fuzzy_match('liverpool', 'chelsea') is False

    def test_fuzzy_match_empty_strings(self):
        normalizer = TeamNormalizer()
        assert normalizer._fuzzy_match('', '') is True

    def test_levenshtein_ratio_identical(self):
        normalizer = TeamNormalizer()
        assert normalizer._levenshtein_ratio('test', 'test') == 1.0

    def test_levenshtein_ratio_completely_different(self):
        normalizer = TeamNormalizer()
        ratio = normalizer._levenshtein_ratio('abc', 'xyz')
        assert ratio < 1.0

    def test_levenshtein_ratio_empty_vs_nonempty(self):
        normalizer = TeamNormalizer()
        assert normalizer._levenshtein_ratio('', 'test') == 0.0

    def test_levenshtein_ratio_both_empty(self):
        normalizer = TeamNormalizer()
        assert normalizer._levenshtein_ratio('', '') == 1.0


class TestEventNormalization:
    def test_normalize_event(self):
        normalizer = EventNormalizer()
        home, away = normalizer.normalize_event('Team A', 'Team B')
        assert isinstance(home, str)
        assert isinstance(away, str)
        assert len(home) > 0
        assert len(away) > 0

    def test_normalize_event_sorted(self):
        normalizer = EventNormalizer()
        home, away = normalizer.normalize_event('Zebra', 'Alpha')
        assert home == 'Alpha'
        assert away == 'Zebra'

    def test_get_event_key(self):
        normalizer = EventNormalizer()
        key = normalizer.get_event_key('Team A', 'Team B', '1x2')
        assert isinstance(key, str)
        assert '|' in key
        assert '1x2' in key

    def test_are_same_event(self):
        normalizer = EventNormalizer()
        assert normalizer.are_same_event('Team A', 'Team B', 'Team A', 'Team B') is True

    def test_are_same_event_different(self):
        normalizer = EventNormalizer()
        assert normalizer.are_same_event('Team A', 'Team B', 'Team C', 'Team D') is False

    def test_are_same_event_swapped(self):
        normalizer = EventNormalizer()
        assert normalizer.are_same_event('Team A', 'Team B', 'Team B', 'Team A') is True


class TestSportsNormalizer:
    def test_normalize_football(self):
        assert SportsNormalizer.normalize_sport('football') == 'football'
        assert SportsNormalizer.normalize_sport('soccer') == 'football'
        assert SportsNormalizer.normalize_sport('футбол') == 'football'

    def test_normalize_hockey(self):
        assert SportsNormalizer.normalize_sport('hockey') == 'hockey'
        assert SportsNormalizer.normalize_sport('хоккей') == 'hockey'

    def test_normalize_basketball(self):
        assert SportsNormalizer.normalize_sport('basketball') == 'basketball'
        assert SportsNormalizer.normalize_sport('баскетбол') == 'basketball'

    def test_normalize_tennis(self):
        assert SportsNormalizer.normalize_sport('tennis') == 'tennis'
        assert SportsNormalizer.normalize_sport('теннис') == 'tennis'

    def test_normalize_unknown_sport(self):
        assert SportsNormalizer.normalize_sport('unknown') == 'other'

    def test_normalize_empty_sport(self):
        assert SportsNormalizer.normalize_sport('') == 'other'
        assert SportsNormalizer.normalize_sport(None) == 'other'


class TestEdgeCases:
    def test_special_characters_in_team_name(self):
        normalizer = TeamNormalizer()
        result = normalizer.normalize('Team A!@#$%')
        assert isinstance(result, str)

    def test_numbers_in_team_name(self):
        normalizer = TeamNormalizer()
        result = normalizer.normalize('Team 123')
        assert isinstance(result, str)

    def test_very_long_team_name(self):
        normalizer = TeamNormalizer()
        long_name = 'A' * 1000
        result = normalizer.normalize(long_name)
        assert isinstance(result, str)

    def test_unicode_team_names(self):
        normalizer = TeamNormalizer()
        result = normalizer.normalize('Реал Мадрид')
        assert isinstance(result, str)
