use crate::base::BookmakerParser;
use crate::{bettery, fonbet, leon, marathon, olimp, pari, sportbet};
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
        parsers.insert("olimp".to_string(), Arc::new(olimp::OlimpParser::new(client.clone())));
        parsers.insert("sportbet".to_string(), Arc::new(sportbet::SportbetParser::new(client.clone())));

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
}
