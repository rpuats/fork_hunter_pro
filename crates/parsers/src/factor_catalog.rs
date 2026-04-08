use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// Factor definition из каталога
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorDef {
    pub id: u64,
    pub name: String,
    pub market_type: String,
    pub selections: Vec<String>,
}

/// Каталог факторов для shared platform
#[derive(Clone)]
pub struct FactorCatalog {
    client: Arc<Client>,
    base_url: String,
    scope_market: u64,
    /// factor_id -> FactorDef
    cache: Arc<tokio::sync::RwLock<HashMap<u64, FactorDef>>>,
}

impl FactorCatalog {
    pub fn new(client: Arc<Client>, base_url: &str, scope_market: u64) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            scope_market,
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Загрузить каталог факторов из API
    pub async fn load(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let urls = [
            format!("{}/line/factorsCatalog/independentFactors?version=0&lang=ru&sysId=21&scopeMarket={}", self.base_url, self.scope_market),
            format!("{}/line/factorsCatalog/sportBasicFactors?version=0&lang=ru&sysId=21", self.base_url),
        ];

        let mut total = 0;
        let mut cache = self.cache.write().await;

        for url in urls {
            debug!(url, "Fetching factor catalog");
            if let Ok(resp) = self.client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        total += self.parse_catalog(&json, &mut cache);
                    }
                }
            }
        }

        info!(factors = total, "Factor catalog loaded");
        Ok(total)
    }

    fn parse_catalog(&self, json: &serde_json::Value, cache: &mut HashMap<u64, FactorDef>) -> usize {
        let mut count = 0;

        // Структура: { factors: [{id, name, outcomes: [{id, name}]}] }
        if let Some(factors) = json.get("factors").and_then(|f| f.as_array()) {
            for factor in factors {
                if let (Some(fid), Some(name)) = (factor.get("id").and_then(|v| v.as_u64()), factor.get("name").and_then(|v| v.as_str())) {
                    let selections: Vec<String> = factor.get("outcomes")
                        .and_then(|o| o.as_array())
                        .map(|arr| arr.iter().filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
                        .unwrap_or_default();

                    let market_type = factor.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();

                    cache.insert(fid, FactorDef {
                        id: fid,
                        name: name.to_string(),
                        market_type,
                        selections,
                    });
                    count += 1;
                }
            }
        }

        // Альтернативная структура: { data: [{factorId, factorName, outcomes: [...]}] }
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let (Some(fid), Some(name)) = (item.get("factorId").and_then(|v| v.as_u64()), item.get("factorName").and_then(|v| v.as_str())) {
                    let selections: Vec<String> = item.get("outcomes")
                        .and_then(|o| o.as_array())
                        .map(|arr| arr.iter().filter_map(|o| o.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
                        .unwrap_or_default();

                    let market_type = item.get("marketType").and_then(|t| t.as_str()).unwrap_or("").to_string();

                    cache.insert(fid, FactorDef {
                        id: fid,
                        name: name.to_string(),
                        market_type,
                        selections,
                    });
                    count += 1;
                }
            }
        }

        count
    }

    /// Получить определение фактора по ID
    pub async fn get_factor(&self, factor_id: u64) -> Option<FactorDef> {
        let cache = self.cache.read().await;
        cache.get(&factor_id).cloned()
    }

    /// Найти все факторы для рынка
    pub async fn find_factors_by_market(&self, market_type: &str) -> Vec<FactorDef> {
        let cache = self.cache.read().await;
        cache.values()
            .filter(|f| f.market_type.to_lowercase().contains(&market_type.to_lowercase())
                || f.name.to_lowercase().contains(&market_type.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Получить все известные факторы
    pub async fn all_factors(&self) -> Vec<FactorDef> {
        let cache = self.cache.read().await;
        cache.values().cloned().collect()
    }

    /// Получить маппинг factor_id -> market_type
    pub async fn factor_to_market(&self) -> HashMap<u64, String> {
        let cache = self.cache.read().await;
        cache.iter().map(|(k, v)| (*k, v.market_type.clone())).collect()
    }
}

/// Известные фактор-иды для дополнительных рынков (обнаруженные динамически)
pub mod known_factors {
    // Тоталы (уже известны)
    pub const TOTAL_OVER: &[u64] = &[924, 1002, 1010, 1054];
    pub const TOTAL_UNDER: &[u64] = &[925, 1003, 1011, 1055];

    // Форы (уже известны)
    pub const HANDICAP: &[u64] = &[1006, 1004, 1005, 1012, 1013];

    //_candidate_ факторы для дополнительных рынков (нужна верификация)
    // Эти IDs основаны на паттернах в ответах API и могут различаться для разных БК
    pub const BTTS_YES: u64 = 926;   // ОЗ Да / Обе забьют - Да
    pub const BTTS_NO: u64 = 927;    // ОЗ Нет / Обе забьют - Нет
    pub const EVEN: u64 = 928;       // Чёт
    pub const ODD: u64 = 929;        // Нечёт
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_factors_constants() {
        assert!(known_factors::TOTAL_OVER.contains(&924));
        assert!(known_factors::TOTAL_OVER.contains(&1002));
        assert!(known_factors::TOTAL_UNDER.contains(&925));
        assert!(known_factors::TOTAL_UNDER.contains(&1003));
        assert!(known_factors::HANDICAP.contains(&1006));
        assert_eq!(known_factors::BTTS_YES, 926);
        assert_eq!(known_factors::BTTS_NO, 927);
        assert_eq!(known_factors::EVEN, 928);
        assert_eq!(known_factors::ODD, 929);
    }

    #[test]
    fn test_factor_catalog_empty() {
        let client = Arc::new(Client::new());
        let catalog = FactorCatalog::new(client, "https://example.com", 2300);

        let all = tokio::runtime::Runtime::new().unwrap().block_on(catalog.all_factors());
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_parse_catalog_flat() {
        let catalog = FactorCatalog::new(Arc::new(Client::new()), "https://example.com", 2300);
        let mut cache = HashMap::new();

        let json = serde_json::json!({
            "factors": [
                {"id": 921, "name": "П1", "type": "1X2", "outcomes": [{"name": "1"}]},
                {"id": 922, "name": "Х", "type": "1X2", "outcomes": [{"name": "X"}]},
                {"id": 926, "name": "ОЗ Да", "type": "BTTS", "outcomes": [{"name": "Да"}]},
            ]
        });

        let count = catalog.parse_catalog(&json, &mut cache);
        assert_eq!(count, 3);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache[&921].name, "П1");
        assert_eq!(cache[&926].market_type, "BTTS");
    }

    #[tokio::test]
    async fn test_parse_catalog_nested_data() {
        let catalog = FactorCatalog::new(Arc::new(Client::new()), "https://example.com", 2300);
        let mut cache = HashMap::new();

        let json = serde_json::json!({
            "data": [
                {"factorId": 921, "factorName": "Исход", "marketType": "1X2", "outcomes": [{"name": "П1"}, {"name": "Х"}, {"name": "П2"}]},
                {"factorId": 926, "factorName": "ОЗ", "marketType": "BTTS", "outcomes": [{"name": "Да"}, {"name": "Нет"}]},
            ]
        });

        let count = catalog.parse_catalog(&json, &mut cache);
        assert_eq!(count, 2);
        assert_eq!(cache[&921].selections, vec!["П1", "Х", "П2"]);
        assert_eq!(cache[&926].selections, vec!["Да", "Нет"]);
    }
}
