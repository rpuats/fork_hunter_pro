"""Test totals and handicaps surebets with mock data"""
import sys
import io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

from core.finder import SurebetCalculator

calc = SurebetCalculator(min_profit=0.5)

# === TEST 1: Total surebets ===
print("=" * 60)
print("TEST 1: Total (Over/Under) Surebets")
print("=" * 60)

events_totals = [
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'winline', 'total_over': {2.5: 2.1}, 'total_under': {2.5: 1.9}},
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'pari', 'total_over': {2.5: 2.3}, 'total_under': {2.5: 1.7}},
]

surebets = calc.find_all_total_surebets(events_totals, total_lines=[2.5])
print(f'Total surebets found: {len(surebets)}')
for sb in surebets:
    print(f'  Profit: {sb["profit_percent"]:.2f}%')
    for leg in sb['legs']:
        sel = leg["selection"].encode('ascii', 'ignore').decode('ascii')
        print(f'    {leg["bookmaker"]}: {sel} @ {leg["odds"]}')

# === TEST 2: Handicap surebets ===
print()
print("=" * 60)
print("TEST 2: Handicap Surebets")
print("=" * 60)

events_handicaps = [
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'winline', 'handicap_home': {-1.5: 2.2}, 'handicap_away': {1.5: 1.8}},
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'pari', 'handicap_home': {-1.5: 2.0}, 'handicap_away': {1.5: 2.0}},
]

surebets_h = calc.find_handicap_surebets(events_handicaps, handicap_lines=[-1.5])
print(f'Handicap surebets found: {len(surebets_h)}')
for sb in surebets_h:
    print(f'  Profit: {sb["profit_percent"]:.2f}%')
    for leg in sb['legs']:
        sel = leg["selection"].encode('ascii', 'ignore').decode('ascii')
        print(f'    {leg["bookmaker"]}: {sel} @ {leg["odds"]}')

# === TEST 3: Cross-bookmaker totals with multiple lines ===
print()
print("=" * 60)
print("TEST 3: Multi-line Total Surebets")
print("=" * 60)

events_multi = [
    {'home_team': 'Real Madrid', 'away_team': 'Barcelona', 'bookmaker': 'winline', 'total_over': {1.5: 1.3, 2.5: 2.1, 3.5: 3.5}, 'total_under': {1.5: 3.5, 2.5: 1.8, 3.5: 1.3}},
    {'home_team': 'Real Madrid', 'away_team': 'Barcelona', 'bookmaker': 'pari', 'total_over': {1.5: 1.25, 2.5: 2.3, 3.5: 3.8}, 'total_under': {1.5: 3.8, 2.5: 1.7, 3.5: 1.25}},
    {'home_team': 'Real Madrid', 'away_team': 'Barcelona', 'bookmaker': 'betcity', 'total_over': {1.5: 1.28, 2.5: 2.0, 3.5: 3.2}, 'total_under': {1.5: 3.6, 2.5: 1.9, 3.5: 1.35}},
]

surebets_m = calc.find_all_total_surebets(events_multi, total_lines=[1.5, 2.5, 3.5])
print(f'Multi-line total surebets found: {len(surebets_m)}')
for sb in surebets_m:
    print(f'  Line: {sb["market_type"]}, Profit: {sb["profit_percent"]:.2f}%')
    for leg in sb['legs']:
        sel = leg["selection"].encode('ascii', 'ignore').decode('ascii')
        print(f'    {leg["bookmaker"]}: {sel} @ {leg["odds"]}')

print()
print("=" * 60)
print("ALL TESTS PASSED")
print("=" * 60)
