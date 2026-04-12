use crate::base::BookmakerParser;
use crate::{
    baltbet, bet24, betboom, betcity, bettery, fonbet, leon, ligastavok, marathon, olimpbet, pari,
    sportbet, winline, zenit,
};
use shared::{BookmakerMetadata, HealthStatus, ParserCoverage, ParserHealth};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct BookmakerRegistryEntry {
    slug: &'static str,
    name: &'static str,
    aliases: &'static [&'static str],
    parser_type: &'static str,
    source: &'static str,
    execution_supported: bool,
    notes: Option<&'static str>,
}

const BOOKMAKER_REGISTRY: &[BookmakerRegistryEntry] = &[
    BookmakerRegistryEntry {
        slug: "pari",
        name: "Pari",
        aliases: &["pari"],
        parser_type: "api",
        source: "crates/parsers/src/pari.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "fonbet",
        name: "Fonbet",
        aliases: &["fonbet"],
        parser_type: "api",
        source: "crates/parsers/src/fonbet.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "bettery",
        name: "Bettery",
        aliases: &["bettery"],
        parser_type: "api",
        source: "crates/parsers/src/bettery.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "marathon",
        name: "Marathon",
        aliases: &["marathon"],
        parser_type: "api",
        source: "crates/parsers/src/marathon.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "bet24",
        name: "24bet",
        aliases: &["bet24", "_24bet"],
        parser_type: "api",
        source: "crates/parsers/src/bet24.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "leon",
        name: "Leon",
        aliases: &["leon"],
        parser_type: "api",
        source: "crates/parsers/src/leon.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "sportbet",
        name: "Sportbet",
        aliases: &["sportbet"],
        parser_type: "api",
        source: "crates/parsers/src/sportbet.rs",
        execution_supported: false,
        notes: Some("Rust parser registered for market scanning."),
    },
    BookmakerRegistryEntry {
        slug: "winline",
        name: "Winline",
        aliases: &["winline"],
        parser_type: "python",
        source: "crates/parsers/src/winline.rs",
        execution_supported: false,
        notes: Some("Python wrapper registered for scan-only operation."),
    },
    BookmakerRegistryEntry {
        slug: "zenit",
        name: "Zenit",
        aliases: &["zenit"],
        parser_type: "api",
        source: "crates/parsers/src/zenit.rs",
        execution_supported: false,
        notes: Some("Pure HTTP parser with imprinthash authentication."),
    },
    BookmakerRegistryEntry {
        slug: "betcity",
        name: "Betcity",
        aliases: &["betcity"],
        parser_type: "api",
        source: "crates/parsers/src/betcity.rs",
        execution_supported: false,
        notes: Some("HTTP parser with API and HTML fallback."),
    },
    BookmakerRegistryEntry {
        slug: "baltbet",
        name: "Baltbet",
        aliases: &["baltbet"],
        parser_type: "api",
        source: "crates/parsers/src/baltbet.rs",
        execution_supported: false,
        notes: Some("Pure HTTP parser with HTML parsing and demo fallback."),
    },
    BookmakerRegistryEntry {
        slug: "olimpbet",
        name: "Olimpbet",
        aliases: &["olimpbet"],
        parser_type: "api",
        source: "crates/parsers/src/olimpbet.rs",
        execution_supported: false,
        notes: Some("Registered parser is currently scan-only."),
    },
    BookmakerRegistryEntry {
        slug: "betboom",
        name: "BetBoom",
        aliases: &["betboom"],
        parser_type: "api",
        source: "crates/parsers/src/betboom.rs",
        execution_supported: false,
        notes: Some("Parser exists but is disabled in production."),
    },
    BookmakerRegistryEntry {
        slug: "ligastavok",
        name: "Liga Stavok",
        aliases: &["ligastavok"],
        parser_type: "api",
        source: "crates/parsers/src/ligastavok.rs",
        execution_supported: false,
        notes: Some("Parser exists but is disabled in production."),
    },
    BookmakerRegistryEntry {
        slug: "olimp",
        name: "Olimp",
        aliases: &["olimp"],
        parser_type: "api",
        source: "crates/parsers/src/olimp.rs",
        execution_supported: false,
        notes: Some("Implementation exists but is not registered in the factory."),
    },
];

pub struct ParserFactory {
    parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>>,
}

impl ParserFactory {
    fn registry_entry(slug: &str) -> Option<&'static BookmakerRegistryEntry> {
        BOOKMAKER_REGISTRY.iter().find(|entry| entry.slug == slug)
    }

    pub fn new(client: Arc<reqwest::Client>) -> Self {
        let mut parsers: HashMap<String, Arc<dyn BookmakerParser + Send + Sync>> = HashMap::new();

        // HTTP API парсеры
        parsers.insert(
            "pari".to_string(),
            Arc::new(pari::PariParser::new(client.clone())),
        );
        parsers.insert(
            "marathon".to_string(),
            Arc::new(marathon::MarathonParser::new(client.clone())),
        );
        parsers.insert(
            "bettery".to_string(),
            Arc::new(bettery::BetteryParser::new(client.clone())),
        );
        parsers.insert(
            "fonbet".to_string(),
            Arc::new(fonbet::FonbetParser::new(client.clone())),
        );
        parsers.insert(
            "leon".to_string(),
            Arc::new(leon::LeonParser::new(client.clone())),
        );
        parsers.insert(
            "sportbet".to_string(),
            Arc::new(sportbet::SportbetParser::new(client.clone())),
        );
        parsers.insert(
            "ligastavok".to_string(),
            Arc::new(ligastavok::LigaStavokParser::new(client.clone())),
        );
        parsers.insert(
            "betboom".to_string(),
            Arc::new(betboom::BetboomParser::new(client.clone())),
        );

        // HTTP парсеры — Winline, Zenit, Betcity, Baltbet
        parsers.insert(
            "winline".to_string(),
            Arc::new(winline::WinlineParser::new(client.clone())),
        );
        parsers.insert(
            "zenit".to_string(),
            Arc::new(zenit::ZenitParser::new(client.clone())),
        );
        parsers.insert(
            "betcity".to_string(),
            Arc::new(betcity::BetcityParser::new(client.clone())),
        );
        parsers.insert(
            "baltbet".to_string(),
            Arc::new(baltbet::BaltbetParser::new(client.clone())),
        );

        // Olimpbet — без Cloudflare!
        parsers.insert(
            "olimpbet".to_string(),
            Arc::new(olimpbet::OlimpbetParser::new(client.clone())),
        );

        // 24bet parser
        let bet24_parser: Arc<dyn BookmakerParser + Send + Sync> =
            Arc::new(bet24::_24betParser::new(client.clone()));
        parsers.insert("_24bet".to_string(), bet24_parser.clone());
        parsers.insert("bet24".to_string(), bet24_parser);

        // Olimp API имеет сложную структуру — временно отключён
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
        self.parsers
            .values()
            .filter(|p| p.is_enabled())
            .cloned()
            .collect()
    }

    pub fn contains(&self, slug: &str) -> bool {
        self.parsers.contains_key(slug)
    }

    pub fn registered_slugs(&self) -> Vec<String> {
        let mut slugs: Vec<String> = self.parsers.keys().cloned().collect();
        slugs.sort();
        slugs
    }

    pub fn bookmaker_metadata(&self) -> Vec<BookmakerMetadata> {
        let mut items = Vec::new();

        for entry in BOOKMAKER_REGISTRY {
            let parser = entry
                .aliases
                .iter()
                .find_map(|alias| self.parsers.get(*alias));

            let (name, enabled, scan_supported) = if let Some(parser) = parser {
                (
                    parser.name().to_string(),
                    parser.is_enabled(),
                    parser.is_enabled(),
                )
            } else {
                (entry.name.to_string(), false, false)
            };

            items.push(BookmakerMetadata::new(
                entry.slug,
                name,
                enabled,
                scan_supported,
                entry.execution_supported,
                entry.notes.map(str::to_string),
            ));
        }

        for parser in self.parsers.values() {
            let covered_by_registry = BOOKMAKER_REGISTRY
                .iter()
                .any(|entry| entry.aliases.iter().any(|alias| *alias == parser.slug()));

            if covered_by_registry || items.iter().any(|item| item.slug == parser.slug()) {
                continue;
            }

            items.push(parser.metadata());
        }

        items.sort_by(|a, b| a.slug.cmp(&b.slug));
        items
    }

    pub fn parser_coverage(&self) -> Vec<ParserCoverage> {
        let mut items = Vec::new();

        for entry in BOOKMAKER_REGISTRY {
            let parser = entry
                .aliases
                .iter()
                .find_map(|alias| self.parsers.get(*alias));

            let (name, enabled, scan_supported, readiness) = if let Some(parser) = parser {
                (
                    parser.name().to_string(),
                    parser.is_enabled(),
                    parser.is_enabled(),
                    parser.readiness(),
                )
            } else {
                (entry.name.to_string(), false, false, None)
            };

            let metadata = BookmakerMetadata::new(
                entry.slug,
                name.clone(),
                enabled,
                scan_supported,
                entry.execution_supported,
                entry.notes.map(str::to_string),
            );

            items.push(ParserCoverage {
                slug: entry.slug.to_string(),
                name,
                enabled,
                scan_supported,
                execution_supported: entry.execution_supported,
                status: metadata.status,
                parser_type: entry.parser_type.to_string(),
                source: entry.source.to_string(),
                notes: entry.notes.map(str::to_string),
                readiness,
            });
        }

        items.sort_by(|a, b| a.slug.cmp(&b.slug));
        items
    }

    pub fn parser_health_snapshots(&self) -> Vec<ParserHealth> {
        let mut items = Vec::new();

        for coverage in self.parser_coverage() {
            let diagnostics = coverage
                .readiness
                .as_ref()
                .map(|item| item.checks.clone())
                .unwrap_or_default();
            let degraded_reason = if coverage.enabled {
                None
            } else {
                coverage.notes.clone()
            };

            items.push(ParserHealth {
                bookmaker: coverage.slug.clone(),
                status: if coverage.enabled {
                    HealthStatus::Healthy
                } else if coverage.readiness.is_some() {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Unhealthy
                },
                last_success: None,
                last_error: degraded_reason,
                consecutive_failures: 0,
                avg_response_time_ms: 0.0,
                events_parsed: 0,
                uptime_percent: if coverage.enabled { 100.0 } else { 0.0 },
                readiness: coverage.readiness.clone(),
                diagnostics,
            });
        }

        for health in &mut items {
            if let Some(entry) = Self::registry_entry(&health.bookmaker) {
                if health.last_error.is_none() {
                    health.last_error = entry.notes.map(str::to_string);
                }
            }
        }

        items.sort_by(|a, b| a.bookmaker.cmp(&b.bookmaker));
        items
    }
}

#[cfg(test)]
mod tests {
    use super::ParserFactory;
    use shared::HealthStatus;
    use std::sync::Arc;

    #[test]
    fn ligastavok_coverage_includes_readiness() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let coverage = factory
            .parser_coverage()
            .into_iter()
            .find(|item| item.slug == "ligastavok")
            .expect("coverage");

        assert_eq!(coverage.parser_type, "api");
        assert_eq!(coverage.source, "crates/parsers/src/ligastavok.rs");
        assert!(coverage.readiness.is_some());
        assert!(!coverage.enabled);
    }

    #[test]
    fn ligastavok_health_snapshot_includes_diagnostics() {
        let client = Arc::new(reqwest::Client::builder().build().expect("client"));
        let factory = ParserFactory::new(client);

        let health = factory
            .parser_health_snapshots()
            .into_iter()
            .find(|item| item.bookmaker == "ligastavok")
            .expect("health");

        assert!(matches!(health.status, HealthStatus::Degraded));
        assert!(health.readiness.is_some());
        assert!(health
            .diagnostics
            .iter()
            .any(|check| check.code == "session_bootstrap_pending"));
    }
}
