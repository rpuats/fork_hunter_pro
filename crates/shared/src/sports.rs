use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
pub enum Sport {
    #[strum(serialize = "football")]
    Football,
    #[strum(serialize = "tennis")]
    Tennis,
    #[strum(serialize = "basketball")]
    Basketball,
    #[strum(serialize = "hockey")]
    Hockey,
    #[strum(serialize = "volleyball")]
    Volleyball,
    #[strum(serialize = "table_tennis")]
    TableTennis,
    #[strum(serialize = "baseball")]
    Baseball,
    #[strum(serialize = "handball")]
    Handball,
    #[strum(serialize = "mma")]
    Mma,
    #[strum(serialize = "boxing")]
    Boxing,
    #[strum(serialize = "esports")]
    Esports,
    #[strum(serialize = "cricket")]
    Cricket,
    #[strum(serialize = "rugby")]
    Rugby,
    #[strum(serialize = "futsal")]
    Futsal,
    #[strum(serialize = "badminton")]
    Badminton,
    #[strum(serialize = "darts")]
    Darts,
    #[strum(serialize = "snooker")]
    Snooker,
    #[strum(serialize = "water_polo")]
    WaterPolo,
    #[strum(serialize = "aussie_rules")]
    AussieRules,
    #[strum(serialize = "beach_volleyball")]
    BeachVolleyball,
    #[strum(serialize = "floorball")]
    Floorball,
    #[strum(serialize = "golf")]
    Golf,
    #[strum(serialize = "motorsport")]
    Motorsport,
    #[strum(serialize = "cycling")]
    Cycling,
    #[strum(serialize = "winter_sports")]
    WinterSports,
    #[strum(serialize = "politics")]
    Politics,
    #[strum(serialize = "entertainment")]
    Entertainment,
    #[strum(serialize = "other")]
    Other,
}

impl Sport {
    pub fn from_str(s: &str) -> Self {
        let s = s.to_lowercase().trim().to_string();
        match s.as_str() {
            "football" | "soccer" | "футбол" => Sport::Football,
            "tennis" | "теннис" => Sport::Tennis,
            "basketball" | "баскетбол" | "баскет" => Sport::Basketball,
            "hockey" | "ice hockey" | "хоккей" | "хоккей с шайбой" => Sport::Hockey,
            "volleyball" | "волейбол" => Sport::Volleyball,
            "table tennis" | "настольный теннис" => Sport::TableTennis,
            "baseball" | "бейсбол" => Sport::Baseball,
            "handball" | "гандбол" => Sport::Handball,
            "mma" | "ufc" | "mixed martial arts" | "смешанные единоборства" => Sport::Mma,
            "boxing" | "бокс" => Sport::Boxing,
            "esports" | "киберспорт" | "e-sports" | "esport" => Sport::Esports,
            "cricket" | "крикет" => Sport::Cricket,
            "rugby" | "регби" | "rugby league" | "rugby union" => Sport::Rugby,
            "futsal" | "мини-футбол" => Sport::Futsal,
            "badminton" | "бадминтон" => Sport::Badminton,
            "darts" | "дартс" => Sport::Darts,
            "snooker" | "снукер" => Sport::Snooker,
            "water polo" | "водное поло" => Sport::WaterPolo,
            "aussie rules" | "афл" => Sport::AussieRules,
            "beach volleyball" | "пляжный волейбол" => Sport::BeachVolleyball,
            "floorball" | "флорбол" => Sport::Floorball,
            "golf" | "гольф" => Sport::Golf,
            "motorsport" | "автоспорт" | "formula 1" | "f1" | "nascar" => Sport::Motorsport,
            "cycling" | "велоспорт" => Sport::Cycling,
            "winter sports" | "зимние виды" | "биатлон" | "лыжи" => Sport::WinterSports,
            "politics" | "политика" => Sport::Politics,
            "entertainment" | "развлечения" | "тв шоу" => Sport::Entertainment,
            _ => Sport::Other,
        }
    }

    pub fn is_esport(&self) -> bool {
        matches!(self, Sport::Esports)
    }

    pub fn is_live_popular(&self) -> bool {
        matches!(
            self,
            Sport::Football
                | Sport::Tennis
                | Sport::Basketball
                | Sport::Hockey
                | Sport::TableTennis
                | Sport::Esports
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display)]
pub enum MarketType {
    #[strum(serialize = "1x2")]
    MatchResult,
    #[strum(serialize = "total")]
    Total,
    #[strum(serialize = "asian_total")]
    AsianTotal,
    #[strum(serialize = "handicap")]
    Handicap,
    #[strum(serialize = "asian_handicap")]
    AsianHandicap,
    #[strum(serialize = "both_teams_score")]
    BothTeamsScore,
    #[strum(serialize = "correct_score")]
    CorrectScore,
    #[strum(serialize = "even_odd")]
    EvenOdd,
    #[strum(serialize = "double_chance")]
    DoubleChance,
    #[strum(serialize = "draw_no_bet")]
    DrawNoBet,
    #[strum(serialize = "half_time_full_time")]
    HalfTimeFullTime,
    #[strum(serialize = "first_half_result")]
    FirstHalfResult,
    #[strum(serialize = "second_half_result")]
    SecondHalfResult,
    #[strum(serialize = "first_half_total")]
    FirstHalfTotal,
    #[strum(serialize = "second_half_total")]
    SecondHalfTotal,
    #[strum(serialize = "first_half_handicap")]
    FirstHalfHandicap,
    #[strum(serialize = "individual_total")]
    IndividualTotal,
    #[strum(serialize = "team_total")]
    TeamTotal,
    #[strum(serialize = "winning_margin")]
    WinningMargin,
    #[strum(serialize = "odd_even_first_half")]
    OddEvenFirstHalf,
    #[strum(serialize = "odd_even_second_half")]
    OddEvenSecondHalf,
    #[strum(serialize = "race_to")]
    RaceTo,
    #[strum(serialize = "exact_sets")]
    ExactSets,
    #[strum(serialize = "set_betting")]
    SetBetting,
    #[strum(serialize = "game_betting")]
    GameBetting,
    #[strum(serialize = "moneyline")]
    Moneyline,
    #[strum(serialize = "puck_line")]
    PuckLine,
    #[strum(serialize = "run_line")]
    RunLine,
    #[strum(serialize = "map_winner")]
    MapWinner,
    #[strum(serialize = "round_betting")]
    RoundBetting,
    #[strum(serialize = "method_of_victory")]
    MethodOfVictory,
    #[strum(serialize = "custom")]
    Custom,
}

impl MarketType {
    pub fn from_str(s: &str) -> Self {
        let s = s.to_lowercase().trim().to_string();
        match s.as_str() {
            "1x2" | "match result" | "основной исход" | "п1п2" | "w1w2" => MarketType::MatchResult,
            "total" | "тотал" | "over/under" | "больше/меньше" => MarketType::Total,
            "asian total" | "азиатский тотал" => MarketType::AsianTotal,
            "handicap" | "фора" | "ф" => MarketType::Handicap,
            "asian handicap" | "азиатская фора" | "аф" => MarketType::AsianHandicap,
            "both teams score" | "обе забьют" | "oz" | "oz да/нет" => MarketType::BothTeamsScore,
            "correct score" | "точный счёт" | "точный счет" => MarketType::CorrectScore,
            "even/odd" | "чёт/нечет" | "чет/нечет" | "even odd" => MarketType::EvenOdd,
            "double chance" | "двойной шанс" | "1x x2 12" => MarketType::DoubleChance,
            "draw no bet" | "фора 0" | "dnb" => MarketType::DrawNoBet,
            "half time/full time" | "тайм/матч" => MarketType::HalfTimeFullTime,
            "1st half result" | "1-й тайм исход" | "1h result" => MarketType::FirstHalfResult,
            "2nd half result" | "2-й тайм исход" | "2h result" => MarketType::SecondHalfResult,
            "1st half total" | "1-й тайм тотал" | "1h total" => MarketType::FirstHalfTotal,
            "2nd half total" | "2-й тайм тотал" | "2h total" => MarketType::SecondHalfTotal,
            "1st half handicap" | "1-й тайм фора" => MarketType::FirstHalfHandicap,
            "individual total" | "индивидуальный тотал" | "ит" => MarketType::IndividualTotal,
            "team total" | "командный тотал" => MarketType::TeamTotal,
            "winning margin" | "разница" | "победа с разницей" => MarketType::WinningMargin,
            "odd/even 1h" | "чёт/нечет 1т" => MarketType::OddEvenFirstHalf,
            "odd/even 2h" | "чёт/нечет 2т" => MarketType::OddEvenSecondHalf,
            "race to" | "кто первый" | "race to X goals" => MarketType::RaceTo,
            "exact sets" | "точный счёт по сетам" => MarketType::ExactSets,
            "set betting" | "беттинг на сеты" => MarketType::SetBetting,
            "game betting" | "беттинг на геймы" => MarketType::GameBetting,
            "moneyline" | "победитель" | "money line" => MarketType::Moneyline,
            "puck line" | "паклайн" => MarketType::PuckLine,
            "run line" | "ранлайн" => MarketType::RunLine,
            "map winner" | "победитель карты" => MarketType::MapWinner,
            "round betting" | "беттинг на раунды" => MarketType::RoundBetting,
            "method of victory" | "способ победы" => MarketType::MethodOfVictory,
            _ => MarketType::Custom,
        }
    }

    pub fn is_two_way(&self) -> bool {
        matches!(
            self,
            MarketType::Total
                | MarketType::AsianTotal
                | MarketType::Handicap
                | MarketType::AsianHandicap
                | MarketType::BothTeamsScore
                | MarketType::EvenOdd
                | MarketType::DrawNoBet
                | MarketType::IndividualTotal
                | MarketType::TeamTotal
                | MarketType::OddEvenFirstHalf
                | MarketType::OddEvenSecondHalf
                | MarketType::PuckLine
                | MarketType::RunLine
                | MarketType::MapWinner
        )
    }

    pub fn is_three_way(&self) -> bool {
        matches!(
            self,
            MarketType::MatchResult
                | MarketType::DoubleChance
                | MarketType::HalfTimeFullTime
                | MarketType::FirstHalfResult
                | MarketType::SecondHalfResult
                | MarketType::FirstHalfTotal
                | MarketType::SecondHalfTotal
                | MarketType::FirstHalfHandicap
                | MarketType::MethodOfVictory
        )
    }

    pub fn is_multi_outcome(&self) -> bool {
        matches!(
            self,
            MarketType::CorrectScore
                | MarketType::WinningMargin
                | MarketType::ExactSets
                | MarketType::SetBetting
                | MarketType::GameBetting
                | MarketType::RoundBetting
                | MarketType::RaceTo
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerFilter {
    pub sports: Vec<Sport>,
    pub markets: Vec<MarketType>,
    pub min_profit: f64,
    pub max_profit: f64,
    pub min_odds: f64,
    pub max_odds: f64,
    pub include_live: bool,
    pub include_prematch: bool,
    pub min_corridor_size: f64,
    pub max_express_legs: usize,
    pub bookmakers: Vec<String>,
    pub leagues: Vec<String>,
}

impl Default for ScannerFilter {
    fn default() -> Self {
        Self {
            sports: vec![
                Sport::Football,
                Sport::Tennis,
                Sport::Basketball,
                Sport::Hockey,
                Sport::TableTennis,
                Sport::Volleyball,
                Sport::Esports,
            ],
            markets: vec![
                MarketType::MatchResult,
                MarketType::Total,
                MarketType::Handicap,
                MarketType::AsianHandicap,
                MarketType::BothTeamsScore,
                MarketType::EvenOdd,
                MarketType::DoubleChance,
                MarketType::CorrectScore,
                MarketType::IndividualTotal,
            ],
            min_profit: 0.5,
            max_profit: 30.0,
            min_odds: 1.01,
            max_odds: 100.0,
            include_live: true,
            include_prematch: true,
            min_corridor_size: 0.5,
            max_express_legs: 3,
            bookmakers: vec![],
            leagues: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetConfig {
    pub enabled: bool,
    pub max_stake_per_bet: f64,
    pub max_daily_stake: f64,
    pub min_profit_percent: f64,
    pub delay_between_bets_ms: u64,
    pub max_bets_per_hour: u32,
    pub stealth_mode: bool,
    pub emergency_stop_loss: f64,
}

impl Default for AutoBetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_stake_per_bet: 5000.0,
            max_daily_stake: 50000.0,
            min_profit_percent: 1.0,
            delay_between_bets_ms: 3000,
            max_bets_per_hour: 20,
            stealth_mode: true,
            emergency_stop_loss: 10000.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankrollConfig {
    pub total_budget: f64,
    pub per_bookmaker: std::collections::HashMap<String, f64>,
    pub kelly_fraction: f64,
    pub max_exposure_percent: f64,
    pub auto_rebalance: bool,
    pub rebalance_threshold: f64,
}

impl Default for BankrollConfig {
    fn default() -> Self {
        Self {
            total_budget: 100000.0,
            per_bookmaker: std::collections::HashMap::new(),
            kelly_fraction: 0.25,
            max_exposure_percent: 5.0,
            auto_rebalance: true,
            rebalance_threshold: 20.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusConfig {
    pub enabled: bool,
    pub auto_claim: bool,
    pub min_bonus_ev: f64,
    pub priority_order: Vec<String>,
    pub track_wager_progress: bool,
}

impl Default for BonusConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_claim: false,
            min_bonus_ev: 50.0,
            priority_order: vec![],
            track_wager_progress: true,
        }
    }
}
