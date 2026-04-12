use chrono::Utc;
/// 🧪 Диагностический тест: Cross-BK Event Matching
///
/// Цель: проверить что события от разных БК правильно матчатся
/// и вилки находятся между букмекерами
///
/// Запуск: cargo test --test cross_bk_matching -- --nocapture
use engine::calculator::SurebetCalculator;
use engine::normalizer::Normalizer;
use shared::odds::OddsType;
use shared::{Event, Odd, Sport};
use std::collections::HashMap;

fn make_event(id: &str, bookmaker: &str, home: &str, away: &str, league: &str) -> Event {
    Event {
        id: id.to_string(),
        sport: Sport::Football,
        league: league.to_string(),
        home_team: home.to_string(),
        away_team: away.to_string(),
        start_time: None,
        is_live: false,
        bookmaker_slug: bookmaker.to_string(),
        raw_url: None,
        extra: HashMap::new(),
    }
}

fn make_odd(
    id: &str,
    event_id: &str,
    bookmaker: &str,
    market: &str,
    selection: &str,
    odds: f64,
    line: Option<f64>,
) -> Odd {
    Odd {
        id: id.to_string(),
        event_id: event_id.to_string(),
        bookmaker_slug: bookmaker.to_string(),
        market: market.to_string(),
        selection: selection.to_string(),
        odds,
        odds_type: match selection {
            "1" | "П1" | "home" => OddsType::Home,
            "X" | "Х" | "draw" => OddsType::Draw,
            "2" | "П2" | "away" => OddsType::Away,
            "Over" | "больше" | "тб" => OddsType::Over,
            "Under" | "меньше" | "тм" => OddsType::Under,
            "Yes" | "да" => OddsType::BothTeamsScoreYes,
            "No" | "нет" => OddsType::BothTeamsScoreNo,
            _ => OddsType::Home,
        },
        line,
        timestamp: Utc::now(),
    }
}

/// Fingerprint из сканнера — копируем логику
fn event_fingerprint(event: &Event) -> String {
    let norm = Normalizer::new();
    let norm_event = norm.normalize_event(event.clone());

    let home = normalize_team_name(&norm_event.home_team);
    let away = normalize_team_name(&norm_event.away_team);
    let league = norm_event.league.to_lowercase().replace(" ", "");
    let (first, second) = if home < away {
        (home, away)
    } else {
        (away, home)
    };
    format!("{:?}|{}|{}|{}", event.sport, league, first, second)
}

fn normalize_team_name(name: &str) -> String {
    let mut s = name
        .to_lowercase()
        .replace("фк ", "")
        .replace("ск ", "")
        .replace("пк ", "")
        .replace("фк", "")
        .replace("ск", "")
        .replace("пк", "")
        .replace("хк ", "")
        .replace("хк", "")
        .replace(" москва", "")
        .replace(" спб", "")
        .replace(" санкт-петербург", "")
        .replace(" с.-петербург", "")
        .replace(" петербург", "")
        .replace(" питер", "")
        .replace("(", "")
        .replace(")", "")
        .replace("-", " ")
        .replace(".", "")
        .replace(",", "")
        .replace("'", "")
        .replace("\"", "")
        .replace("_", " ")
        .trim()
        .to_string();

    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    s.trim().to_string()
}

#[test]
fn test_01_fingerprint_basic() {
    println!("\n🧪 ТЕСТ 1: Базовый fingerprint");

    // Pari event
    let pari_event = make_event("pari_123", "pari", "Манчестер Юнайтед", "Ливерпуль", "АПЛ");

    // Fonbet event — SAME MATCH but different naming
    let fonbet_event = make_event(
        "fonbet_456",
        "fonbet",
        "Манчестер Юнайтед ",
        "Ливерпуль",
        "Английская Премьер-Лига",
    );

    let fp_pari = event_fingerprint(&pari_event);
    let fp_fonbet = event_fingerprint(&fonbet_event);

    println!(
        "  Pari:    {} vs {} ({})",
        pari_event.home_team, pari_event.away_team, pari_event.league
    );
    println!("    → FP: {}", fp_pari);
    println!(
        "  Fonbet:  {} vs {} ({})",
        fonbet_event.home_team, fonbet_event.away_team, fonbet_event.league
    );
    println!("    → FP: {}", fp_fonbet);
    println!(
        "  Match:  {}",
        if fp_pari == fp_fonbet {
            "✅ YES"
        } else {
            "❌ NO"
        }
    );

    assert_eq!(fp_pari, fp_fonbet, "Fingerprints must match for same game");
}

#[test]
fn test_02_different_leagues_no_match() {
    println!("\n🧪 ТЕСТ 2: Разные лиги — НЕ должны матчиться");

    let event_rpl = make_event("rpl_1", "pari", "Зенит", "Спартак", "РПЛ");

    let event_epl = make_event("epl_1", "fonbet", "Зенит", "Спартак", "АПЛ"); // Same teams, different league

    let fp_rpl = event_fingerprint(&event_rpl);
    let fp_epl = event_fingerprint(&event_epl);

    println!("  RPL:    {} (FP: {})", event_rpl.league, fp_rpl);
    println!("  EPL:    {} (FP: {})", event_epl.league, fp_epl);
    println!(
        "  Match:  {}",
        if fp_rpl == fp_epl {
            "❌ YES (bad!)"
        } else {
            "✅ NO (good!)"
        }
    );

    assert_ne!(fp_rpl, fp_epl, "Different leagues must not match");
}

#[test]
fn test_03_various_name_formats() {
    println!("\n🧪 ТЕСТ 3: Разные форматы названий");

    let names = vec![
        ("Реал Мадрид", "Барселона", "Ла Лига"),
        ("Реал", "Барса", "Испания"),
        ("Real Madrid", "Barcelona", "Primera Division"),
        ("фк Реал Мадрид", "фк Барселона", "la liga"),
    ];

    let fps: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, (home, away, league))| {
            let event = make_event(&format!("ev{}", i), "bk", home, away, league);
            let fp = event_fingerprint(&event);
            println!("  #{}: {} vs {} ({}) → {}", i + 1, home, away, league, fp);
            fp
        })
        .collect();

    // All must match
    for i in 1..fps.len() {
        assert_eq!(fps[0], fps[i], "All Real Madrid vs Barcelona must match");
    }
    println!("  ✅ All {} formats match!", fps.len());
}

#[test]
fn test_04_cross_bk_surebet_1x2() {
    println!("\n🧪 ТЕСТ 4: Кросс-БК вилка 1X2");

    // Создаём одно событие (как будто одна БК)
    let event = make_event("evt1", "pari", "Реал Мадрид", "Барселона", "Ла Лига");

    // Pari даёт 1 @ 2.10
    // Fonbet даёт X @ 3.80
    // Marathon даёт 2 @ 4.20
    let odds = vec![
        make_odd("o1", "evt1", "pari", "1X2", "1", 2.10, None),
        make_odd("o2", "evt1", "fonbet", "1X2", "X", 3.80, None),
        make_odd("o3", "evt1", "marathon", "1X2", "2", 4.20, None),
    ];

    let calc = SurebetCalculator::new(0.5, 30.0, 1000.0, 10000, 0.01);
    let surebets = calc.find_surebets(&[event], &odds);

    let profit = shared::odds::calculate_surebet_profit(&[2.10, 3.80, 4.20]);
    println!("  Odds: Pari:1@2.10, Fonbet:X@3.80, Marathon:2@4.20");
    println!("  Expected profit: {:.2}%", profit.unwrap_or(0.0));
    println!("  Found surebets: {}", surebets.len());

    if !surebets.is_empty() {
        let sb = &surebets[0];
        println!("  ✅ Surebet found! Profit: {:.2}%", sb.profit_percent);
        println!("  Legs:");
        for leg in &sb.legs {
            println!(
                "    - {} {}@{:.2} stake={:.2}",
                leg.bookmaker, leg.selection, leg.odds, leg.stake
            );
        }
    } else {
        println!("  ❌ No surebet found");
    }

    assert!(!surebets.is_empty(), "Should find cross-BK 1X2 surebet");
}

#[test]
fn test_05_cross_bk_surebet_totals() {
    println!("\n🧪 ТЕСТ 5: Кросс-БК вилка тоталы");

    let event = make_event("evt2", "pari", "Зенит", "Спартак", "РПЛ");

    // Pari: Over 2.5 @ 2.05
    // Fonbet: Under 2.5 @ 2.05
    let odds = vec![
        make_odd("o4", "evt2", "pari", "Total", "Over", 2.05, Some(2.5)),
        make_odd("o5", "evt2", "fonbet", "Total", "Under", 2.05, Some(2.5)),
    ];

    let calc = SurebetCalculator::new(0.5, 30.0, 1000.0, 10000, 0.01);
    let surebets = calc.find_surebets(&[event], &odds);

    println!("  Odds: Pari:Over2.5@2.05, Fonbet:Under2.5@2.05");
    println!("  Found surebets: {}", surebets.len());

    if !surebets.is_empty() {
        let sb = &surebets[0];
        println!("  ✅ Surebet found! Profit: {:.2}%", sb.profit_percent);
    } else {
        println!("  ❌ No surebet found");
    }

    assert!(!surebets.is_empty(), "Should find cross-BK total surebet");
}

#[test]
fn test_06_normalizer_team_matching() {
    println!("\n🧪 ТЕСТ 6: Нормализатор команд");

    let norm = Normalizer::new();

    let teams = vec![
        ("Манчестер Юнайтед", "Manchester United"),
        ("Манчестер Сити", "Manchester City"),
        ("Реал Мадрид", "Real Madrid"),
        ("Барселона", "Barcelona"),
        ("Зенит", "Zenit"),
        ("Спартак", "Spartak Moscow"),
        ("ЦСКА", "CSKA Moscow"),
        ("Локомотив", "Lokomotiv Moscow"),
    ];

    for (input, expected_contains) in &teams {
        let normalized = norm.normalize_team(input);
        println!(
            "  '{}' → '{}' (contains '{}': {})",
            input,
            normalized,
            expected_contains,
            normalized.contains(expected_contains)
                || normalized
                    .to_lowercase()
                    .contains(&expected_contains.to_lowercase())
        );
    }
}

#[test]
fn test_07_league_normalization() {
    println!("\n🧪 ТЕСТ 7: Нормализация лиг через Normalizer");

    let norm = Normalizer::new();

    let leagues = vec![
        ("АПЛ", "Premier League"),
        ("Английская Премьер-Лига", "Premier League"),
        ("EPL", "Premier League"),
        ("англия", "Premier League"),
        ("РПЛ", "Russian Premier League"),
        ("Russian Premier League", "Russian Premier League"),
        ("Россия", "Russian Premier League"),
        ("Ла Лига", "La Liga"),
        ("Primera Division", "La Liga"),
        ("Испания", "La Liga"),
        ("ЛЧ", "UEFA Champions League"),
        ("Champions League", "UEFA Champions League"),
    ];

    for (input, expected) in &leagues {
        let normalized = norm.normalize_league(input);
        let pass = normalized == *expected;
        println!(
            "  '{}' → '{}' (expected '{}': {})",
            input,
            normalized,
            expected,
            if pass { "✅" } else { "❌" }
        );
        assert_eq!(
            normalized, *expected,
            "League '{}' should normalize to '{}'",
            input, expected
        );
    }
    println!("  ✅ All {} league normalizations passed!", leagues.len());
}

#[test]
fn test_08_full_pipeline_simulation() {
    println!("\n🧪 ТЕСТ 8: Полная симуляция pipeline");

    // Создаём события от 3 БК для одного матча
    let events = vec![
        make_event("pari_100", "pari", "Реал Мадрид", "Барселона", "Ла Лига"),
        make_event(
            "fonbet_200",
            "fonbet",
            "Real Madrid",
            "Barcelona",
            "Испания",
        ),
        make_event(
            "marathon_300",
            "marathon",
            "Реал",
            "Барселона ",
            "Primera Division",
        ),
    ];

    println!("  Events:");
    let mut fingerprints = Vec::new();
    for ev in &events {
        let fp = event_fingerprint(ev);
        fingerprints.push(fp.clone());
        println!(
            "    {} ({}): {} vs {} → {}",
            ev.bookmaker_slug, ev.id, ev.home_team, ev.away_team, fp
        );
    }

    // Проверяем что все fingerprint совпадают
    let unique_fps: std::collections::HashSet<_> = fingerprints.iter().collect();
    println!("  Unique fingerprints: {}", unique_fps.len());
    assert_eq!(unique_fps.len(), 1, "All events must have same fingerprint");
    println!("  ✅ All events matched!");

    // Создаём odds от разных БК
    let all_odds = vec![
        // Pari: 1X2
        make_odd("o1", "pari_100", "pari", "1X2", "1", 2.10, None),
        make_odd("o2", "pari_100", "pari", "1X2", "X", 3.50, None),
        // Fonbet: 1X2
        make_odd("o3", "fonbet_200", "fonbet", "1X2", "1", 2.05, None),
        make_odd("o4", "fonbet_200", "fonbet", "1X2", "X", 3.80, None),
        make_odd("o5", "fonbet_200", "fonbet", "1X2", "2", 3.90, None),
        // Marathon: 1X2
        make_odd("o6", "marathon_300", "marathon", "1X2", "1", 2.00, None),
        make_odd("o7", "marathon_300", "marathon", "1X2", "2", 4.20, None),
    ];

    println!("\n  Odds from {} bookmakers:", unique_fps.len());
    for odd in &all_odds {
        println!("    {} {}@{}", odd.bookmaker_slug, odd.selection, odd.odds);
    }

    // Группируем по fingerprint (как делает сканнер)
    let mut matches: HashMap<String, Vec<&Event>> = HashMap::new();
    for event in &events {
        let fp = event_fingerprint(event);
        matches.entry(fp).or_default().push(event);
    }

    let mut odds_by_match: HashMap<String, Vec<&Odd>> = HashMap::new();
    for odd in &all_odds {
        if let Some(event) = events.iter().find(|e| e.id == odd.event_id) {
            let fp = event_fingerprint(event);
            odds_by_match.entry(fp).or_default().push(odd);
        }
    }

    println!("\n  Matched groups: {}", matches.len());
    for (fp, match_events) in &matches {
        let odds = odds_by_match.get(fp).cloned().unwrap_or_default();
        let bks: std::collections::HashSet<_> =
            odds.iter().map(|o| o.bookmaker_slug.as_str()).collect();
        println!(
            "    FP: {} ({} events, {} odds from {:?})",
            fp,
            match_events.len(),
            odds.len(),
            bks
        );

        // Теперь ищем вилки
        let norm = Normalizer::new();
        let norm_events: Vec<Event> = match_events
            .iter()
            .map(|e| norm.normalize_event((**e).clone()))
            .collect();

        let calc = SurebetCalculator::new(0.5, 30.0, 1000.0, 10000, 0.01);
        let surebets = calc.find_surebets(
            &norm_events,
            &odds.iter().map(|o| (**o).clone()).collect::<Vec<_>>(),
        );

        println!("    Surebets found: {}", surebets.len());
        for sb in &surebets {
            println!("      💰 Profit: {:.2}%", sb.profit_percent);
            for leg in &sb.legs {
                println!(
                    "        - {} {}@{:.2} stake={:.2}",
                    leg.bookmaker, leg.selection, leg.odds, leg.stake
                );
            }
        }
    }
}

#[test]
fn test_09_detect_real_world_naming_differences() {
    println!("\n🧪 ТЕСТ 9: Реальные различия в названиях БК");

    // Реальные примеры как БК называют команды
    let test_cases = vec![
        // Case 1: English vs Russian
        (
            make_event("p1", "pari", "Manchester United", "Liverpool", "EPL"),
            make_event("f1", "fonbet", "Манчестер Юнайтед", "Ливерпуль", "Англия"),
            true, // Should match
        ),
        // Case 2: With/without FC prefix
        (
            make_event("p2", "pari", "ФК Зенит", "Спартак Москва", "РПЛ"),
            make_event("m1", "marathon", "Зенит", "Спартак", "Россия"),
            true,
        ),
        // Case 3: Different city suffixes
        (
            make_event("f2", "fonbet", "ЦСКА Москва", "Локомотив Москва", "РПЛ"),
            make_event(
                "m2",
                "marathon",
                "ЦСКА",
                "Локомотив",
                "Российская Премьер-Лига",
            ),
            true,
        ),
        // Case 4: Different games entirely
        (
            make_event("p3", "pari", "Реал Мадрид", "Барселона", "Ла Лига"),
            make_event("f3", "fonbet", "Ювентус", "Милан", "Серия А"),
            false, // Should NOT match
        ),
    ];

    for (i, (ev1, ev2, should_match)) in test_cases.iter().enumerate() {
        let fp1 = event_fingerprint(ev1);
        let fp2 = event_fingerprint(ev2);
        let matches = fp1 == fp2;

        println!("  Case #{}:", i + 1);
        println!(
            "    A: {} vs {} ({})",
            ev1.home_team, ev1.away_team, ev1.league
        );
        println!("       → {}", fp1);
        println!(
            "    B: {} vs {} ({})",
            ev2.home_team, ev2.away_team, ev2.league
        );
        println!("       → {}", fp2);
        println!(
            "    Expected: {}, Got: {} {}",
            if *should_match { "MATCH" } else { "NO MATCH" },
            if matches { "MATCH" } else { "NO MATCH" },
            if matches == *should_match {
                "✅"
            } else {
                "❌"
            }
        );

        assert_eq!(matches, *should_match, "Case #{} matching failed", i + 1);
    }

    println!("  ✅ All {} test cases passed!", test_cases.len());
}
