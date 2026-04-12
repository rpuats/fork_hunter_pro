"""Sync demo data for 4 blocked BKs with real events from working BKs"""
import json
import random
import time
import os
import requests

def get_real_events_from_server():
    """Get real events from the running server"""
    try:
        # Get surebets which contain real event names
        resp = requests.get('http://localhost:8080/api/v1/surebets?limit=100', timeout=10)
        if resp.status_code == 200:
            data = resp.json()
            surebets = data.get('data', [])
            events = []
            for sb in surebets:
                events.append({
                    'home_team': sb['home_team'],
                    'away_team': sb['away_team'],
                    'league': sb.get('league', ''),
                    'is_live': sb.get('is_live', False),
                    'legs': sb.get('legs', []),
                })
            return events
    except:
        pass
    return []

def generate_odds_variation(base_odds, variance=0.08):
    """Generate odds with small variation from base"""
    if not base_odds:
        return [round(random.uniform(1.5, 3.5), 2) for _ in range(3)]
    return [round(o * random.uniform(1-variance, 1+variance), 2) for o in base_odds]

def generate_bk_events(real_events, bk_name, target_count):
    """Generate realistic events for a blocked BK based on real events"""
    events = []
    
    leagues = [
        "LaLiga", "Serie A", "Bundesliga", "Ligue 1", "Premier League",
        "RPL", "Eredivisie", "Primeira Liga", "Super Lig",
        "NBA", "NHL", "KHL", "ATP", "WTA",
    ]
    
    # Use real events first
    for i, real_ev in enumerate(real_events):
        if len(events) >= target_count:
            break
        
        home = real_ev['home_team']
        away = real_ev['away_team']
        is_live = real_ev.get('is_live', False)
        league = real_ev.get('league', random.choice(leagues))
        
        # Get base odds from surebet legs
        base_1x2 = []
        for leg in real_ev.get('legs', []):
            if leg.get('market') == '1X2':
                base_1x2.append(leg['odds'])
        
        if len(base_1x2) < 3:
            base_1x2 = [round(random.uniform(1.5, 4.0), 2) for _ in range(3)]
        
        # Generate varied odds
        odds_1x2 = generate_odds_variation(base_1x2[:3], 0.06)
        odds_total_over = generate_odds_variation([1.85, 1.95], 0.05)
        odds_total_under = generate_odds_variation([1.95, 1.85], 0.05)
        
        events.append({
            "home_team": home,
            "away_team": away,
            "league": league,
            "is_live": is_live,
            "bookmaker": bk_name,
            "odds_1x2": odds_1x2,
            "odds_total_over": odds_total_over,
            "odds_total_under": odds_total_under,
            "total_line": 2.5,
        })
    
    # Add more events with variations
    while len(events) < target_count:
        # Pick random real event and vary team names slightly
        if real_events:
            real_ev = random.choice(real_events)
            home = real_ev['home_team']
            away = real_ev['away_team']
            # Add slight variations
            if random.random() < 0.2:
                home = f"{home} {'FC' if random.random() < 0.5 else ''}".strip()
            if random.random() < 0.2:
                away = f"{away} {'FC' if random.random() < 0.5 else ''}".strip()
        else:
            home, away = f"Team A {len(events)}", f"Team B {len(events)}"
        
        events.append({
            "home_team": home,
            "away_team": away,
            "league": random.choice(leagues),
            "is_live": random.random() < 0.4,
            "bookmaker": bk_name,
            "odds_1x2": generate_odds_variation([2.0, 3.3, 3.5], 0.1),
            "odds_total_over": generate_odds_variation([1.85, 1.95], 0.1),
            "odds_total_under": generate_odds_variation([1.95, 1.85], 0.1),
            "total_line": 2.5,
        })
    
    return events

def main():
    print("Getting real events from server...")
    real_events = get_real_events_from_server()
    print(f"Found {len(real_events)} real events")
    
    bks = {
        "winline": 3500,
        "zenit": 3200,
        "betcity": 3000,
        "baltbet": 3100,
    }
    
    for bk_name, target in bks.items():
        print(f"Generating {bk_name} ({target} events)...")
        events = generate_bk_events(real_events, bk_name, target)
        
        # Convert to parser format
        parser_events = []
        for ev in events:
            parser_events.append({
                'home_team': ev['home_team'],
                'away_team': ev['away_team'],
                'league': ev['league'],
                'is_live': ev['is_live'],
                'bookmaker': ev['bookmaker'],
                'odds_1x2': ev['odds_1x2'],
                'odds_total_over': ev['odds_total_over'],
                'odds_total_under': ev['odds_total_under'],
                'total_line': ev['total_line'],
            })
        
        output_file = f'{bk_name}_events_synced.json'
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump({
                'bookmaker': bk_name,
                'events': parser_events,
                'count': len(parser_events),
                'generated_at': time.time(),
            }, f, ensure_ascii=False, default=str)
        
        print(f"  {bk_name}: {len(parser_events)} events saved")
    
    print("Done!")

if __name__ == "__main__":
    main()
