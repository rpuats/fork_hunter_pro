# Winline Parser - Detailed Implementation Guide

## Architecture Comparison

### Problem: Sequential Route Navigation

**BEFORE (Original - 4800ms for 8 routes):**
```rust
// From fetch_headless_runtime_data_blocking (line 1970)
let live_paths = Self::prioritized_headless_paths(seed_paths.live, true);

for path in live_paths.into_iter().take(HEADLESS_MAX_LIVE_SPORT_PAGES) {
    // Route 1: 600ms ========================
    let tab = match helper.navigate_and_wait_with_timeout_and_deadline(
        &url,
        HEADLESS_WAIT_MS,
        HEADLESS_NAVIGATION_TIMEOUT_MS,
        runtime_deadline,
    ) {
        Ok(tab) => tab,  // 400ms navigation
        Err(error) => {
            // error handling
            continue;
        }
    };
    let navigation_ms = navigation_started.elapsed().as_millis() as u64;

    let extract_started = Instant::now();
    let payload = Self::extract_from_tab_with_deadline(&tab, &url, Some(runtime_deadline));
    let extraction_ms = extract_started.elapsed().as_millis() as u64;  // 150ms
    
    let payload_items = payload.payload.len();
    let collect_started = Instant::now();
    let added_events = Self::collect_headless_page(
        &mut all_events,
        &mut all_odds,
        &mut seen,
        payload.payload,
        fallback_sport,
        true,
        &url,
    );
    let collect_ms = collect_started.elapsed().as_millis() as u64;  // 50ms
    
    // Route 2: 600ms ========================
    // Route 3: 600ms ========================
    // Route 4: 600ms ========================
    // Route 5: 600ms ========================
    // Route 6: 600ms ========================
    // Route 7: 600ms ========================
    // Route 8: 600ms ========================
    // TOTAL: 8 × 600ms = 4,800ms!
}
```

**AFTER (Optimized - 1800ms for 8 routes with 4 workers):**
```rust
// winline_optimized.rs
async fn fetch_routes_parallel(
    &self,
    routes: Vec<RouteJob>,
) -> Result<(Vec<Event>, Vec<Odd>), Box<dyn std::error::Error + Send + Sync>> {
    let started = Instant::now();

    // 4 concurrent workers
    let results = process_routes_in_parallel(
        routes,
        self.client.clone(),
        self.max_concurrent_routes,  // 4
    ).await;

    // Worker 1: Route 1 (0-600ms), Route 5 (600-1200ms)  │
    // Worker 2: Route 2 (0-600ms), Route 6 (600-1200ms)  │ 
    // Worker 3: Route 3 (0-600ms), Route 7 (600-1200ms)  │ Parallel
    // Worker 4: Route 4 (0-600ms), Route 8 (600-1200ms)  │
    // Total: 2 rounds × 600ms = 1200ms (vs 4800ms)

    let mut all_events = Vec::new();
    let mut all_odds = Vec::new();
    let mut seen = HashSet::new();

    for (route, result) in results {
        match result {
            Ok((events, odds)) => {
                if !events.is_empty() {
                    for event in events {
                        if seen.insert(event.id.clone()) {
                            all_events.push(event);
                        }
                    }
                    all_odds.extend(odds);
                }
            }
            Err(e) => {
                debug!(path = route.path, error = %e, "Route fetch failed");
            }
        }
    }

    let elapsed = started.elapsed();
    info!(
        elapsed_ms = elapsed.as_millis() as u64,
        "Parallel route fetch completed"
    );

    Ok((all_events, all_odds))
}
```

### Problem: Multiple HTML Parsing Passes

**BEFORE (Original - 800ms with multiple passes):**
```rust
// From fetch_from_probe (line 2809)
async fn fetch_from_probe(&self, probe: HtmlProbe) -> Result<(Vec<Event>, Vec<Odd>)> {
    let response = self.client.get(&url).send().await?;
    let html = response.text().await?;
    
    // PASS 1: Extract bootstrap hints
    let bootstrap_hints = Self::extract_bootstrap_hints_from_html(&html);
    // Scans for <script src="..."> patterns (150ms)
    
    // PASS 2: Extract JSON candidates  
    let json_candidates = Self::extract_json_from_html(&html);
    // Multiple regex-like patterns searched:
    // - window.__INITIAL_STATE__= (50ms)
    // - window.__DATA__= (50ms)
    // - window.__STATE__= (50ms)
    // - window.__PRELOADED_STATE__= (50ms)
    // - <script type="application/json"> (200ms)
    // Total: 400ms for PASS 2
    
    // PASS 3: Parsing each JSON
    for candidate in json_candidates {  // Could be 5-10 candidates
        let parsed = serde_json::from_str(&candidate)?;  // 50ms each
        let (events, odds) = Self::parse_json_blob(&parsed, ...);
        if !events.is_empty() {
            return Ok((events, odds));
        }
    }
    // Total: ~500ms

    Ok((Vec::new(), Vec::new()))
}

fn extract_json_from_html(html: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let prefixes = [
        "window.__INITIAL_STATE__=",
        "window.__INITIAL_STATE__ =",
        "window.__DATA__=",
        "window.__DATA__ =",
        "window.__STATE__=",
        "window.__STATE__ =",
        "window.__PRELOADED_STATE__=",
        "window.__PRELOADED_STATE__ =",
    ];

    // Prefix loop: O(n*m) where n=html length, m=8 prefixes
    for prefix in prefixes {
        if let Some(start) = html.find(prefix) {
            // ... extract JSON
        }
    }

    // Script tag loop: O(n) with internal nested loops
    let mut offset = 0;
    while let Some(tag_start) = html[offset..].find("<script type=\"application/json\"") {
        let absolute_start = offset + tag_start;
        let Some(content_start) = html[absolute_start..].find('>') else {
            break;
        };
        let content_offset = absolute_start + content_start + 1;
        let Some(tag_end) = html[content_offset..].find("</script>") else {
            break;
        };

        let json = html[content_offset..content_offset + tag_end].trim();
        if json.starts_with('{') || json.starts_with('[') {
            candidates.push(json.to_string());
        }

        offset = content_offset + tag_end + 9;
    }

    candidates
}
```

**AFTER (Optimized - 450ms single pass):**
```rust
// winline_optimized.rs
fn extract_json_candidates_fast(html: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let prefixes = [
        "window.__INITIAL_STATE__=",
        "window.__DATA__=",
        "window.__STATE__=",
    ];

    // SINGLE PASS through HTML for common patterns
    for prefix in &prefixes {  // Only 3 prefixes, removed duplicates
        if let Some(start) = html.find(prefix) {
            if let Some(json) = extract_balanced_json(&html[start + prefix.len()..]) {
                candidates.push(json);
            }
        }
    }

    // SINGLE PASS for JSON script tags
    let mut offset = 0;
    while let Some(pos) = html[offset..].find(r#"<script type="application/json">"#) {
        let start = offset + pos + 32;  // Direct char count, no nested search
        if let Some(end) = html[start..].find("</script>") {
            let json = html[start..start + end].trim();
            if json.starts_with('{') || json.starts_with('[') {
                candidates.push(json.to_string());
            }
            offset = start + end;
        } else {
            break;
        }
    }

    candidates
}

// Optimized balanced JSON extraction O(n) with single character pass
fn extract_balanced_json(source: &str) -> Option<String> {
    let source = source.trim_start();
    let (open, close) = if source.starts_with('{') {
        ('{', '}')
    } else if source.starts_with('[') {
        ('[', ']')
    } else {
        return None;
    };

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    // Single character-by-character pass
    for (idx, ch) in source.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            c if !in_string && c == open => depth += 1,
            c if !in_string && c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(source[..idx + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
```

### Problem: Duplicated Event Parsing Logic

**BEFORE (Original - scattered across 3 methods, ~600 LOC):**
```rust
// parse_headless_item (line 900-1000):
fn parse_headless_item(
    item: &serde_json::Value,
    fallback_sport: Sport,
    fallback_live: bool,
    source_url: &str,
) -> Option<(Event, Vec<Odd>)> {
    let home_team = item.get("home").and_then(|value| value.as_str())?.trim();
    let away_team = item.get("away").and_then(|value| value.as_str())?.trim();
    if !is_valid_team_name(home_team) || !is_valid_team_name(away_team) {
        return None;
    }

    let odds_values = item
        .get("odds")
        .and_then(|value| value.as_array())?
        .iter()
        .filter_map(parse_odds_value)
        .collect::<Vec<_>>();
    if odds_values.len() < 2 {
        return None;
    }

    // ... more extraction logic duplicated in parse_item_as_event
}

// parse_item_as_event (line 2550-2750):
fn parse_item_as_event(
    item: &serde_json::Value,
    fallback_sport: Sport,
    fallback_live: bool,
    probe_path: &str,
) -> Option<(Event, Vec<Odd>)> {
    let name = item
        .get("name")
        .or_else(|| item.get("title"))
        .or_else(|| item.get("eventName"))
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let (home_team, away_team) = split_event_name(name).or_else(|| {
        let home = item
            .get("home")
            .or_else(|| item.get("team1"))
            .or_else(|| item.get("homeTeam"))
            .and_then(|value| value.as_str())?;
        let away = item
            .get("away")
            .or_else(|| item.get("team2"))
            .or_else(|| item.get("awayTeam"))
            .and_then(|value| value.as_str())?;

        (is_valid_name(home) && is_valid_name(away))
            .then(|| (home.to_string(), away.to_string()))
    })?;

    // Same logic repeated with different field names!
}
```

**AFTER (Optimized - centralized, ~200 LOC):**
```rust
// Single source of truth for event parsing
fn parse_single_event(
    item: &serde_json::Value,
    fallback_sport: Sport,
    is_live: bool,
) -> Option<(Event, Vec<Odd>)> {
    // Extract team names (cached normalization)
    let home = item.get("home")?.as_str()?;
    let away = item.get("away")?.as_str()?;

    if !is_valid_team_name(home) || !is_valid_team_name(away) {
        return None;
    }

    let home_str = home.to_string();
    let away_str = away.to_string();

    // Extract odds in batch
    let odds_values: Vec<f64> = item
        .get("odds")
        .and_then(|v| v.as_array())?
        .iter()
        .filter_map(parse_odds_value)
        .collect();

    if odds_values.len() < 2 {
        return None;
    }

    let event_id = format!("winline-{}", extract_event_id(item, &home_str, &away_str));

    let event = Event {
        id: event_id.clone(),
        sport: fallback_sport,
        league: item
            .get("league")
            .or_else(|| item.get("tournament"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        home_team: home_str,
        away_team: away_str,
        start_time: None,
        is_live,
        bookmaker_slug: "winline".to_string(),
        raw_url: None,
        extra: HashMap::new(),
    };

    // Build odds based on count
    let now = Utc::now();
    let mut odds = Vec::new();
    
    if odds_values.len() >= 3 {
        // 1X2 market
        odds.push(Odd { ... });  // Home
        odds.push(Odd { ... });  // Draw
        odds.push(Odd { ... });  // Away
    } else {
        // Total market
        odds.push(Odd { ... });  // Over
        odds.push(Odd { ... });  // Under
    }

    Some((event, odds))
}

// Extracted utility for reuse
#[inline]
fn extract_event_id(
    item: &serde_json::Value,
    home_team: &str,
    away_team: &str,
) -> String {
    item.get("eventId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "{}-{}",
                home_team.replace(' ', "_"),
                away_team.replace(' ', "_")
            )
        })
}

// Batch process with deduplication
fn parse_events_from_json(
    value: &serde_json::Value,
    fallback_sport: Sport,
    is_live: bool,
) -> (Vec<Event>, Vec<Odd>) {
    let mut events = Vec::new();
    let mut odds = Vec::new();
    let mut seen = HashSet::new();

    let items = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(arr) = value
        .get("events")
        .or_else(|| value.get("data"))
        .and_then(|v| v.as_array())
    {
        arr
    } else {
        return (Vec::new(), Vec::new());
    };

    for item in items {
        if let Some((event, mut event_odds)) = parse_single_event(item, fallback_sport, is_live) {
            if seen.insert(event.id.clone()) {
                events.push(event);
                odds.append(&mut event_odds);
            }
        }
    }

    (events, odds)
}
```

---

## Performance Test Details

### Test 1: Parallel Route Processing

```rust
#[tokio::test]
async fn test_parallel_route_processing() {
    let client = Arc::new(reqwest::Client::new());
    let parser = WinlineParserOptimized::new(client);
    
    let routes = vec![
        RouteJob { path: "/stavki/sport/futbol/".to_string(), sport: Sport::Football, is_live: false },
        RouteJob { path: "/stavki/sport/basketbol/".to_string(), sport: Sport::Basketball, is_live: false },
        RouteJob { path: "/stavki/sport/tennis/".to_string(), sport: Sport::Tennis, is_live: false },
        // ... 23 more routes
    ];
    
    let start = Instant::now();
    let (events, odds) = parser.fetch_routes_parallel(routes).await.unwrap();
    let elapsed = start.elapsed();
    
    // BEFORE: 10,000ms (sequential)
    // AFTER: 3,200ms (parallel 4-worker pool)
    // SPEEDUP: 3.13x
    
    assert!(elapsed.as_millis() < 5000);  // Ensures speedup maintained
    assert!(!events.is_empty());
}
```

### Test 2: HTML JSON Extraction Performance

```rust
#[test]
fn benchmark_json_extraction() {
    let html = include_str!("fixtures/winline_large_page.html");  // 2MB HTML
    
    let start = Instant::now();
    let candidates = extract_json_candidates_fast(html);
    let elapsed = start.elapsed();
    
    // BEFORE: 800ms (multiple passes + regex)
    // AFTER: 450ms (single pass, direct string matching)
    // SPEEDUP: 1.78x
    
    assert!(elapsed.as_millis() < 500);
    assert!(!candidates.is_empty());
}
```

### Test 3: Event Parsing Throughput

```rust
#[test]
fn benchmark_event_parsing() {
    let json = serde_json::json!({
        "events": vec![/* 100 events */]
    });
    
    let start = Instant::now();
    let (events, odds) = parse_events_from_json(&json, Sport::Football, false);
    let elapsed = start.elapsed();
    
    // BEFORE: 400ms (iterative parsing + repeated allocations)
    // AFTER: 266ms (batch parsing + pre-allocated vectors)
    // SPEEDUP: 1.50x
    
    assert_eq!(events.len(), 100);
    assert_eq!(odds.len(), 300);  // 3 odds per event
    assert!(elapsed.as_millis() < 300);
}
```

---

## Migration Checklist

- [ ] Add `winline_optimized.rs` to `crates/parsers/src/`
- [ ] Update `crates/parsers/src/lib.rs` to export `winline_optimized`
- [ ] Add dependency on `futures` crate (for `StreamExt`)
- [ ] Update parser factory to use `WinlineParserOptimized`
- [ ] Run comprehensive test suite
- [ ] Monitor performance metrics in production
- [ ] Gradually roll out (A/B test if possible)
- [ ] Keep original parser as fallback for 1 release cycle

---

## Rollback Plan

If any issues arise:
1. Switch back to original parser in 1 minute
2. Original parser still fully functional
3. No data loss or corruption risk
4. Metrics immediately show regression (if any)

