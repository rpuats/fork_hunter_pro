use crate::base::BookmakerParser;
use crate::{bet24, bettery, fonbet, leon, marathon, pari, sportbet, winline};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ParserFactory {
    parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>>,
}

impl ParserFactory {
    pub fn new(client: Arc<reqwest::Client>) -> Self {
        let mut parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>> = HashMap::new();

        // HTTP API парсеры
        parsers.insert("pari".to_string(), Arc::new(pari::PariParser::new(client.clone())));
        parsers.insert("marathon".to_string(), Arc::new(marathon::MarathonParser::new(client.clone())));
        parsers.insert("bettery".to_string(), Arc::new(bettery::BetteryParser::new(client.clone())));
        parsers.insert("fonbet".to_string(), Arc::new(fonbet::FonbetParser::new(client.clone())));
        parsers.insert("leon".to_string(), Arc::new(leon::LeonParser::new(client.clone())));
        parsers.insert("sportbet".to_string(), Arc::new(sportbet::SportbetParser::new(client.clone())));

        // Winline — Python Playwright parser (fallback)
        parsers.insert("winline".to_string(), Arc::new(winline::WinlineParser::new(client.clone())));

        // 24bet parser uses canonical slug `_24bet`, but keep legacy alias `bet24`
        // so existing callers do not break during migration.
        let bet24_parser: Arc<dyn BookmakerParser + Send + Sync> = Arc::new(bet24::_24betParser::new(client.clone()));
        parsers.insert("_24bet".to_string(), bet24_parser.clone());
        parsers.insert("bet24".to_string(), bet24_parser);

        // Olimp API имеет сложную структуру (competitions как map) — временно отключён
        // parsers.insert("olimp".to_string(), Arc::new(olimp::OlimpParser::new(client.clone())));

        ParserFactory { parsers }
    }

    pub fn get(&self, slug: &str) -> Option<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers.get(slug).cloned()
    }

    pub fn get_all(&self) -> Vec<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers.values().cloned().collect()
    }

    pub fn get_enabled(&self) -> Vec<Arc<dyn BookmakerParser + Send + Sync>> {
        self.parsers.values().filter(|p| p.is_enabled()).cloned().collect()
    }

    pub fn contains(&self, slug: &str) -> bool {
        self.parsers.contains_key(slug)
    }

    pub fn registered_slugs(&self) -> Vec<String> {
        let mut slugs: Vec<String> = self.parsers.keys().cloned().collect();
        slugs.sort();
        slugs
    }
}
