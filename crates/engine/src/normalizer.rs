use once_cell::sync::Lazy;
use regex::Regex;
use shared::Event;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Структура для кэширования с TTL (24 часа = 86400 секунд)
#[derive(Clone, Debug)]
struct CachedValue<T: Clone> {
    value: T,
    timestamp: u64,
    ttl_secs: u64,
}

impl<T: Clone> CachedValue<T> {
    fn new(value: T, ttl_secs: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            value,
            timestamp,
            ttl_secs,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now - self.timestamp > self.ttl_secs
    }
}

// Кэш для результатов fuzzy matching с TTL 24 часа
static FUZZY_MATCH_CACHE: OnceLock<Mutex<HashMap<String, CachedValue<Option<String>>>>> =
    OnceLock::new();
// Кэш для пар команд с TTL 24 часа
static TEAM_PAIR_CACHE: OnceLock<Mutex<HashMap<String, CachedValue<(String, String)>>>> =
    OnceLock::new();

const CACHE_TTL_24H: u64 = 86400; // 24 часа в секундах

/// Получить или инициализировать кэш fuzzy matching
fn get_fuzzy_cache() -> &'static Mutex<HashMap<String, CachedValue<Option<String>>>> {
    FUZZY_MATCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Получить или инициализировать кэш пар команд
fn get_team_pair_cache() -> &'static Mutex<HashMap<String, CachedValue<(String, String)>>> {
    TEAM_PAIR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

static TEAM_ALIASES: Lazy<HashMap<&str, Vec<&str>>> = Lazy::new(|| {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();

    // ===== РОССИЙСКИЕ КОМАНДЫ (RPL) =====
    map.insert(
        "CSKA Moscow",
        vec![
            "ЦСКА",
            "ЦСКА Москва",
            "PFC CSKA",
            "ЦСКА М",
            "CSKA Moskva",
            "CSKA Moskva",
            "ЦСКА МСК",
            "ПФК ЦСКА",
        ],
    );
    map.insert(
        "Spartak Moscow",
        vec![
            "Спартак",
            "Спартак Москва",
            "FC Spartak",
            "Спартак М",
            "Spartak Moskva",
            "ФК Спартак",
        ],
    );
    map.insert(
        "Zenit",
        vec![
            "Зенит",
            "Зенит СПб",
            "FC Zenit",
            "Зенит Санкт-Петербург",
            "Зенит СПБ",
            "Zenit SPB",
            "ФК Зенит",
        ],
    );
    map.insert(
        "Lokomotiv Moscow",
        vec![
            "Локомотив",
            "Локо Москва",
            "FC Lokomotiv",
            "Локомотив М",
            "Lokomotiv Moskva",
            "ФК Локомотив",
            "Локо",
        ],
    );
    map.insert(
        "Dynamo Moscow",
        vec![
            "Динамо Москва",
            "Динамо М",
            "FC Dynamo",
            "Динамо",
            "Dynamo Moskva",
            "ФК Динамо",
        ],
    );
    map.insert(
        "Krasnodar",
        vec!["Краснодар", "FC Krasnodar", "ФК Краснодар", "Краснодар ФК"],
    );
    map.insert(
        "Rostov",
        vec!["Ростов", "FC Rostov", "FK Rostov", "ФК Ростов", "Ростов ФК"],
    );
    map.insert("Sochi", vec!["Сочи", "FC Sochi", "ФК Сочи", "Сочи ФК"]);
    map.insert(
        "Akhmat Grozny",
        vec!["Ахмат", "Ахмат Грозный", "FC Akhmat", "ФК Ахмат"],
    );
    map.insert("Ufa", vec!["Уфа", "FC Ufa", "ФК Уфа", "Уфа ФК"]);
    map.insert("Orenburg", vec!["Оренбург", "FC Orenburg", "ФК Оренбург"]);
    map.insert(
        "Nizhny Novgorod",
        vec!["Нижний Новгород", "FC Nizhny", "ФК Нижний"],
    );
    map.insert("Khimki", vec!["Химки", "FC Khimki", "ФК Химки"]);
    map.insert(
        "CSKA Sofia",
        vec!["ЦСКА София", "PFC CSKA Sofia", "ПФК ЦСКА София"],
    );
    map.insert("Pari NN", vec!["Пари НН", "Pari Nizhny", "Пари"]);

    // ===== ФУТБОЛ (обобщённый English) =====
    // Испанские
    map.insert(
        "Real Madrid",
        vec![
            "Реал Мадрид",
            "Реал",
            "Real Madrid CF",
            "Real",
            "Real Madrid",
        ],
    );
    map.insert(
        "Barcelona",
        vec!["Барселона", "Барса", "FC Barcelona", "Barça", "Барса"],
    );
    map.insert(
        "Atletico Madrid",
        vec![
            "Атлетико",
            "Атлетико Мадрид",
            "Atlético",
            "Atletico Madridf",
        ],
    );
    map.insert("Sevilla", vec!["Севилья", "FC Sevilla"]);
    map.insert("Valencia", vec!["Валенсия", "CF Valencia"]);
    map.insert("Bilbao", vec!["Бильбао", "Athletic Bilbao"]);

    // Английские
    map.insert(
        "Manchester United",
        vec![
            "Манчестер Юнайтед",
            "Ман Юнайтед",
            "Man Utd",
            "MUFC",
            "Manchester Utd",
        ],
    );
    map.insert(
        "Manchester City",
        vec!["Манчестер Сити", "Ман Сити", "Man City", "MCFC"],
    );
    map.insert("Liverpool", vec!["Ливерпуль", "LFC", "Лив"]);
    map.insert("Chelsea", vec!["Челси", "CFC", "Челсі"]);
    map.insert("Arsenal", vec!["Арсенал", "AFC", "Арсенал Лондон"]);
    map.insert("Tottenham", vec!["Тоттенхэм", "Spurs", "Тоттенхэм Хотспур"]);
    map.insert("Newcastle United", vec!["Ньюкасл", "Newcastle", "NUFC"]);
    map.insert("Brighton", vec!["Брайтон", "Brighton and Hove"]);
    map.insert("Aston Villa", vec!["Астон Вилла", "Aston Villa"]);
    map.insert("Everton", vec!["Эвертон", "Everton FC"]);
    map.insert("Fulham", vec!["Фулхэм", "Fulham FC"]);
    map.insert("Brentford", vec!["Брентфорд", "Brentford FC"]);
    map.insert("Bournemouth", vec!["Борнмут", "Bournemouth AFC"]);
    map.insert("West Ham", vec!["Вест Хэм", "West Ham United"]);

    // Немецкие
    map.insert(
        "Bayern Munich",
        vec![
            "Бавария",
            "Bayern",
            "FC Bayern",
            "Бавария Мюнхен",
            "Bayern Munchen",
        ],
    );
    map.insert(
        "Borussia Dortmund",
        vec![
            "Боруссия Дортмунд",
            "BVB",
            "Боруссия Д",
            "Borussia Dortmund",
        ],
    );
    map.insert("RB Leipzig", vec!["РБ Лейпциг", "Leipzig", "Лейпциг"]);
    map.insert("Schalke 04", vec!["Шальке 04", "Schalke"]);
    map.insert("Werder Bremen", vec!["Вердер Бремен", "Bremen"]);
    map.insert(
        "Eintracht Frankfurt",
        vec!["Айнтрахт Франкфурт", "Frankfurt"],
    );

    // Французские
    map.insert(
        "PSG",
        vec![
            "ПСЖ",
            "Paris Saint-Germain",
            "Пари Сен-Жермен",
            "Париж",
            "PSG",
            "Paris SG",
        ],
    );
    map.insert(
        "Olympique Marseille",
        vec!["Олимпик Марсель", "Марсель", "OM", "Olympique Marseille"],
    );
    map.insert("AS Monaco", vec!["AS Монако", "Monaco", "Монако"]);
    map.insert("Rennes", vec!["Ренн", "Rennes FC"]);

    // Итальянские
    map.insert("Juventus", vec!["Ювентус", "Juve", "Juventus"]);
    map.insert("AC Milan", vec!["Милан", "ACM", "AC Milan"]);
    map.insert(
        "Inter Milan",
        vec![
            "Интер",
            "Inter",
            "Интер Милан",
            "Inter Milan",
            "FC Internazionale",
        ],
    );
    map.insert("AS Roma", vec!["Рома", "AS Roma", "А Рома", "Roma"]);
    map.insert("Napoli", vec!["Наполи", "SSC Napoli", "Napoli"]);
    map.insert("Lazio", vec!["Лацио", "SS Lazio"]);
    map.insert("Fiorentina", vec!["Фиорентина", "ACF Fiorentina"]);

    // Португальские
    map.insert("Benfica", vec!["Бенфика", "SL Benfica"]);
    map.insert("Porto", vec!["Порту", "FC Porto"]);
    map.insert("Sporting", vec!["Спортинг", "Sporting CP"]);

    // Нидерланды
    map.insert("Ajax", vec!["Аякс", "AFC Ajax"]);
    map.insert("PSV", vec!["ПСВ", "PSV Eindhoven"]);
    map.insert("Feyenoord", vec!["Фейеноорд", "Feyenoord"]);

    // Другое
    map.insert("Alaves", vec!["Алавес", "Deportivo Alaves", "Alavés"]);
    map.insert(
        "Juventud Las Piedras",
        vec!["Ювентуд", "Juventud", "CA Juventud"],
    );

    map
});

static CLEANUP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Zа-яА-Я0-9\s\-]").unwrap());
static EXTRA_SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

// ===== LEAGUE VARIATIONS MAP (ALL RUSSIAN LEAGUES) =====
static LEAGUE_VARIATIONS: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    let mut map: HashMap<&str, &str> = HashMap::new();

    // RPL / Russian Premier League
    map.insert("рпл", "Russian Premier League");
    map.insert("rpl", "Russian Premier League");
    map.insert("russian premier league", "Russian Premier League");
    map.insert("россия", "Russian Premier League");
    map.insert("российская премьер-лига", "Russian Premier League");
    map.insert("российская премьер лига", "Russian Premier League");
    map.insert("премьер лига", "Russian Premier League");
    map.insert("премьер-лига", "Russian Premier League");
    map.insert("premier league ru", "Russian Premier League");

    // FNL / Russian Football National League
    map.insert("fnl", "Russian Football National League");
    map.insert("fnl1", "Russian Football National League");
    map.insert("fnl-1", "Russian Football National League");
    map.insert("фнл", "Russian Football National League");
    map.insert("национальная лига", "Russian Football National League");
    map.insert(
        "футбольная национальная лига",
        "Russian Football National League",
    );
    map.insert("fnl - первый дивизион", "Russian Football National League");

    // Russian Cup
    map.insert("кубок россии", "Russian Cup");
    map.insert("russian cup", "Russian Cup");
    map.insert("кубок рф", "Russian Cup");
    map.insert("кубок", "Russian Cup");

    // English Premier League
    map.insert("апл", "Premier League");
    map.insert("epl", "Premier League");
    map.insert("premier league", "Premier League");
    map.insert("english premier league", "Premier League");
    map.insert("англия", "Premier League");
    map.insert("английская премьер-лига", "Premier League");
    map.insert("английская премьер лига", "Premier League");
    map.insert("англ пл", "Premier League");

    // Championship (English 2nd Division)
    map.insert("championship", "Championship");
    map.insert("чемпионшип", "Championship");
    map.insert("англия 2", "Championship");
    map.insert("영어 championship", "Championship");

    // Spanish La Liga
    map.insert("ла лига", "La Liga");
    map.insert("la liga", "La Liga");
    map.insert("primera division", "La Liga");
    map.insert("испания", "La Liga");
    map.insert("примера", "La Liga");
    map.insert("ла-лига", "La Liga");

    // German Bundesliga
    map.insert("бундеслига", "Bundesliga");
    map.insert("bundesliga", "Bundesliga");
    map.insert("германия", "Bundesliga");
    map.insert("нем пл", "Bundesliga");
    map.insert("bundesliga 1", "Bundesliga");

    // Bundesliga 2
    map.insert("бундеслига 2", "Bundesliga 2");
    map.insert("bundesliga 2", "Bundesliga 2");
    map.insert("германия 2", "Bundesliga 2");

    // Italian Serie A
    map.insert("серия а", "Serie A");
    map.insert("serie a", "Serie A");
    map.insert("италия", "Serie A");
    map.insert("серия-а", "Serie A");
    map.insert("serie-a", "Serie A");

    // Serie B
    map.insert("серия б", "Serie B");
    map.insert("serie b", "Serie B");
    map.insert("италия 2", "Serie B");

    // French Ligue 1
    map.insert("лига 1", "Ligue 1");
    map.insert("ligue 1", "Ligue 1");
    map.insert("франция", "Ligue 1");
    map.insert("l1", "Ligue 1");
    map.insert("лиге 1", "Ligue 1");
    map.insert("лиге-1", "Ligue 1");

    // Ligue 2
    map.insert("лига 2", "Ligue 2");
    map.insert("ligue 2", "Ligue 2");
    map.insert("франция 2", "Ligue 2");

    // UEFA Champions League
    map.insert("лч", "UEFA Champions League");
    map.insert("ucl", "UEFA Champions League");
    map.insert("champions league", "UEFA Champions League");
    map.insert("uefa champions league", "UEFA Champions League");
    map.insert("лига чемпионов", "UEFA Champions League");
    map.insert("лига-чемпионов", "UEFA Champions League");
    map.insert("cl", "UEFA Champions League");

    // UEFA Europa League
    map.insert("ле", "UEFA Europa League");
    map.insert("uel", "UEFA Europa League");
    map.insert("europa league", "UEFA Europa League");
    map.insert("uefa europa league", "UEFA Europa League");
    map.insert("лига европы", "UEFA Europa League");
    map.insert("лига-европы", "UEFA Europa League");

    // UEFA Conference League
    map.insert("лк", "UEFA Conference League");
    map.insert("uecl", "UEFA Conference League");
    map.insert("conference league", "UEFA Conference League");
    map.insert("лига конференций", "UEFA Conference League");

    // International
    map.insert("world cup", "FIFA World Cup");
    map.insert("чемпионат мира", "FIFA World Cup");
    map.insert("世界杯", "FIFA World Cup");
    map.insert("euro", "UEFA Euro");
    map.insert("евро", "UEFA Euro");
    map.insert("copa america", "Copa America");
    map.insert("копа америка", "Copa America");

    // Friendly / Others
    map.insert("friendly", "Friendly");
    map.insert("товарищеский", "Friendly");
    map.insert("friendly match", "Friendly");

    map
});

// ===== SPORT VARIATIONS MAP (Fuzzy matching for sport names) =====
static SPORT_VARIATIONS: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    let mut map: HashMap<&str, &str> = HashMap::new();

    // Football
    map.insert("футбол", "Football");
    map.insert("football", "Football");
    map.insert("soccer", "Football");
    map.insert("soccer", "Football");
    map.insert("foot", "Football");
    map.insert("фут", "Football");

    // Basketball
    map.insert("баскетбол", "Basketball");
    map.insert("basketball", "Basketball");
    map.insert("basket", "Basketball");
    map.insert("баск", "Basketball");

    // Tennis
    map.insert("теннис", "Tennis");
    map.insert("tennis", "Tennis");
    map.insert("тен", "Tennis");

    // Ice Hockey
    map.insert("хоккей", "Ice Hockey");
    map.insert("hockey", "Ice Hockey");
    map.insert("ice hockey", "Ice Hockey");
    map.insert("хок", "Ice Hockey");

    // Volleyball
    map.insert("волейбол", "Volleyball");
    map.insert("volleyball", "Volleyball");
    map.insert("волей", "Volleyball");

    // Handball
    map.insert("гандбол", "Handball");
    map.insert("handball", "Handball");

    // American Football
    map.insert("американский футбол", "American Football");
    map.insert("american football", "American Football");
    map.insert("american football", "American Football");

    // Baseball
    map.insert("бейсбол", "Baseball");
    map.insert("baseball", "Baseball");

    // Esports
    map.insert("киберспорт", "Esports");
    map.insert("esports", "Esports");
    map.insert("e-sports", "Esports");
    map.insert("cs", "Esports");
    map.insert("dota", "Esports");

    map
});

/// Вычисление расстояния Левенштейна (оптимизированное)
#[inline]
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Используем одну строку вместо полной матрицы для оптимизации памяти
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 0..=n {
        prev[i] = i;
    }

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = std::cmp::min(
                std::cmp::min(curr[j - 1] + 1, prev[j] + 1),
                prev[j - 1] + cost,
            );
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Вычисление процента сходства на основе Levenshtein distance
/// Возвращает значение от 0 до 100 (процент сходства)
#[inline]
fn similarity_percentage(distance: usize, max_len: usize) -> f64 {
    if max_len == 0 {
        return 100.0;
    }
    let similarity = 1.0 - (distance as f64 / max_len as f64);
    let pct = (similarity * 100.0).max(0.0);
    // Keep deterministic comparisons in tests and avoid float tails like 19.999999999.
    (pct * 1_000_000.0).round() / 1_000_000.0
}

/// Проверка fuzzy совпадения с порогом расстояния (legacy, оставлена для совместимости)
#[allow(dead_code)]
fn fuzzy_match(input: &str, candidates: &[&str], max_dist: usize) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut best = None;
    let mut best_dist = usize::MAX;

    for candidate in candidates {
        let cand_lower = candidate.to_lowercase();
        let dist = levenshtein(&input_lower, &cand_lower);
        if dist <= max_dist && dist < best_dist {
            best_dist = dist;
            best = Some(candidate.to_string());
        }
    }

    best
}

/// Нечёткое совпадение команды с порогом 85% сходства
/// Использует Levenshtein distance и кэширует результаты с TTL 24h
pub fn fuzzy_match_team(
    input: &str,
    candidates: &[(&str, &str)],
    threshold: f64,
) -> Option<String> {
    if input.is_empty() || candidates.is_empty() {
        return None;
    }

    let input_lower = input.to_lowercase();

    // Проверяем кэш с TTL
    let cache = get_fuzzy_cache();
    let cache_key = format!("{}::{}", input_lower, threshold as u32);

    if let Ok(mut cache_guard) = cache.lock() {
        if let Some(cached) = cache_guard.get(&cache_key) {
            if !cached.is_expired() {
                return cached.value.clone();
            } else {
                // Удаляем истёкший кэш
                cache_guard.remove(&cache_key);
            }
        }
    }

    let mut best_match: Option<String> = None;
    let mut best_similarity = 0.0;
    for (candidate, canonical) in candidates {
        let cand_lower = candidate.to_lowercase();
        let max_len = input_lower.len().max(cand_lower.len());
        let distance = levenshtein(&input_lower, &cand_lower);
        let similarity = similarity_percentage(distance, max_len);

        if similarity >= threshold && similarity > best_similarity {
            best_similarity = similarity;
            best_match = Some(canonical.to_string());
        }

        // Short-hand token inputs like "spartak s" should still map to the canonical alias.
        if input_lower.len() >= 5
            && (cand_lower.starts_with(&input_lower) || input_lower.starts_with(&cand_lower))
        {
            let contains_similarity = 90.0;
            if contains_similarity >= threshold && contains_similarity > best_similarity {
                best_similarity = contains_similarity;
                best_match = Some(canonical.to_string());
            }
        }
    }

    // Кэшируем результат с TTL 24h
    if let Ok(mut cache_guard) = cache.lock() {
        cache_guard.insert(
            cache_key,
            CachedValue::new(best_match.clone(), CACHE_TTL_24H),
        );
    }

    best_match
}

/// Fuzzy matching для лиг с TTL кэшированием
fn fuzzy_match_league(input: &str, threshold: f64) -> Option<String> {
    if input.is_empty() {
        return None;
    }

    let input_lower = input.to_lowercase();

    // Проверяем точное совпадение сначала
    if let Some(&canonical) = LEAGUE_VARIATIONS.get(input_lower.as_str()) {
        return Some(canonical.to_string());
    }

    let mut best_match: Option<String> = None;
    let mut best_similarity = 0.0;
    let max_len = input_lower.len();

    for (league_name, canonical) in LEAGUE_VARIATIONS.iter() {
        let distance = levenshtein(&input_lower, league_name);
        let similarity = similarity_percentage(distance, max_len.max(league_name.len()));

        if similarity >= threshold && similarity > best_similarity {
            best_similarity = similarity;
            best_match = Some(canonical.to_string());
        }
    }

    best_match
}

/// Fuzzy matching для спортов с TTL кэшированием
fn fuzzy_match_sport(input: &str, threshold: f64) -> Option<String> {
    if input.is_empty() {
        return None;
    }

    let input_lower = input.to_lowercase();

    // Проверяем точное совпадение сначала
    if let Some(&canonical) = SPORT_VARIATIONS.get(input_lower.as_str()) {
        return Some(canonical.to_string());
    }

    let mut best_match: Option<String> = None;
    let mut best_similarity = 0.0;
    let max_len = input_lower.len();

    for (sport_name, canonical) in SPORT_VARIATIONS.iter() {
        let distance = levenshtein(&input_lower, sport_name);
        let similarity = similarity_percentage(distance, max_len.max(sport_name.len()));

        if similarity >= threshold && similarity > best_similarity {
            best_similarity = similarity;
            best_match = Some(canonical.to_string());
        }
    }

    best_match
}

/// Кэширование пары команд с TTL 24h
fn cache_team_pair(home: &str, away: &str, normalized_home: &str, normalized_away: &str) {
    let cache = get_team_pair_cache();
    let cache_key = format!("{}vs{}", home.to_lowercase(), away.to_lowercase());

    if let Ok(mut cache_guard) = cache.lock() {
        cache_guard.insert(
            cache_key,
            CachedValue::new(
                (normalized_home.to_string(), normalized_away.to_string()),
                CACHE_TTL_24H,
            ),
        );
    }
}

/// Получение пары команд из кэша с проверкой TTL
fn get_cached_team_pair(home: &str, away: &str) -> Option<(String, String)> {
    let cache = get_team_pair_cache();
    let cache_key = format!("{}vs{}", home.to_lowercase(), away.to_lowercase());

    if let Ok(mut cache_guard) = cache.lock() {
        if let Some(cached) = cache_guard.get(&cache_key) {
            if !cached.is_expired() {
                return Some(cached.value.clone());
            } else {
                // Удаляем истёкший кэш
                cache_guard.remove(&cache_key);
            }
        }
    }

    None
}

#[derive(Clone)]
pub struct Normalizer {
    aliases: HashMap<String, String>,
}

impl Normalizer {
    pub fn new() -> Self {
        let mut aliases: HashMap<String, String> = HashMap::new();
        for (canonical, alias_list) in TEAM_ALIASES.iter() {
            for alias in alias_list.iter() {
                aliases.insert(alias.to_lowercase(), canonical.to_string());
            }
            aliases.insert(canonical.to_lowercase(), canonical.to_string());
        }

        Self { aliases }
    }

    pub fn normalize_team(&self, team: &str) -> String {
        let cleaned = self.clean_team_name(team);
        let lower = cleaned.to_lowercase();

        // 1. Точное совпадение
        if let Some(canonical) = self.aliases.get(&lower) {
            return canonical.clone();
        }

        // 2. Частичное совпадение (contains)
        for (alias, canonical) in &self.aliases {
            if lower.len() >= 3
                && alias.len() >= 3
                && (lower.contains(alias) || alias.contains(&lower))
            {
                return canonical.clone();
            }
        }

        // 3. Fuzzy matching с 85% порогом сходства
        // Подготавливаем кандидатов: (alias, canonical)
        let candidates: Vec<(&str, &str)> = self
            .aliases
            .iter()
            .map(|(alias, canonical)| (alias.as_str(), canonical.as_str()))
            .collect();

        if let Some(fuzzy_match) = fuzzy_match_team(&lower, &candidates, 85.0) {
            return fuzzy_match;
        }

        cleaned
    }

    /// Нормализация спорта с fuzzy matching (футбол = football, хоккей = ice hockey)
    pub fn normalize_sport(&self, sport: &str) -> String {
        let lower = sport.trim().to_lowercase();

        // Точное совпадение
        if let Some(&canonical) = SPORT_VARIATIONS.get(lower.as_str()) {
            return canonical.to_string();
        }

        // Fuzzy matching с 80% порогом
        if let Some(fuzzy_match) = fuzzy_match_sport(&lower, 80.0) {
            return fuzzy_match;
        }

        // Fallback: return original trimmed
        sport.trim().to_string()
    }

    pub fn normalize_event(&self, event: Event) -> Event {
        // Получаем нормализованные команды из кэша или вычисляем их
        let (normalized_home, normalized_away) =
            if let Some(cached) = get_cached_team_pair(&event.home_team, &event.away_team) {
                cached
            } else {
                let home = self.normalize_team(&event.home_team);
                let away = self.normalize_team(&event.away_team);
                cache_team_pair(&event.home_team, &event.away_team, &home, &away);
                (home, away)
            };

        Event {
            id: event.id,
            sport: event.sport,
            league: self.normalize_league(&event.league),
            home_team: normalized_home,
            away_team: normalized_away,
            start_time: event.start_time,
            is_live: event.is_live,
            bookmaker_slug: event.bookmaker_slug,
            raw_url: event.raw_url,
            extra: event.extra,
        }
    }

    pub fn normalize_league(&self, league: &str) -> String {
        let lower = league.trim().to_lowercase();

        // Точное совпадение в LEAGUE_VARIATIONS
        if let Some(&canonical) = LEAGUE_VARIATIONS.get(lower.as_str()) {
            return canonical.to_string();
        }

        // Fuzzy matching с 80% порогом для лиг
        if let Some(fuzzy_match) = fuzzy_match_league(&lower, 80.0) {
            return fuzzy_match;
        }

        // Fallback: return original trimmed
        league.trim().to_string()
    }

    pub fn events_match(&self, event_a: &Event, event_b: &Event) -> bool {
        if event_a.sport != event_b.sport {
            return false;
        }

        let home_a = self.normalize_team(&event_a.home_team);
        let away_a = self.normalize_team(&event_a.away_team);
        let home_b = self.normalize_team(&event_b.home_team);
        let away_b = self.normalize_team(&event_b.away_team);

        (home_a == home_b && away_a == away_b) || (home_a == away_b && away_a == home_b)
    }

    fn clean_team_name(&self, name: &str) -> String {
        let name = CLEANUP_RE.replace_all(name, "");
        let name = EXTRA_SPACE_RE.replace_all(&name, " ");
        name.trim().to_string()
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Sport;
    use std::collections::HashMap;

    fn make_event(home: &str, away: &str) -> Event {
        Event {
            id: "test".into(),
            sport: Sport::Football,
            league: "Test".into(),
            home_team: home.into(),
            away_team: away.into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    // ===== TEAM NORMALIZATION TESTS (10 tests) =====
    #[test]
    fn test_normalize_team_russian() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Реал Мадрид"), "Real Madrid");
        assert_eq!(norm.normalize_team("Барселона"), "Barcelona");
        assert_eq!(norm.normalize_team("Зенит"), "Zenit");
    }

    #[test]
    fn test_normalize_team_english() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Man Utd"), "Manchester United");
        assert_eq!(norm.normalize_team("Man City"), "Manchester City");
    }

    #[test]
    fn test_normalize_team_case_insensitive() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("real"), "Real Madrid");
        assert_eq!(norm.normalize_team("REAL MADRID"), "Real Madrid");
        assert_eq!(norm.normalize_team("MAN UTD"), "Manchester United");
    }

    #[test]
    fn test_normalize_team_contains() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Барса"), "Barcelona");
        assert_eq!(norm.normalize_team("LFC"), "Liverpool");
        assert_eq!(norm.normalize_team("CFC"), "Chelsea");
    }

    #[test]
    fn test_normalize_russian_teams() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("ЦСКА"), "CSKA Moscow");
        assert_eq!(norm.normalize_team("Спартак"), "Spartak Moscow");
        assert_eq!(norm.normalize_team("Локомотив"), "Lokomotiv Moscow");
        assert_eq!(norm.normalize_team("Динамо Москва"), "Dynamo Moscow");
    }

    #[test]
    fn test_normalize_russian_teams_full_names() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("ЦСКА Москва"), "CSKA Moscow");
        assert_eq!(norm.normalize_team("ПФК ЦСКА"), "CSKA Moscow");
        assert_eq!(norm.normalize_team("FC Lokomotiv"), "Lokomotiv Moscow");
        assert_eq!(norm.normalize_team("ФК Зенит"), "Zenit");
    }

    #[test]
    fn test_normalize_new_russian_teams() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Краснодар"), "Krasnodar");
        assert_eq!(norm.normalize_team("Ростов"), "Rostov");
        assert_eq!(norm.normalize_team("Ахмат"), "Akhmat Grozny");
        assert_eq!(norm.normalize_team("Уфа"), "Ufa");
    }

    #[test]
    fn test_normalize_english_teams() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Liverpool"), "Liverpool");
        assert_eq!(norm.normalize_team("Chelsea"), "Chelsea");
        assert_eq!(norm.normalize_team("Arsenal"), "Arsenal");
        assert_eq!(norm.normalize_team("Tottenham"), "Tottenham");
    }

    #[test]
    fn test_normalize_special_chars() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Team@#$%"), "Team");
        assert_eq!(norm.normalize_team("Real  Madrid"), "Real Madrid");
    }

    #[test]
    fn test_normalize_team_abbreviations() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("MUFC"), "Manchester United");
        assert_eq!(norm.normalize_team("MCFC"), "Manchester City");
    }

    // ===== LEAGUE NORMALIZATION TESTS (15 tests) =====
    #[test]
    fn test_normalize_league_rpl() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("рпл"), "Russian Premier League");
        assert_eq!(norm.normalize_league("RPL"), "Russian Premier League");
        assert_eq!(
            norm.normalize_league("russian premier league"),
            "Russian Premier League"
        );
        assert_eq!(
            norm.normalize_league("российская премьер-лига"),
            "Russian Premier League"
        );
    }

    #[test]
    fn test_normalize_league_fnl() {
        let norm = Normalizer::new();
        assert_eq!(
            norm.normalize_league("fnl"),
            "Russian Football National League"
        );
        assert_eq!(
            norm.normalize_league("ФНЛ"),
            "Russian Football National League"
        );
        assert_eq!(
            norm.normalize_league("национальная лига"),
            "Russian Football National League"
        );
    }

    #[test]
    fn test_normalize_league_russian_cup() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("кубок россии"), "Russian Cup");
        assert_eq!(norm.normalize_league("russian cup"), "Russian Cup");
        assert_eq!(norm.normalize_league("кубок"), "Russian Cup");
    }

    #[test]
    fn test_normalize_league_epl() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("апл"), "Premier League");
        assert_eq!(norm.normalize_league("EPL"), "Premier League");
        assert_eq!(norm.normalize_league("premier league"), "Premier League");
        assert_eq!(norm.normalize_league("англия"), "Premier League");
    }

    #[test]
    fn test_normalize_league_championship() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("championship"), "Championship");
        assert_eq!(norm.normalize_league("чемпионшип"), "Championship");
    }

    #[test]
    fn test_normalize_league_la_liga() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("ла лига"), "La Liga");
        assert_eq!(norm.normalize_league("la liga"), "La Liga");
        assert_eq!(norm.normalize_league("примера"), "La Liga");
        assert_eq!(norm.normalize_league("испания"), "La Liga");
    }

    #[test]
    fn test_normalize_league_bundesliga() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("бундеслига"), "Bundesliga");
        assert_eq!(norm.normalize_league("bundesliga"), "Bundesliga");
        assert_eq!(norm.normalize_league("германия"), "Bundesliga");
    }

    #[test]
    fn test_normalize_league_bundesliga2() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("бундеслига 2"), "Bundesliga 2");
        assert_eq!(norm.normalize_league("bundesliga 2"), "Bundesliga 2");
    }

    #[test]
    fn test_normalize_league_serie_a() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("серия а"), "Serie A");
        assert_eq!(norm.normalize_league("serie a"), "Serie A");
        assert_eq!(norm.normalize_league("италия"), "Serie A");
    }

    #[test]
    fn test_normalize_league_serie_b() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("серия б"), "Serie B");
        assert_eq!(norm.normalize_league("serie b"), "Serie B");
    }

    #[test]
    fn test_normalize_league_ligue1() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("лига 1"), "Ligue 1");
        assert_eq!(norm.normalize_league("ligue 1"), "Ligue 1");
        assert_eq!(norm.normalize_league("франция"), "Ligue 1");
    }

    #[test]
    fn test_normalize_league_ligue2() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("лига 2"), "Ligue 2");
        assert_eq!(norm.normalize_league("ligue 2"), "Ligue 2");
    }

    #[test]
    fn test_normalize_league_champions_league() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("лч"), "UEFA Champions League");
        assert_eq!(norm.normalize_league("UCL"), "UEFA Champions League");
        assert_eq!(
            norm.normalize_league("champions league"),
            "UEFA Champions League"
        );
        assert_eq!(
            norm.normalize_league("лига чемпионов"),
            "UEFA Champions League"
        );
    }

    #[test]
    fn test_normalize_league_europa_league() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("ле"), "UEFA Europa League");
        assert_eq!(norm.normalize_league("UEL"), "UEFA Europa League");
        assert_eq!(norm.normalize_league("europa league"), "UEFA Europa League");
        assert_eq!(norm.normalize_league("лига европы"), "UEFA Europa League");
    }

    #[test]
    fn test_normalize_league_conference_league() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_league("лк"), "UEFA Conference League");
        assert_eq!(norm.normalize_league("UECL"), "UEFA Conference League");
        assert_eq!(
            norm.normalize_league("conference league"),
            "UEFA Conference League"
        );
    }

    // ===== SPORT NORMALIZATION TESTS (8 tests) =====
    #[test]
    fn test_normalize_sport_football() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("футбол"), "Football");
        assert_eq!(norm.normalize_sport("football"), "Football");
        assert_eq!(norm.normalize_sport("soccer"), "Football");
        assert_eq!(norm.normalize_sport("foot"), "Football");
    }

    #[test]
    fn test_normalize_sport_basketball() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("баскетбол"), "Basketball");
        assert_eq!(norm.normalize_sport("basketball"), "Basketball");
        assert_eq!(norm.normalize_sport("basket"), "Basketball");
    }

    #[test]
    fn test_normalize_sport_tennis() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("теннис"), "Tennis");
        assert_eq!(norm.normalize_sport("tennis"), "Tennis");
    }

    #[test]
    fn test_normalize_sport_hockey() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("хоккей"), "Ice Hockey");
        assert_eq!(norm.normalize_sport("hockey"), "Ice Hockey");
        assert_eq!(norm.normalize_sport("ice hockey"), "Ice Hockey");
    }

    #[test]
    fn test_normalize_sport_volleyball() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("волейбол"), "Volleyball");
        assert_eq!(norm.normalize_sport("volleyball"), "Volleyball");
    }

    #[test]
    fn test_normalize_sport_handball() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("гандбол"), "Handball");
        assert_eq!(norm.normalize_sport("handball"), "Handball");
    }

    #[test]
    fn test_normalize_sport_american_football() {
        let norm = Normalizer::new();
        assert_eq!(
            norm.normalize_sport("американский футбол"),
            "American Football"
        );
        assert_eq!(
            norm.normalize_sport("american football"),
            "American Football"
        );
    }

    #[test]
    fn test_normalize_sport_esports() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_sport("киберспорт"), "Esports");
        assert_eq!(norm.normalize_sport("esports"), "Esports");
        assert_eq!(norm.normalize_sport("e-sports"), "Esports");
    }

    // ===== FUZZY MATCHING TESTS (8 tests) =====
    #[test]
    fn test_fuzzy_matching_typos_cska() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("CSKA Moskva");
        assert_eq!(
            result, "CSKA Moscow",
            "Should fuzzy match despite 'Moskva' typo"
        );
    }

    #[test]
    fn test_fuzzy_matching_typos_spartak() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Spartak S.");
        assert!(
            result.contains("Spartak"),
            "Should fuzzy match despite incomplete name"
        );
    }

    #[test]
    fn test_fuzzy_matching_manchester_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Манчестр Юнайтед");
        assert_eq!(
            result, "Manchester United",
            "Should fuzzy match Manchester United"
        );
    }

    #[test]
    fn test_fuzzy_matching_madrid_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Реал Мадри");
        assert_eq!(result, "Real Madrid", "Should fuzzy match Real Madrid");
    }

    #[test]
    fn test_fuzzy_matching_liverpool_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Liverpol");
        assert_eq!(result, "Liverpool", "Should fuzzy match Liverpool");
    }

    #[test]
    fn test_fuzzy_matching_barcelona_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Barselona");
        assert_eq!(result, "Barcelona", "Should fuzzy match Barcelona");
    }

    #[test]
    fn test_fuzzy_matching_chelsea_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Chalsea");
        assert_eq!(result, "Chelsea", "Should fuzzy match Chelsea");
    }

    #[test]
    fn test_fuzzy_matching_arsenal_typo() {
        let norm = Normalizer::new();
        let result = norm.normalize_team("Arsebal");
        assert_eq!(result, "Arsenal", "Should fuzzy match Arsenal");
    }

    // ===== LEVENSHTEIN DISTANCE TESTS (7 tests) =====
    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("kitten", "kitten"), 0);
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn test_levenshtein_classic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn test_levenshtein_one_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_single_char() {
        assert_eq!(levenshtein("a", "b"), 1);
        assert_eq!(levenshtein("a", "a"), 0);
    }

    #[test]
    fn test_levenshtein_case_sensitive() {
        assert_eq!(levenshtein("ABC", "abc"), 3);
        assert_eq!(levenshtein("Abc", "abc"), 1);
    }

    #[test]
    fn test_levenshtein_transposition() {
        assert_eq!(levenshtein("ab", "ba"), 2);
    }

    #[test]
    fn test_levenshtein_russian_chars() {
        assert_eq!(levenshtein("привет", "привет"), 0);
        assert_eq!(levenshtein("привет", "привет"), 0);
    }

    // ===== SIMILARITY PERCENTAGE TESTS (5 tests) =====
    #[test]
    fn test_similarity_perfect_match() {
        assert_eq!(similarity_percentage(0, 10), 100.0);
        assert_eq!(similarity_percentage(0, 1), 100.0);
    }

    #[test]
    fn test_similarity_half_distance() {
        assert_eq!(similarity_percentage(5, 10), 50.0);
    }

    #[test]
    fn test_similarity_zero_max_len() {
        assert_eq!(similarity_percentage(0, 0), 100.0);
    }

    #[test]
    fn test_similarity_high_distance() {
        assert_eq!(similarity_percentage(8, 10), 20.0);
    }

    #[test]
    fn test_similarity_85_percent_threshold() {
        assert!(similarity_percentage(1, 10) >= 85.0);
        assert!(similarity_percentage(2, 10) < 85.0);
    }

    // ===== FUZZY MATCH TEAM FUNCTION TESTS (5 tests) =====
    #[test]
    fn test_fuzzy_match_team_exact() {
        let candidates = vec![
            ("manchester united", "Manchester United"),
            ("real madrid", "Real Madrid"),
        ];
        let result = fuzzy_match_team("manchester united", &candidates, 85.0);
        assert_eq!(result, Some("Manchester United".to_string()));
    }

    #[test]
    fn test_fuzzy_match_team_typo() {
        let candidates = vec![
            ("manchester united", "Manchester United"),
            ("manchester city", "Manchester City"),
        ];
        let result = fuzzy_match_team("manchester untied", &candidates, 85.0);
        assert_eq!(result, Some("Manchester United".to_string()));
    }

    #[test]
    fn test_fuzzy_match_team_no_match() {
        let candidates = vec![
            ("manchester united", "Manchester United"),
            ("real madrid", "Real Madrid"),
        ];
        let result = fuzzy_match_team("xyz123", &candidates, 85.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_fuzzy_match_team_empty_input() {
        let candidates = vec![("manchester united", "Manchester United")];
        let result = fuzzy_match_team("", &candidates, 85.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_fuzzy_match_team_empty_candidates() {
        let candidates: Vec<(&str, &str)> = vec![];
        let result = fuzzy_match_team("manchester united", &candidates, 85.0);
        assert_eq!(result, None);
    }

    // ===== CACHE TTL TESTS (3 tests) =====
    #[test]
    fn test_fuzzy_match_team_caching() {
        let candidates = vec![("manchester united", "Manchester United")];
        let result1 = fuzzy_match_team("manchester untied", &candidates, 85.0);
        let result2 = fuzzy_match_team("manchester untied", &candidates, 85.0);
        assert_eq!(result1, result2);
        assert_eq!(result1, Some("Manchester United".to_string()));
    }

    #[test]
    fn test_fuzzy_match_team_different_thresholds() {
        let candidates = vec![("manchester united", "Manchester United")];
        let result_strict = fuzzy_match_team("xyz", &candidates, 90.0);
        let result_loose = fuzzy_match_team("manchester untied", &candidates, 50.0);
        assert_eq!(result_strict, None);
        assert!(result_loose.is_some());
    }

    #[test]
    fn test_fuzzy_match_team_case_insensitive() {
        let candidates = vec![("manchester united", "Manchester United")];
        let result_lower = fuzzy_match_team("manchester united", &candidates, 85.0);
        let result_upper = fuzzy_match_team("MANCHESTER UNITED", &candidates, 85.0);
        assert_eq!(result_lower, result_upper);
    }

    // ===== EVENT MATCHING TESTS (5 tests) =====
    #[test]
    fn test_events_match() {
        let norm = Normalizer::new();
        let event_a = make_event("Реал Мадрид", "Барселона");
        let event_b = make_event("Real Madrid", "Barcelona");
        assert!(norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_events_match_fuzzy() {
        let norm = Normalizer::new();
        let event_a = make_event("Реал Мадри", "Барселона");
        let event_b = make_event("Real Madrid", "Barcelona");
        assert!(norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_events_match_reversed() {
        let norm = Normalizer::new();
        let event_a = make_event("Real Madrid", "Barcelona");
        let event_b = make_event("Barcelona", "Real Madrid");
        assert!(norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_events_not_match_different_sport() {
        let norm = Normalizer::new();
        let mut event_a = make_event("Team A", "Team B");
        event_a.sport = Sport::Football;
        let mut event_b = make_event("Team A", "Team B");
        event_b.sport = Sport::Tennis;
        assert!(!norm.events_match(&event_a, &event_b));
    }

    #[test]
    fn test_events_not_match_different_teams() {
        let norm = Normalizer::new();
        let event_a = make_event("Real Madrid", "Barcelona");
        let event_b = make_event("Manchester United", "Liverpool");
        assert!(!norm.events_match(&event_a, &event_b));
    }

    // ===== INTEGRATION TESTS (6 tests) =====
    #[test]
    fn test_normalize_event_full() {
        let norm = Normalizer::new();
        let event = Event {
            id: "test".into(),
            sport: Sport::Football,
            league: "рпл".into(),
            home_team: "Реал Мадри".into(),
            away_team: "Барселона".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "test".into(),
            raw_url: None,
            extra: HashMap::new(),
        };

        let normalized = norm.normalize_event(event);
        assert_eq!(normalized.home_team, "Real Madrid");
        assert_eq!(normalized.away_team, "Barcelona");
        assert_eq!(normalized.league, "Russian Premier League");
    }

    #[test]
    fn test_accuracy_improvement_metrics() {
        let norm = Normalizer::new();

        let test_cases = vec![
            ("CSKA Moscow", "CSKA Moscow"),
            ("Spartak Moscow", "Spartak Moscow"),
            ("Zenit", "Zenit"),
            ("Real Madrid", "Real Madrid"),
            ("Barcelona", "Barcelona"),
            ("Manchester United", "Manchester United"),
            ("Manchester City", "Manchester City"),
            ("Liverpool", "Liverpool"),
            ("Chelsea", "Chelsea"),
            ("Arsenal", "Arsenal"),
            ("Bayern Munich", "Bayern Munich"),
            ("PSG", "PSG"),
            ("Juventus", "Juventus"),
            ("Inter Milan", "Inter Milan"),
        ];

        let mut correct = 0;
        for (input, expected) in test_cases.iter() {
            let result = norm.normalize_team(input);
            if result == *expected {
                correct += 1;
            }
        }

        let accuracy = (correct as f64 / test_cases.len() as f64) * 100.0;
        assert!(
            accuracy >= 99.0,
            "Expected 99%+ accuracy, got {:.1}%",
            accuracy
        );
    }

    #[test]
    fn test_comprehensive_team_coverage() {
        let norm = Normalizer::new();

        // Russian teams
        assert_eq!(norm.normalize_team("ЦСКА"), "CSKA Moscow");
        assert_eq!(norm.normalize_team("Краснодар"), "Krasnodar");

        // European teams
        assert_eq!(norm.normalize_team("Bayern"), "Bayern Munich");
        assert_eq!(norm.normalize_team("Rennes"), "Rennes");

        // Teams with multiple aliases
        assert_eq!(
            norm.normalize_team("Манчестер Юнайтед"),
            "Manchester United"
        );
    }

    #[test]
    fn test_league_coverage_comprehensive() {
        let norm = Normalizer::new();

        // Russian leagues
        assert_eq!(norm.normalize_league("рпл"), "Russian Premier League");
        assert_eq!(
            norm.normalize_league("фнл"),
            "Russian Football National League"
        );

        // European leagues
        assert_eq!(norm.normalize_league("апл"), "Premier League");
        assert_eq!(norm.normalize_league("ла лига"), "La Liga");
        assert_eq!(norm.normalize_league("серия а"), "Serie A");

        // International competitions
        assert_eq!(norm.normalize_league("лч"), "UEFA Champions League");
        assert_eq!(norm.normalize_league("лк"), "UEFA Conference League");
    }

    #[test]
    fn test_clean_team_name() {
        let norm = Normalizer::new();
        assert_eq!(norm.normalize_team("Team (FC)"), "Team FC");
        assert_eq!(norm.normalize_team("  Extra   Spaces  "), "Extra Spaces");
    }
}
