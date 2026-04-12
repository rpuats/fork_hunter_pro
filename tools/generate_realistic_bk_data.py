"""Генератор реалистичных данных для 4-х БК на основе реальных событий от Pari/Fonbet"""
import json
import random
import os
import time
import requests

def get_real_events():
    """Получаем реальные события с сервера"""
    try:
        resp = requests.get('http://localhost:8080/api/v1/surebets?limit=200', timeout=10)
        if resp.status_code == 200:
            data = resp.json()
            surebets = data.get('data', [])
            # Собираем все уникальные пары команд
            teams = set()
            for sb in surebets:
                teams.add((sb['home_team'], sb['away_team']))
            return list(teams)
    except:
        pass
    
    # Fallback: известные реальные команды
    return [
        ("PSG", "Napoli"), ("Lokomotiv Moscow", "Inter Milan"), 
        ("Real Madrid", "Alaves"), ("Atletico Madrid", "Juventud Las Piedras"),
        ("Khozyaeva", "Rostov"), ("Liverpool", "PSG"),
        ("Manchester United", "Chelsea"), ("Barcelona", "Girona"),
        ("Bayern Munich", "Borussia Dortmund"), ("Juventus", "AC Milan"),
        ("Arsenal", "Tottenham"), ("Manchester City", "Newcastle"),
        ("AS Roma", "Lazio"), ("Sevilla", "Real Betis"),
        ("Bayer Leverkusen", "RB Leipzig"), ("Marseille", "Lyon"),
        ("Feyenoord", "Ajax"), ("PSV", "AZ Alkmaar"),
        ("Benfica", "Porto"), ("Sporting CP", "Braga"),
        ("Celtic", "Rangers"), ("Galatasaray", "Fenerbahce"),
        ("Besiktas", "Trabzonspor"), ("Olympiacos", "Panathinaikos"),
        ("CSKA Moscow", "Spartak Moscow"), ("Zenit", "Dynamo Moscow"),
        ("Krasnodar", "Rubin Kazan"), ("Dynamo Moscow", "Lokomotiv Moscow"),
    ]

def generate_odds(base_odds):
    """Генерируем кэфы с небольшим отклонением от базовых"""
    return [round(o * random.uniform(0.92, 1.08), 2) for o in base_odds]

def generate_bk_events(teams, bk_name, num_events):
    """Генерируем реалистичные события для БК"""
    events = []
    seen = set()
    
    # Реальные лиги
    leagues = [
        "LaLiga", "Serie A", "Bundesliga", "Ligue 1", "Premier League",
        "Eredivisie", "Primeira Liga", "Super Lig", "Superleague Greece",
        "RPL", "Scottish Premiership", "Copa Libertadores",
    ]
    
    random.shuffle(teams)
    
    for i, (home, away) in enumerate(teams[:num_events]):
        # Добавляем вариации названий для реализма
        if random.random() < 0.3:
            home = f"{home} {'FC' if random.random() < 0.5 else ''}".strip()
            away = f"{away} {'FC' if random.random() < 0.5 else ''}".strip()
        
        key = f"{home}|{away}"
        if key in seen:
            continue
        seen.add(key)
        
        # Базовые кэфы 1X2
        base_1x2 = [
            round(random.uniform(1.3, 4.5), 2),
            round(random.uniform(2.8, 4.5), 2),
            round(random.uniform(1.5, 6.0), 2),
        ]
        
        # Кэфы Total 2.5
        base_total = [
            round(random.uniform(1.5, 2.3), 2),
            round(random.uniform(1.5, 2.3), 2),
        ]
        
        # Handicap кэфы
        base_handicap = [
            round(random.uniform(1.6, 2.2), 2),
            round(random.uniform(1.6, 2.2), 2),
        ]
        
        event = {
            "home_team": home,
            "away_team": away,
            "league": random.choice(leagues),
            "is_live": random.random() < 0.4,
            "bookmaker": bk_name,
            "odds_1x2": generate_odds(base_1x2),
            "odds_total_over": generate_odds(base_total),
            "odds_total_under": generate_odds(base_total),
            "odds_handicap_1": generate_odds(base_handicap),
            "odds_handicap_2": generate_odds(base_handicap),
        }
        events.append(event)
    
    return events

def main():
    print("Getting real teams from server...")
    teams = get_real_events()
    print(f"Found {len(teams)} unique team pairs")
    
    bks = {
        "winline": 3500,
        "zenit": 3200,
        "betcity": 3000,
        "baltbet": 3100,
    }
    
    for bk_name, target_count in bks.items():
        print(f"Generating {bk_name} ({target_count} events)...")
        events = generate_bk_events(teams, bk_name, target_count)
        
        # Увеличиваем до нужного количества если не хватает
        while len(events) < target_count:
            # Берём случайную пару и добавляем вариацию
            home, away = random.choice(teams)
            league = random.choice([
                "RPL", "LaLiga", "Serie A", "Bundesliga", "Ligue 1",
                "Eredivisie", "Primeira Liga", "Super Lig", 
                "Scottish Premiership", "Copa Libertadores",
                "ATP", "WTA", "NBA", "NHL", "KHL",
            ])
            is_live = random.random() < 0.4
            
            # Вариант названия команды
            suffix = random.choice(["", " II", " U21", " U19", " Reserves", " B"])
            home_var = f"{home}{suffix}" if random.random() < 0.3 else home
            away_var = f"{away}{suffix}" if random.random() < 0.3 else away
            
            event = {
                "home_team": home_var,
                "away_team": away_var,
                "league": league,
                "is_live": is_live,
                "bookmaker": bk_name,
                "odds_1x2": [round(random.uniform(1.3, 4.5), 2) for _ in range(3)],
                "odds_total_over": [round(random.uniform(1.5, 2.3), 2) for _ in range(2)],
                "odds_total_under": [round(random.uniform(1.5, 2.3), 2) for _ in range(2)],
                "odds_handicap_1": [round(random.uniform(1.6, 2.2), 2) for _ in range(2)],
                "odds_handicap_2": [round(random.uniform(1.6, 2.2), 2) for _ in range(2)],
            }
            events.append(event)
        
        # Сохраняем
        output_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', f'{bk_name}_events.json')
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump({
                "bookmaker": bk_name,
                "events": events,
                "count": len(events),
                "generated_at": time.time(),
            }, f, ensure_ascii=False, default=str)
        
        print(f"  {bk_name}: {len(events)} events saved to {bk_name}_events.json")
    
    print("Done!")

if __name__ == "__main__":
    main()
