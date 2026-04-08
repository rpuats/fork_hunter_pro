# services/mock_data.py
"""
Mock data generator for testing the system without real bookmaker APIs
"""
import random
import time
from typing import List, Dict
from datetime import datetime

TEAMS = {
    'football': [
        ('Реал Мадрид', 'Барселона'),
        ('Манчестер Сити', 'Ливерпуль'),
        ('Бавария', 'Боруссия Дортмунд'),
        ('ПСЖ', 'Марсель'),
        ('Ювентус', 'Интер'),
        ('Спартак', 'ЦСКА'),
        ('Зенит', 'Локомотив'),
        ('Арсенал', 'Челси'),
        ('Атлетико', 'Севилья'),
        ('Наполи', 'Милан'),
        ('Тоттенхэм', 'Манчестер Юнайтед'),
        ('Рома', 'Лацио'),
        ('Динамо Москва', 'Краснодар'),
        ('Ростов', 'Урал'),
        ('Бетис', 'Вильярреал'),
    ],
    'hockey': [
        ('ЦСКА', 'СКА'),
        ('Динамо Москва', 'Спартак'),
        ('Ак Барс', 'Металлург'),
        ('Авангард', 'Салават Юлаев'),
    ],
    'basketball': [
        ('ЦСКА', 'Реал'),
        ('Барселона', 'Олимпиакос'),
        ('Фенербахче', 'Анадолу'),
    ],
}

BOOKMAKERS = ['winline', 'fonbet', 'pari', 'olimp', 'betboom', '1xstavka', 'leon', 'marathon', 'betcity', 'pinup', 'zenit', 'olimpbet']


def generate_mock_events(count: int = 50) -> List[Dict]:
    """Generate realistic mock events for testing"""
    events = []
    
    for i in range(count):
        sport = random.choice(list(TEAMS.keys()))
        home, away = random.choice(TEAMS[sport])
        bookmaker = random.choice(BOOKMAKERS)
        
        base_odds = random.uniform(1.5, 3.5)
        home_odds = round(base_odds + random.uniform(-0.3, 0.3), 2)
        away_odds = round(base_odds + random.uniform(-0.3, 0.3), 2)
        draw_odds = round(random.uniform(2.8, 4.2), 2) if sport == 'football' else None
        
        events.append({
            'id': f"mock_{bookmaker}_{i}_{int(time.time())}",
            'bookmaker': bookmaker,
            'sport': sport,
            'home_team': home,
            'away_team': away,
            'league': f"Test League {random.randint(1, 5)}",
            'home_odds': max(1.01, home_odds),
            'draw_odds': draw_odds if draw_odds and draw_odds > 1.0 else None,
            'away_odds': max(1.01, away_odds),
            'is_live': random.choice([True, True, True, False]),
            'market': '1x2',
            'source_url': f"https://{bookmaker}.ru/live",
            'scraped_at': time.time()
        })
    
    return events


def generate_mock_surebets(count: int = 5) -> List[Dict]:
    """Generate mock surebets for testing"""
    surebets = []
    
    for i in range(count):
        sport = random.choice(list(TEAMS.keys()))
        home, away = random.choice(TEAMS[sport])
        bk1, bk2 = random.sample(BOOKMAKERS, 2)
        
        profit = round(random.uniform(0.5, 12.0), 2)
        total_stake = 10000.0
        
        home_odds = round(random.uniform(1.8, 2.5), 2)
        away_odds = round(random.uniform(1.8, 2.5), 2)
        
        margin = (1/home_odds) + (1/away_odds)
        if margin >= 1:
            home_odds = round(random.uniform(2.0, 2.8), 2)
            away_odds = round(random.uniform(2.0, 2.8), 2)
            margin = (1/home_odds) + (1/away_odds)
        
        profit = round((1/margin - 1) * 100, 2)
        stake1 = total_stake * (1/home_odds) / margin
        stake2 = total_stake * (1/away_odds) / margin
        
        surebet = {
            'id': f"mock_sb_{i}_{int(time.time())}",
            'event_name': f"{home} vs {away}",
            'sport': sport,
            'market_type': '2-way',
            'is_live': random.choice([True, True, False]),
            'profit_percent': profit,
            'total_stake': total_stake,
            'estimated_profit': round(total_stake * profit / 100, 2),
            'legs': [
                {
                    'bookmaker': bk1,
                    'market': '1',
                    'selection': 'П1',
                    'odds': home_odds,
                    'event_name': f"{home} - {away}",
                    'calculated_stake': round(stake1, 2),
                    'stake_percent': round(stake1/total_stake*100, 1)
                },
                {
                    'bookmaker': bk2,
                    'market': '2',
                    'selection': 'П2',
                    'odds': away_odds,
                    'event_name': f"{home} - {away}",
                    'calculated_stake': round(stake2, 2),
                    'stake_percent': round(stake2/total_stake*100, 1)
                }
            ],
            'bookmakers': [bk1, bk2],
            'found_at': datetime.utcnow().isoformat()
        }
        
        surebets.append(surebet)
    
    return sorted(surebets, key=lambda x: x['profit_percent'], reverse=True)
