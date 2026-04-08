import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.odds_verifier import OddsVerifier
verifier = OddsVerifier()
surebet = {
    'event_name': 'Test',
    'legs': [
        {'bookmaker': 'winline', 'odds': 2.1, 'selection': 'P1'},
        {'bookmaker': 'pari', 'odds': 2.0, 'selection': 'P2'}
    ],
    'profit_percent': 1.5
}
events = [
    {'home_team': 'Test', 'away_team': 'Test', 'bookmaker': 'winline', 'home_odds': 2.1, 'away_odds': 3.8},
    {'home_team': 'Test', 'away_team': 'Test', 'bookmaker': 'pari', 'home_odds': 1.9, 'away_odds': 4.2}
]
valid = verifier.verify_surebet(surebet, events)
print(f'ODDS VERIFICATION: {"VALID" if valid else "EXPIRED"}')
