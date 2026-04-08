use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info, warn};

/// Полное определение фактора из каталога
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorDefinition {
    pub id: u64,
    pub name: String,
    pub market_type: String,
    pub selection_name: String,
    pub sport: String,
}

/// Каталог факторов для shared platform (*-resources.com)
#[derive(Clone)]
pub struct FactorsCatalog {
    client: Arc<Client>,
    base_url: String,
    scope_market: u64,
    /// factor_id -> FactorDefinition
    cache: Arc<RwLock<HashMap<u64, FactorDefinition>>>,
    /// market_name -> Vec<factor_id>
    market_index: Arc<RwLock<HashMap<String, Vec<u64>>>>,
}

impl FactorsCatalog {
    pub fn new(client: Arc<Client>, base_url: &str, scope_market: u64) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            scope_market,
            cache: Arc::new(RwLock::new(HashMap::new())),
            market_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Загрузить полный каталог факторов из API
    pub async fn load(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let endpoints = vec![
            format!("{}/line/factorsCatalog/independentFactors?version=0&lang=ru&sysId=21&scopeMarket={}", self.base_url, self.scope_market),
            format!("{}/line/factorsCatalog/sportBasicFactors?version=0&lang=ru&sysId=21", self.base_url),
            format!("{}/line/factorsCatalog/tables?version=0&lang=ru&sysId=21", self.base_url),
        ];

        let mut total = 0;

        for url in endpoints {
            debug!(url, "Fetching factors catalog");
            match self.client.get(&url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .header("Accept", "application/json")
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<serde_json::Value>().await {
                            Ok(json) => {
                                let count = self.parse_catalog(&json);
                                total += count;
                                debug!(url, count, "Factors parsed");
                            }
                            Err(e) => {
                                warn!(url, error = %e, "Failed to parse JSON from factors catalog");
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!(url, error = %e, "Failed to fetch factors catalog");
                }
            }
        }

        // Заполняем market_index
        self.rebuild_market_index();

        info!(total_factors = total, markets = self.market_index.read().len(), "Factors catalog loaded");
        Ok(total)
    }

    /// Распарсить JSON каталог
    fn parse_catalog(&self, json: &serde_json::Value) -> usize {
        let mut count = 0;
        let mut cache = self.cache.write();

        // Формат 1: { factors: [{id, name, type, outcomes: [{id, name}]}] }
        if let Some(factors) = json.get("factors").and_then(|f| f.as_array()) {
            for factor in factors {
                if let (Some(fid), Some(name)) = (factor.get("id").and_then(|v| v.as_u64()), factor.get("name").and_then(|v| v.as_str())) {
                    let market_type = factor.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    let outcomes = factor.get("outcomes").and_then(|o| o.as_array());

                    if let Some(outcomes_arr) = &outcomes {
                        for outcome in outcomes_arr.iter() {
                            if let (Some(oid), Some(oname)) = (outcome.get("id").and_then(|v| v.as_u64()), outcome.get("name").and_then(|v| v.as_str())) {
                                cache.insert(oid, FactorDefinition {
                                    id: oid,
                                    name: oname.to_string(),
                                    market_type: market_type.clone(),
                                    selection_name: oname.to_string(),
                                    sport: factor.get("sport").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                                });
                                count += 1;
                            }
                        }
                    }

                    // Сам фактор тоже может быть outcome
                    if outcomes.as_ref().map_or(true, |o| o.is_empty()) {
                        cache.insert(fid, FactorDefinition {
                            id: fid,
                            name: name.to_string(),
                            market_type: market_type.clone(),
                            selection_name: name.to_string(),
                            sport: factor.get("sport").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                        });
                        count += 1;
                    }
                }
            }
        }

        // Формат 2: { data: [{factorId, factorName, marketType, outcomes: [...]}] }
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let (Some(fid), Some(fname)) = (item.get("factorId").and_then(|v| v.as_u64()), item.get("factorName").and_then(|v| v.as_str())) {
                    let market_type = item.get("marketType").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    let sport = item.get("sport").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    
                    if let Some(outcomes) = item.get("outcomes").and_then(|o| o.as_array()) {
                        for outcome in outcomes {
                            if let Some(oname) = outcome.get("name").and_then(|v| v.as_str()) {
                                // outcomeId может быть factorId + outcomeIndex
                                let oid = outcome.get("id").and_then(|v| v.as_u64()).unwrap_or(fid);
                                cache.insert(oid, FactorDefinition {
                                    id: oid,
                                    name: oname.to_string(),
                                    market_type: market_type.clone(),
                                    selection_name: oname.to_string(),
                                    sport: sport.clone(),
                                });
                                count += 1;
                            }
                        }
                    } else {
                        cache.insert(fid, FactorDefinition {
                            id: fid,
                            name: fname.to_string(),
                            market_type: market_type.clone(),
                            selection_name: fname.to_string(),
                            sport: sport.clone(),
                        });
                        count += 1;
                    }
                }
            }
        }

        // Формат 3: Прямой маппинг { "921": "П1", "922": "Х", ... }
        if let Some(obj) = json.as_object() {
            for (key, val) in obj {
                if let Ok(fid) = key.parse::<u64>() {
                    if let Some(name) = val.as_str() {
                        cache.entry(fid).or_insert_with(|| FactorDefinition {
                            id: fid,
                            name: name.to_string(),
                            market_type: Self::guess_market_type(name),
                            selection_name: name.to_string(),
                            sport: String::new(),
                        });
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// Угадать тип рынка по названию
    fn guess_market_type(name: &str) -> String {
        let n = name.to_lowercase();
        if n.contains("итб") || n.contains("итм") || n.contains("individual") { "IndividualTotal".into() }
        else if n.contains("тб") || n.contains("тм") || n.contains("total") { "Total".into() }
        else if n.contains("фора") || n.contains("handicap") || n.contains("ф ") { "Handicap".into() }
        else if n.contains("оз") || n.contains("обе") || n.contains("btts") || n.contains("both") { "BothTeamsScore".into() }
        else if n.contains("чёт") || n.contains("нечет") || n.contains("even") || n.contains("odd") { "EvenOdd".into() }
        else if n.contains("двойн") || n.contains("double") || n.contains("1x") || n.contains("x2") || n.contains("12") { "DoubleChance".into() }
        else if n.contains("точн") || n.contains("correct") || n.contains("score") { "CorrectScore".into() }
        else if n.contains("1-й тайм") || n.contains("1h") { "FirstHalf".into() }
        else if n.contains("2-й тайм") || n.contains("2h") { "SecondHalf".into() }
        else if n.contains("п1") || n.contains("п2") || n.contains("х") || n.contains("1x2") { "1X2".into() }
        else { n }
    }

    /// Перестроить индекс market -> factor_ids
    fn rebuild_market_index(&self) {
        let cache = self.cache.read();
        let mut market_index: HashMap<String, Vec<u64>> = HashMap::new();
        
        for (&fid, def) in cache.iter() {
            market_index.entry(def.market_type.clone()).or_default().push(fid);
        }
        
        for v in market_index.values_mut() {
            v.sort();
        }
        
        drop(cache);
        *self.market_index.write() = market_index;
    }

    /// Получить определение фактора по ID
    pub fn get_factor(&self, factor_id: u64) -> Option<FactorDefinition> {
        self.cache.read().get(&factor_id).cloned()
    }

    /// Получить все factor IDs для типа рынка
    pub fn get_factor_ids(&self, market_type: &str) -> Vec<u64> {
        self.market_index.read().get(market_type).cloned().unwrap_or_default()
    }

    /// Получить все известные рынки
    pub fn get_all_markets(&self) -> Vec<String> {
        let mut markets: Vec<String> = self.market_index.read().keys().cloned().collect();
        markets.sort();
        markets
    }

    /// Получить все известные факторы
    pub fn get_all_factors(&self) -> Vec<FactorDefinition> {
        self.cache.read().values().cloned().collect()
    }

    /// Печатная сводка
    pub fn print_summary(&self) {
        let cache = self.cache.read();
        let market_index = self.market_index.read();
        
        info!("=== Factors Catalog Summary ===");
        info!("Total unique factors: {}", cache.len());
        info!("Total markets: {}", market_index.len());
        
        for (market, ids) in market_index.iter() {
            info!("  {}: {} factors (IDs: {:?})", market, ids.len(), ids.iter().take(5).collect::<Vec<_>>());
        }
    }

    /// Найти факторы по ключевому слову
    pub fn find_factors(&self, keyword: &str) -> Vec<FactorDefinition> {
        let kw = keyword.to_lowercase();
        self.cache.read().values()
            .filter(|f| {
                f.name.to_lowercase().contains(&kw) ||
                f.market_type.to_lowercase().contains(&kw) ||
                f.selection_name.to_lowercase().contains(&kw)
            })
            .cloned()
            .collect()
    }
}

impl std::fmt::Debug for FactorsCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactorsCatalog")
            .field("base_url", &self.base_url)
            .field("scope_market", &self.scope_market)
            .finish_non_exhaustive()
    }
}
