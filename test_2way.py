import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.finder import SurebetCalculator

calc = SurebetCalculator(min_profit=0.5)

events = [
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'winline', 'home_odds': 2.1, 'draw_odds': 3.4, 'away_odds': 3.8, 'league': 'Test'},
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'pari', 'home_odds': 1.9, 'draw_odds': 3.5, 'away_odds': 4.2, 'league': 'Test'},
    {'home_team': 'Team A', 'away_team': 'Team B', 'bookmaker': 'baltbet', 'home_odds': 2.0, 'draw_odds': 3.3, 'away_odds': 4.0, 'league': 'Test'},
]

surebets = calc.find_2way_surebets(events)
print(f'2-WAY SUREBETS: {len(surebets)} found')
for sb in surebets[:5]:
    legs = sb['legs']
    print(f'  {sb["event_name"]}: {sb["profit_percent"]:.2f}% | {legs[0]["bookmaker"]} ({legs[0]["odds"]}) vs {legs[1]["bookmaker"]} ({legs[1]["odds"]})')
    print(f'    Stakes: {legs[0]["calculated_stake"]:.0f} / {legs[1]["calculated_stake"]:.0f} | Profit: {sb["estimated_profit"]:.0f}')
