import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.odds_verifier import OddsVerifier

verifier = OddsVerifier()

surebet = {
    'event_name': 'Team A vs Team B',
    'legs': [
        {'bookmaker': 'winline', 'odds': 2.1, 'selection': 'P1'},
        {'bookmaker': 'pari', 'odds': 2.0, 'selection': 'P2'}
    ],
    'profit_percent': 1.5
}

events = [
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'winline', 'home_odds': 2.1, 'away_odds': 3.8},
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'pari', 'home_odds': 1.9, 'away_odds': 4.2},
]

valid = verifier.verify_surebet(surebet, events)
status = "VALID" if valid else "EXPIRED"
print(f'ODDS VERIFICATION: {status}')
print(f'  Surebet: {surebet["event_name"]}')
print(f'  Profit: {surebet["profit_percent"]}%')
