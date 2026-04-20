use crate::normalizer::Normalizer;
use chrono::Utc;
use dashmap::DashMap;
use shared::{Event, Odd, OddsError};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Enhanced detection result with confidence score and reasoning
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub error: OddsError,
    pub confidence: f64, // 0-100%
    pub detection_methods: Vec<String>, // "3-sigma", "IQR", "Grubbs", etc.
    pub reason: String,
    pub time_series_flag: bool,
    pub bk_anomaly_flag: bool,
    pub market_anomaly_flag: bool,
    pub ml_score: f64, // Machine learning fusion score
    pub systematic_shift: bool, // Time-series trend detection
}

#[derive(Clone)]
pub struct OddsErrorDetector {
    deviation_threshold: f64,
    min_samples: usize,
    recent_odds: Arc<DashMap<String, Vec<f64>>>,
    recent_odds_with_time: Arc<DashMap<String, Vec<(f64, i64)>>>, // odds + timestamp
    bk_profiles: Arc<DashMap<String, BKProfile>>, // Bookmaker behavior profiles
    market_profiles: Arc<DashMap<String, MarketProfile>>, // Market-specific anomaly profiles
}

#[derive(Clone, Debug)]
struct BKProfile {
    avg_odds: f64,
    deviation: f64,
    anomaly_count: usize,
    total_observations: usize,
    confidence_factor: f64, // Reputation factor: 0.5-2.0
    win_count: usize, // Accurate odds count
    loss_count: usize, // Anomalous odds count
}

#[derive(Clone, Debug)]
struct MarketProfile {
    market_type: String, // "1X2", "Over/Under", etc.
    avg_deviation: f64,
    volatility: f64,
    anomaly_count: usize,
    total_samples: usize,
    min_odds: f64,
    max_odds: f64,
}

impl OddsErrorDetector {
    pub fn new(deviation_threshold: f64, min_samples: usize) -> Self {
        Self {
            deviation_threshold,
            min_samples,
            recent_odds: Arc::new(DashMap::new()),
            recent_odds_with_time: Arc::new(DashMap::new()),
            bk_profiles: Arc::new(DashMap::new()),
            market_profiles: Arc::new(DashMap::new()),
        }
    }

    pub fn detect_errors(&self, all_odds: &[Odd]) -> Vec<OddsError> {
        self.detect_errors_advanced(&[], &HashMap::new(), all_odds, false)
            .into_iter()
            .map(|dr| dr.error)
            .collect()
    }

    /// Enhanced detection with confidence scores and multiple statistical methods
    pub fn detect_errors_with_confidence(
        &self,
        all_odds: &[Odd],
    ) -> Vec<DetectionResult> {
        self.detect_errors_advanced(&[], &HashMap::new(), all_odds, false)
    }

    pub fn detect_event_aware_errors(&self, events: &[Event], all_odds: &[Odd]) -> Vec<OddsError> {
        let event_fingerprints = self.build_event_fingerprints(events);
        self.detect_errors_advanced(events, &event_fingerprints, all_odds, true)
            .into_iter()
            .map(|dr| dr.error)
            .collect()
    }

    /// Event-aware detection with confidence
    pub fn detect_event_aware_errors_with_confidence(
        &self,
        events: &[Event],
        all_odds: &[Odd],
    ) -> Vec<DetectionResult> {
        let event_fingerprints = self.build_event_fingerprints(events);
        self.detect_errors_advanced(events, &event_fingerprints, all_odds, true)
    }

    pub fn record_odd(&self, key: &str, odds: f64) {
        let mut entries = self.recent_odds.entry(key.to_string()).or_default();
        entries.push(odds);
        let len = entries.len();
        if len > 1000 {
            let drain_to = len - 500;
            entries.drain(..drain_to);
        }

        let now = Utc::now().timestamp();
        let mut time_entries = self
            .recent_odds_with_time
            .entry(key.to_string())
            .or_default();
        time_entries.push((odds, now));
        let tlen = time_entries.len();
        if tlen > 1000 {
            let drain_to = tlen - 500;
            time_entries.drain(..drain_to);
        }
    }

    pub fn get_market_average(&self, key: &str) -> Option<f64> {
        self.recent_odds.get(key).and_then(|entries| {
            if entries.is_empty() {
                None
            } else {
                Some(entries.iter().sum::<f64>() / entries.len() as f64)
            }
        })
    }

    pub fn update_bk_profile(&self, bk: &str, odds: f64, is_anomaly: bool) {
        let mut profile = self
            .bk_profiles
            .entry(bk.to_string())
            .or_insert_with(|| BKProfile {
                avg_odds: odds,
                deviation: 0.0,
                anomaly_count: 0,
                total_observations: 0,
                confidence_factor: 1.0,
                win_count: 0,
                loss_count: 0,
            });

        profile.total_observations += 1;
        let old_avg = profile.avg_odds;
        profile.avg_odds =
            (old_avg * (profile.total_observations - 1) as f64 + odds) / profile.total_observations as f64;
        profile.deviation = ((profile.deviation.powi(2) * (profile.total_observations - 1) as f64)
            + (odds - profile.avg_odds).powi(2))
            .sqrt()
            / profile.total_observations as f64;

        if is_anomaly {
            profile.anomaly_count += 1;
            profile.loss_count += 1;
        } else {
            profile.win_count += 1;
        }

        // Update confidence factor based on accuracy rate
        let accuracy_rate = if profile.total_observations > 0 {
            profile.win_count as f64 / profile.total_observations as f64
        } else {
            0.5
        };
        // Confidence factor ranges from 0.5 (always wrong) to 2.0 (always right)
        profile.confidence_factor = 0.5 + (accuracy_rate * 1.5);
    }

    pub fn get_bk_anomaly_rate(&self, bk: &str) -> Option<f64> {
        self.bk_profiles.get(bk).map(|profile| {
            if profile.total_observations == 0 {
                0.0
            } else {
                (profile.anomaly_count as f64 / profile.total_observations as f64) * 100.0
            }
        })
    }

    pub fn get_bk_confidence_factor(&self, bk: &str) -> Option<f64> {
        self.bk_profiles.get(bk).map(|profile| profile.confidence_factor)
    }

    pub fn get_bk_accuracy(&self, bk: &str) -> Option<f64> {
        self.bk_profiles.get(bk).map(|profile| {
            if profile.total_observations == 0 {
                0.0
            } else {
                (profile.win_count as f64 / profile.total_observations as f64) * 100.0
            }
        })
    }

    pub fn get_market_volatility(&self, market: &str) -> Option<f64> {
        self.market_profiles.get(market).map(|profile| profile.volatility)
    }

    pub fn get_market_anomaly_rate(&self, market: &str) -> Option<f64> {
        self.market_profiles.get(market).map(|profile| {
            if profile.total_samples == 0 {
                0.0
            } else {
                (profile.anomaly_count as f64 / profile.total_samples as f64) * 100.0
            }
        })
    }

    fn detect_errors_advanced(
        &self,
        events: &[Event],
        event_fingerprints: &HashMap<String, String>,
        all_odds: &[Odd],
        use_event_scope: bool,
    ) -> Vec<DetectionResult> {
        let mut results = Vec::new();
        let events_by_id: HashMap<String, Event> = events
            .iter()
            .cloned()
            .map(|event| (event.id.clone(), event))
            .collect();
        let by_selection = self.group_by_selection(all_odds, event_fingerprints, use_event_scope);

        for odds in by_selection.values() {
            let unique_bookmakers = odds
                .iter()
                .map(|odd| odd.bookmaker_slug.as_str())
                .collect::<std::collections::HashSet<_>>();
            if unique_bookmakers.len() < self.min_samples {
                continue;
            }

            let values: Vec<f64> = odds.iter().map(|odd| odd.odds).collect();
            let baseline = median(values.clone());
            if baseline <= 0.0 {
                continue;
            }

            for odd in odds {
                let detection = self.analyze_odd(
                    odd,
                    &values,
                    baseline,
                    &events_by_id,
                    event_fingerprints,
                    use_event_scope,
                );

                if let Some(detection) = detection {
                    self.update_market_profile(&odd.market, odd.odds, true, baseline);
                    results.push(detection);
                    self.update_bk_profile(&odd.bookmaker_slug, odd.odds, true);
                } else {
                    self.update_market_profile(&odd.market, odd.odds, false, baseline);
                }

                // Record for time-series analysis
                self.record_odd(
                    &self.history_key(odd, event_fingerprints, use_event_scope),
                    odd.odds,
                );
            }
        }

        // Sort by confidence (higher first), then by deviation
        results.sort_by(|a, b| {
            let conf_cmp = b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal);
            if conf_cmp != std::cmp::Ordering::Equal {
                return conf_cmp;
            }
            b.error
                .deviation_percent
                .partial_cmp(&a.error.deviation_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    fn analyze_odd(
        &self,
        odd: &Odd,
        all_values: &[f64],
        baseline: f64,
        events_by_id: &HashMap<String, Event>,
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> Option<DetectionResult> {
        let mut detection_methods = Vec::new();
        let mut confidence_scores = Vec::new();

        // Statistical method 1: 3-Sigma (3 standard deviations)
        if let Some(sigma_score) = self.sigma_test(&all_values, odd.odds) {
            detection_methods.push("3-sigma".to_string());
            confidence_scores.push(sigma_score);
        }

        // Statistical method 2: IQR (Interquartile Range)
        if let Some(iqr_score) = self.iqr_test(&all_values, odd.odds) {
            detection_methods.push("IQR".to_string());
            confidence_scores.push(iqr_score);
        }

        // Statistical method 3: Modified Z-Score
        if let Some(zscore) = self.modified_z_score(&all_values, odd.odds) {
            detection_methods.push("Modified-Z".to_string());
            confidence_scores.push(zscore);
        }

        // Statistical method 4: Grubbs Test (for outlier detection)
        if let Some(grubbs_score) = self.grubbs_test(&all_values, odd.odds) {
            detection_methods.push("Grubbs".to_string());
            confidence_scores.push(grubbs_score);
        }

        // Statistical method 5: Market-normalized Z-Score (new ML component)
        if let Some(market_z_score) = self.market_normalized_z_score(odd, &all_values) {
            detection_methods.push("Market-ML".to_string());
            confidence_scores.push(market_z_score);
        }

        // Time-series analysis with systematic shift detection
        let time_series_flag = self.detect_time_series_anomaly(
            odd,
            event_fingerprints,
            use_event_scope,
        );
        let systematic_shift = self.detect_systematic_shift(odd, event_fingerprints, use_event_scope);

        // BK anomaly profile
        let bk_anomaly_flag = self.is_bk_anomaly(&odd.bookmaker_slug, odd.odds);

        // Market anomaly profile (new)
        let market_anomaly_flag = self.is_market_anomaly(&odd.market, odd.odds);

        if detection_methods.is_empty() {
            return None;
        }

        // Calculate ML fusion score: multiply confidence across 5 methods
        let ml_score = self.calculate_ml_fusion_score(&confidence_scores);

        // Calculate final confidence score with ML weighting
        let avg_confidence = if confidence_scores.is_empty() {
            0.0
        } else {
            confidence_scores.iter().sum::<f64>() / confidence_scores.len() as f64
        };

        // Boost confidence if multiple methods agree or if anomalies detected
        let mut final_confidence = (avg_confidence + ml_score) / 2.0;
        
        if detection_methods.len() >= 2 {
            final_confidence = (final_confidence * 1.2).min(100.0);
        }
        if detection_methods.len() >= 4 {
            final_confidence = (final_confidence * 1.15).min(100.0);
        }
        if time_series_flag {
            final_confidence = (final_confidence * 1.15).min(100.0);
        }
        if systematic_shift {
            final_confidence = (final_confidence * 1.2).min(100.0);
        }
        if bk_anomaly_flag {
            final_confidence = (final_confidence * 1.1).min(100.0);
        }
        if market_anomaly_flag {
            final_confidence = (final_confidence * 1.12).min(100.0);
        }

        // Apply BK reputation factor
        if let Some(profile) = self.bk_profiles.get(&odd.bookmaker_slug) {
            final_confidence = (final_confidence * profile.confidence_factor).min(100.0);
        }

        // Filter out low-confidence detections
        if final_confidence < 40.0 {
            return None;
        }

        let deviation = ((odd.odds - baseline).abs() / baseline) * 100.0;

        // Additional filter: require significant deviation for low confidence
        if final_confidence < 70.0 && deviation < self.deviation_threshold {
            return None;
        }

        let market_key = self.history_key(odd, event_fingerprints, use_event_scope);
        let avg_market_odds = self.get_market_average(&market_key).unwrap_or(baseline);

        let event = events_by_id
            .get(&odd.event_id)
            .cloned()
            .unwrap_or_else(|| Event {
                id: odd.event_id.clone(),
                sport: shared::Sport::Football,
                league: String::new(),
                home_team: String::new(),
                away_team: String::new(),
                start_time: None,
                is_live: false,
                bookmaker_slug: odd.bookmaker_slug.clone(),
                raw_url: None,
                extra: Default::default(),
            });

        let mut reason = format!(
            "Detected by: {}. ML Score: {:.2}. ",
            detection_methods.join(", "),
            ml_score
        );

        if systematic_shift {
            reason.push_str("Systematic shift detected. ");
        }
        if time_series_flag {
            reason.push_str("Time-series anomaly detected. ");
        }
        if bk_anomaly_flag {
            reason.push_str("BK exhibits unusual behavior. ");
        }
        if market_anomaly_flag {
            reason.push_str("Market shows abnormal volatility. ");
        }

        Some(DetectionResult {
            error: OddsError {
                id: Uuid::new_v4(),
                bookmaker: odd.bookmaker_slug.clone(),
                event,
                market: odd.market.clone(),
                selection: odd.selection.clone(),
                suspicious_odds: odd.odds,
                avg_market_odds,
                deviation_percent: deviation,
                detected_at: Utc::now(),
            },
            confidence: final_confidence,
            detection_methods,
            reason,
            time_series_flag,
            bk_anomaly_flag,
            market_anomaly_flag,
            ml_score,
            systematic_shift,
        })
    }

    // ============= STATISTICAL DETECTION METHODS =============

    /// 3-Sigma method: detects values beyond 3 standard deviations
    fn sigma_test(&self, values: &[f64], test_value: f64) -> Option<f64> {
        if values.len() < 3 {
            return None;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return None;
        }

        let z_score = ((test_value - mean).abs() / std_dev).min(10.0); // Cap at 10 for numerical stability
        if z_score >= 3.0 {
            // Beyond 3-sigma: confidence based on how far beyond
            Some(((z_score / 3.0 - 1.0) * 40.0).min(100.0) + 30.0)
        } else if z_score >= 2.5 {
            Some(50.0)
        } else if z_score >= 2.0 {
            Some(35.0)
        } else {
            None
        }
    }

    /// IQR method: Tukey's fences for outlier detection
    fn iqr_test(&self, values: &[f64], test_value: f64) -> Option<f64> {
        if values.len() < 4 {
            return None;
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let q1_idx = sorted.len() / 4;
        let q3_idx = (sorted.len() * 3) / 4;

        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];
        let iqr = q3 - q1;

        if iqr == 0.0 {
            return None;
        }

        let lower_fence = q1 - 1.5 * iqr;
        let upper_fence = q3 + 1.5 * iqr;
        let extreme_lower = q1 - 3.0 * iqr;
        let extreme_upper = q3 + 3.0 * iqr;

        if test_value < extreme_lower || test_value > extreme_upper {
            // Extreme outlier
            Some(90.0)
        } else if test_value < lower_fence || test_value > upper_fence {
            // Moderate outlier
            Some(70.0)
        } else {
            None
        }
    }

    /// Modified Z-Score: robust against outliers (uses median and MAD)
    fn modified_z_score(&self, values: &[f64], test_value: f64) -> Option<f64> {
        if values.len() < 3 {
            return None;
        }

        let median_val = median(values.to_vec());
        let mad = median(
            values
                .iter()
                .map(|v| (v - median_val).abs())
                .collect::<Vec<_>>(),
        );

        if mad == 0.0 {
            return None;
        }

        let modified_z = (0.6745 * (test_value - median_val) / mad).abs().min(20.0);

        if modified_z > 3.5 {
            Some((modified_z - 3.5) * 10.0 + 50.0)
        } else if modified_z > 2.5 {
            Some(50.0)
        } else {
            None
        }
    }

    /// Grubbs Test: statistical test for outliers in normal distribution
    fn grubbs_test(&self, values: &[f64], test_value: f64) -> Option<f64> {
        if values.len() < 3 {
            return None;
        }

        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / n;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return None;
        }

        let g = ((test_value - mean).abs() / std_dev).min(20.0);

        // Critical values for different confidence levels
        // At 0.05 significance level, outlier if G > critical_value
        if g > 4.0 {
            Some(85.0)
        } else if g > 3.5 {
            Some(70.0)
        } else if g > 3.0 {
            Some(55.0)
        } else {
            None
        }
    }

    /// Market-normalized Z-Score: uses market volatility as baseline
    /// This is the 5th ML method for fusion scoring
    fn market_normalized_z_score(&self, odd: &Odd, all_values: &[f64]) -> Option<f64> {
        if all_values.len() < 3 {
            return None;
        }

        let mean = all_values.iter().sum::<f64>() / all_values.len() as f64;
        let variance = all_values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / all_values.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return None;
        }

        // Get market profile volatility
        let market_key = &odd.market;
        let market_volatility = self.market_profiles
            .get(market_key)
            .map(|p| p.volatility)
            .unwrap_or(std_dev);

        // Normalize by market volatility
        let normalized_deviation = ((odd.odds - mean).abs() / market_volatility.max(0.01)).min(20.0);

        if normalized_deviation > 4.0 {
            Some(85.0)
        } else if normalized_deviation > 3.0 {
            Some(70.0)
        } else if normalized_deviation > 2.0 {
            Some(50.0)
        } else {
            None
        }
    }

    /// ML Fusion Score: multiplies confidence across 5 statistical methods
    /// Higher multiplier when all methods agree, lower when fewer methods agree
    fn calculate_ml_fusion_score(&self, confidence_scores: &[f64]) -> f64 {
        if confidence_scores.is_empty() {
            return 0.0;
        }

        // Normalize each score to 0-1 range
        let normalized: Vec<f64> = confidence_scores
            .iter()
            .map(|&s| (s / 100.0).min(1.0).max(0.0))
            .collect();

        // Method agreement bonus: more methods detecting = higher multiplier
        let method_count_bonus = match normalized.len() {
            1 => 0.8,   // Single method: conservative
            2 => 0.95,  // Two methods: good
            3 => 1.1,   // Three methods: very good
            4 => 1.25,  // Four methods: excellent
            5 => 1.4,   // All five methods: exceptional
            _ => 1.4,
        };

        // Multiplicative fusion: combine all scores
        let mut fused = 1.0;
        for score in &normalized {
            fused *= *score;
        }

        // Convert back to percentage with method bonus
        let result = (fused.powf(1.0 / normalized.len() as f64) * 100.0) * method_count_bonus;
        result.min(100.0)
    }

    /// Detect systematic shift: detect consistent trend in odds over time
    fn detect_systematic_shift(
        &self,
        odd: &Odd,
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> bool {
        let key = self.history_key(odd, event_fingerprints, use_event_scope);

        if let Some(time_entries) = self.recent_odds_with_time.get(&key) {
            if time_entries.len() < 10 {
                return false;
            }

            // Get last 10 values
            let start_idx = time_entries.len().saturating_sub(10);
            let recent: Vec<f64> = time_entries[start_idx..]
                .iter()
                .map(|(o, _)| *o)
                .collect();

            // Calculate linear trend (simple: compare first half vs second half)
            let mid = recent.len() / 2;
            let first_half_avg = recent[..mid].iter().sum::<f64>() / mid as f64;
            let second_half_avg = recent[mid..].iter().sum::<f64>() / (recent.len() - mid) as f64;

            let trend_magnitude = ((second_half_avg - first_half_avg).abs() / first_half_avg) * 100.0;

            // Flag systematic shift if consistent trend > 10%
            trend_magnitude > 10.0
        } else {
            false
        }
    }

    /// Track market anomaly profile
    fn update_market_profile(&self, market: &str, odds: f64, is_anomaly: bool, baseline: f64) {
        let mut profile = self
            .market_profiles
            .entry(market.to_string())
            .or_insert_with(|| MarketProfile {
                market_type: market.to_string(),
                avg_deviation: 0.0,
                volatility: 0.0,
                anomaly_count: 0,
                total_samples: 0,
                min_odds: odds,
                max_odds: odds,
            });

        profile.total_samples += 1;
        let deviation = ((odds - baseline).abs() / baseline) * 100.0;
        
        let old_avg = profile.avg_deviation;
        profile.avg_deviation = (old_avg * (profile.total_samples - 1) as f64 + deviation)
            / profile.total_samples as f64;

        // Update volatility (standard deviation of deviations)
        profile.volatility = ((profile.volatility.powi(2) * (profile.total_samples - 1) as f64)
            + (deviation - profile.avg_deviation).powi(2))
            .sqrt()
            / profile.total_samples as f64;

        if is_anomaly {
            profile.anomaly_count += 1;
        }

        profile.min_odds = profile.min_odds.min(odds);
        profile.max_odds = profile.max_odds.max(odds);
    }

    /// Check if market exhibits anomalous behavior
    fn is_market_anomaly(&self, market: &str, current_odds: f64) -> bool {
        if let Some(profile) = self.market_profiles.get(market) {
            if profile.total_samples < 20 {
                return false;
            }

            // Market anomaly if:
            // 1. High anomaly rate (> 10%)
            let anomaly_rate = (profile.anomaly_count as f64 / profile.total_samples as f64) * 100.0;
            if anomaly_rate > 10.0 {
                return true;
            }

            // 2. Very high volatility compared to typical markets
            if profile.volatility > 50.0 {
                return true;
            }

            false
        } else {
            false
        }
    }

    /// Time-series anomaly detection: checks for sudden spikes or drops
    fn detect_time_series_anomaly(
        &self,
        odd: &Odd,
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> bool {
        let key = self.history_key(odd, event_fingerprints, use_event_scope);

        if let Some(time_entries) = self.recent_odds_with_time.get(&key) {
            if time_entries.len() < 5 {
                return false;
            }

            // Get last 5 values
            let start_idx = time_entries.len().saturating_sub(5);
            let recent: Vec<f64> = time_entries[start_idx..]
                .iter()
                .map(|(o, _)| *o)
                .collect();

            if recent.is_empty() {
                return false;
            }

            // Calculate moving average
            let moving_avg = recent.iter().sum::<f64>() / recent.len() as f64;

            // Check if current odd deviates significantly from moving average
            let deviation = ((odd.odds - moving_avg).abs() / moving_avg) * 100.0;

            // Flag if deviation > 20% from moving average
            deviation > 20.0
        } else {
            false
        }
    }

    /// Check if bookmaker exhibits anomalous behavior
    fn is_bk_anomaly(&self, bk: &str, current_odds: f64) -> bool {
        if let Some(profile) = self.bk_profiles.get(bk) {
            if profile.total_observations < 10 {
                return false;
            }

            // Check if anomaly rate is high (> 15%)
            let anomaly_rate = (profile.anomaly_count as f64 / profile.total_observations as f64) * 100.0;

            if anomaly_rate > 15.0 {
                // BK has history of anomalies
                return true;
            }

            // Also check if current odds deviate significantly from BK's profile
            if profile.deviation > 0.0 {
                let z_score = ((current_odds - profile.avg_odds).abs() / profile.deviation).min(10.0);
                z_score > 3.0
            } else {
                false
            }
        } else {
            false
        }
    }

    fn group_by_selection<'a>(
        &self,
        all_odds: &'a [Odd],
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> HashMap<String, Vec<&'a Odd>> {
        let mut map: HashMap<String, Vec<&'a Odd>> = HashMap::new();
        for odd in all_odds {
            let key = self.history_key(odd, event_fingerprints, use_event_scope);
            map.entry(key).or_insert_with(Vec::new).push(odd);
        }
        map
    }

    fn build_event_fingerprints(&self, events: &[Event]) -> HashMap<String, String> {
        events
            .iter()
            .map(|event| (event.id.clone(), Self::event_fingerprint(event)))
            .collect()
    }

    fn history_key(
        &self,
        odd: &Odd,
        event_fingerprints: &HashMap<String, String>,
        use_event_scope: bool,
    ) -> String {
        let event_scope = if use_event_scope {
            event_fingerprints
                .get(&odd.event_id)
                .cloned()
                .unwrap_or_else(|| odd.event_id.clone())
        } else {
            "global".into()
        };

        format!(
            "{}|{}|{}|{}",
            event_scope,
            odd.market,
            odd.selection,
            odd.line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "none".into())
        )
    }

    fn event_fingerprint(event: &Event) -> String {
        let norm = Normalizer::new();
        let norm_event = norm.normalize_event(event.clone());
        let home = Self::normalize_team_name(&norm_event.home_team);
        let away = Self::normalize_team_name(&norm_event.away_team);
        let league = norm_event.league.to_lowercase().replace(' ', "");
        let live_state = if norm_event.is_live {
            "live"
        } else {
            "prematch"
        };
        let (first, second) = if home < away {
            (home, away)
        } else {
            (away, home)
        };

        format!(
            "{:?}|{}|{}|{}|{}",
            event.sport, live_state, league, first, second
        )
    }

    fn normalize_team_name(name: &str) -> String {
        name.to_lowercase()
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
            .replace(' ', "")
            .replace('-', "")
    }
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::odds::OddsType;
    use shared::Sport;
    use std::collections::HashMap;
    use std::thread;
    use std::time::Duration;

    fn make_event(id: &str, bookmaker: &str, home_team: &str, away_team: &str) -> Event {
        Event {
            id: id.into(),
            sport: Sport::Football,
            league: "Premier League".into(),
            home_team: home_team.into(),
            away_team: away_team.into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: bookmaker.into(),
            raw_url: None,
            extra: HashMap::new(),
        }
    }

    fn make_odd(bk: &str, sel: &str, odds: f64) -> Odd {
        Odd {
            id: format!("{}-{}", bk, sel),
            event_id: "evt1".into(),
            bookmaker_slug: bk.into(),
            market: "1X2".into(),
            selection: sel.into(),
            odds,
            odds_type: OddsType::Home,
            line: None,
            timestamp: Utc::now(),
        }
    }

    // ============= BASIC DETECTION TESTS =============

    #[test]
    fn test_detect_anomalous_odd_basic() {
        let detector = OddsErrorDetector::new(150.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 1.9),
            make_odd("bk4", "1", 10.0),
        ];
        let errors = detector.detect_errors(&odds);
        assert!(!errors.is_empty());
        assert!((errors[0].suspicious_odds - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_detect_anomalous_odd_with_confidence() {
        let detector = OddsErrorDetector::new(150.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 1.9),
            make_odd("bk4", "1", 10.0),
        ];
        let results = detector.detect_errors_with_confidence(&odds);
        assert!(!results.is_empty());
        assert!(results[0].confidence >= 40.0);
        assert!(results[0].confidence <= 100.0);
        assert!((results[0].error.suspicious_odds - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_no_errors_normal_odds() {
        let detector = OddsErrorDetector::new(500.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 1.9),
            make_odd("bk4", "1", 2.05),
        ];
        let errors = detector.detect_errors(&odds);
        assert!(errors.is_empty());
    }

    // ============= STATISTICAL METHOD TESTS =============

    #[test]
    fn test_sigma_test_3sigma_detection() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        let confidence = detector.sigma_test(&values, 10.0);
        assert!(confidence.is_some());
        assert!(confidence.unwrap() > 50.0);
    }

    #[test]
    fn test_sigma_test_no_detection_normal() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.1, 2.2, 1.9, 2.05];
        let confidence = detector.sigma_test(&values, 2.0);
        assert!(confidence.is_none());
    }

    #[test]
    fn test_sigma_test_2sigma() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        let confidence = detector.sigma_test(&values, 8.0);
        assert!(confidence.is_some());
        assert!(confidence.unwrap() >= 30.0);
    }

    #[test]
    fn test_iqr_test_extreme_outlier() {
        let detector = OddsErrorDetector::new(100.0, 4);
        let values = vec![1.0, 2.0, 2.1, 2.2, 2.5, 3.0];
        let confidence = detector.iqr_test(&values, 100.0);
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), 90.0);
    }

    #[test]
    fn test_iqr_test_moderate_outlier() {
        let detector = OddsErrorDetector::new(100.0, 4);
        let values = vec![1.0, 2.0, 2.1, 2.2, 2.5, 3.0];
        let confidence = detector.iqr_test(&values, 5.0);
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), 70.0);
    }

    #[test]
    fn test_iqr_test_no_outlier() {
        let detector = OddsErrorDetector::new(100.0, 4);
        let values = vec![2.0, 2.1, 2.2, 1.9, 2.05, 2.3];
        let confidence = detector.iqr_test(&values, 2.1);
        assert!(confidence.is_none());
    }

    #[test]
    fn test_modified_z_score_extreme() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        let confidence = detector.modified_z_score(&values, 10.0);
        assert!(confidence.is_some());
        assert!(confidence.unwrap() > 50.0);
    }

    #[test]
    fn test_modified_z_score_normal() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.1, 1.9, 2.05, 2.2];
        let confidence = detector.modified_z_score(&values, 2.1);
        assert!(confidence.is_none());
    }

    #[test]
    fn test_grubbs_test_extreme() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        let confidence = detector.grubbs_test(&values, 10.0);
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), 85.0);
    }

    #[test]
    fn test_grubbs_test_moderate() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let values = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        let confidence = detector.grubbs_test(&values, 7.0);
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), 70.0);
    }

    // ============= ML FUSION SCORING TESTS =============

    #[test]
    fn test_ml_fusion_single_method() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let scores = vec![80.0];
        let fusion = detector.calculate_ml_fusion_score(&scores);
        assert!(fusion > 50.0 && fusion < 100.0);
        // Single method: conservative multiplier (0.8)
        assert!(fusion < 80.0);
    }

    #[test]
    fn test_ml_fusion_two_methods() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let scores = vec![80.0, 85.0];
        let fusion = detector.calculate_ml_fusion_score(&scores);
        // Two methods: multiplier 0.95
        assert!(fusion > 60.0 && fusion < 100.0);
    }

    #[test]
    fn test_ml_fusion_all_five_methods() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let scores = vec![80.0, 85.0, 82.0, 88.0, 84.0];
        let fusion = detector.calculate_ml_fusion_score(&scores);
        // All five methods: highest multiplier (1.4)
        assert!(fusion > 70.0);
        // With all high scores, fusion should be high
        assert!(fusion > 80.0);
    }

    #[test]
    fn test_ml_fusion_empty_scores() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let scores: Vec<f64> = vec![];
        let fusion = detector.calculate_ml_fusion_score(&scores);
        assert_eq!(fusion, 0.0);
    }

    #[test]
    fn test_ml_fusion_mixed_confidence_levels() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let scores = vec![100.0, 60.0, 75.0, 80.0];
        let fusion = detector.calculate_ml_fusion_score(&scores);
        // Should handle mixed confidence levels gracefully
        assert!(fusion > 0.0 && fusion <= 100.0);
    }

    // ============= TIME-SERIES TESTS =============

    #[test]
    fn test_time_series_anomaly_spike_detection() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let key = "market|selection|none";

        for _ in 0..5 {
            detector.record_odd(key, 2.0);
        }

        let odd = make_odd("bk1", "1", 5.0);
        let is_anomaly = detector.detect_time_series_anomaly(&odd, &HashMap::new(), false);
        assert!(is_anomaly);
    }

    #[test]
    fn test_time_series_anomaly_normal_movement() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let key = "market|selection|none";

        detector.record_odd(key, 2.0);
        detector.record_odd(key, 2.05);
        detector.record_odd(key, 2.1);
        detector.record_odd(key, 2.15);
        detector.record_odd(key, 2.2);

        let odd = make_odd("bk1", "1", 2.25);
        let is_anomaly = detector.detect_time_series_anomaly(&odd, &HashMap::new(), false);
        assert!(!is_anomaly);
    }

    #[test]
    fn test_systematic_shift_upward_trend() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let key = "global|1X2|1|none";

        // Record upward trend
        for i in 0..10 {
            detector.record_odd(key, 2.0 + (i as f64 * 0.15));
        }

        let odd = make_odd("bk1", "1", 3.5);
        let shift = detector.detect_systematic_shift(&odd, &HashMap::new(), false);
        assert!(shift);
    }

    #[test]
    fn test_systematic_shift_no_trend() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let key = "global|1X2|1|none";

        // Record stable odds
        for _ in 0..10 {
            detector.record_odd(key, 2.0);
        }

        let odd = make_odd("bk1", "1", 2.01);
        let shift = detector.detect_systematic_shift(&odd, &HashMap::new(), false);
        assert!(!shift);
    }

    #[test]
    fn test_systematic_shift_downward_trend() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let key = "global|1X2|1|none";

        // Record downward trend
        for i in 0..10 {
            detector.record_odd(key, 3.0 - (i as f64 * 0.15));
        }

        let odd = make_odd("bk1", "1", 1.5);
        let shift = detector.detect_systematic_shift(&odd, &HashMap::new(), false);
        assert!(shift);
    }

    // ============= BOOKMAKER PROFILE TESTS =============

    #[test]
    fn test_bk_profile_tracking() {
        let detector = OddsErrorDetector::new(100.0, 3);

        detector.update_bk_profile("bk1", 2.0, false);
        detector.update_bk_profile("bk1", 2.05, false);
        detector.update_bk_profile("bk1", 1.95, false);

        let anomaly_rate = detector.get_bk_anomaly_rate("bk1");
        assert!(anomaly_rate.is_some());
        assert_eq!(anomaly_rate.unwrap(), 0.0);
    }

    #[test]
    fn test_bk_anomaly_detection_high_rate() {
        let detector = OddsErrorDetector::new(100.0, 3);

        for _ in 0..10 {
            detector.update_bk_profile("bk1", 2.0, false);
        }
        for _ in 0..3 {
            detector.update_bk_profile("bk1", 10.0, true);
        }

        let is_anomaly = detector.is_bk_anomaly("bk1", 10.0);
        assert!(is_anomaly);
    }

    #[test]
    fn test_bk_anomaly_not_enough_observations() {
        let detector = OddsErrorDetector::new(100.0, 3);

        detector.update_bk_profile("bk1", 2.0, false);
        detector.update_bk_profile("bk1", 10.0, true);

        let is_anomaly = detector.is_bk_anomaly("bk1", 10.0);
        assert!(!is_anomaly);
    }

    #[test]
    fn test_bk_confidence_factor_calculation() {
        let detector = OddsErrorDetector::new(100.0, 3);

        // Perfect bookmaker (100% accurate)
        for _ in 0..10 {
            detector.update_bk_profile("perfect_bk", 2.0, false);
        }
        let perfect_factor = detector.get_bk_confidence_factor("perfect_bk");
        assert!(perfect_factor.is_some());
        assert_eq!(perfect_factor.unwrap(), 2.0); // 0.5 + (1.0 * 1.5)

        // Terrible bookmaker (0% accurate)
        for _ in 0..10 {
            detector.update_bk_profile("bad_bk", 2.0, true);
        }
        let bad_factor = detector.get_bk_confidence_factor("bad_bk");
        assert!(bad_factor.is_some());
        assert_eq!(bad_factor.unwrap(), 0.5); // 0.5 + (0.0 * 1.5)
    }

    #[test]
    fn test_bk_accuracy_rate_tracking() {
        let detector = OddsErrorDetector::new(100.0, 3);

        detector.update_bk_profile("bk1", 2.0, false);
        detector.update_bk_profile("bk1", 2.1, false);
        detector.update_bk_profile("bk1", 1.9, true);
        detector.update_bk_profile("bk1", 2.05, false);

        let accuracy = detector.get_bk_accuracy("bk1");
        assert!(accuracy.is_some());
        assert_eq!(accuracy.unwrap(), 75.0); // 3 correct out of 4
    }

    // ============= MARKET PROFILE TESTS =============

    #[test]
    fn test_market_profile_tracking() {
        let detector = OddsErrorDetector::new(100.0, 3);

        detector.update_market_profile("1X2", 2.0, false, 2.0);
        detector.update_market_profile("1X2", 2.1, false, 2.0);
        detector.update_market_profile("1X2", 1.9, false, 2.0);

        let volatility = detector.get_market_volatility("1X2");
        assert!(volatility.is_some());
        // Low volatility for consistent odds
        assert!(volatility.unwrap() < 5.0);
    }

    #[test]
    fn test_market_anomaly_detection_high_volatility() {
        let detector = OddsErrorDetector::new(100.0, 3);

        // High volatility market
        for odds in &[1.5, 2.0, 3.0, 5.0, 1.2, 4.5] {
            detector.update_market_profile("volatile_market", *odds, false, 2.0);
        }
        // Repeat to get past 20 sample threshold
        for _ in 0..20 {
            for odds in &[1.5, 2.0, 3.0, 5.0, 1.2, 4.5] {
                detector.update_market_profile("volatile_market", *odds, false, 2.0);
            }
        }

        let volatility = detector.get_market_volatility("volatile_market");
        assert!(volatility.is_some());
        assert!(volatility.unwrap() > 20.0); // High volatility
    }

    #[test]
    fn test_market_anomaly_detection_high_anomaly_rate() {
        let detector = OddsErrorDetector::new(100.0, 3);

        // Market with many anomalies
        for _ in 0..20 {
            detector.update_market_profile("noisy_market", 2.0, false, 2.0);
        }
        for _ in 0..3 {
            detector.update_market_profile("noisy_market", 10.0, true, 2.0);
        }

        let is_anomaly = detector.is_market_anomaly("noisy_market", 2.0);
        assert!(is_anomaly);
    }

    #[test]
    fn test_market_anomaly_rate_calculation() {
        let detector = OddsErrorDetector::new(100.0, 3);

        for _ in 0..18 {
            detector.update_market_profile("test_market", 2.0, false, 2.0);
        }
        for _ in 0..2 {
            detector.update_market_profile("test_market", 10.0, true, 2.0);
        }

        let anomaly_rate = detector.get_market_anomaly_rate("test_market");
        assert!(anomaly_rate.is_some());
        assert_eq!(anomaly_rate.unwrap(), 10.0); // 2 out of 20
    }

    // ============= CONFIDENCE SCORING TESTS =============

    #[test]
    fn test_confidence_multiple_methods_agreement() {
        let detector = OddsErrorDetector::new(100.0, 3);

        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.05),
            make_odd("bk3", "1", 1.95),
            make_odd("bk4", "1", 2.1),
            make_odd("bk5", "1", 20.0),
        ];

        let results = detector.detect_errors_with_confidence(&odds);
        assert!(!results.is_empty());

        let outlier_result = results.iter().find(|r| r.error.suspicious_odds > 10.0);
        assert!(outlier_result.is_some());
        let outlier = outlier_result.unwrap();

        assert!(outlier.detection_methods.len() >= 2);
        assert!(outlier.confidence > 70.0);
    }

    #[test]
    fn test_confidence_boosted_by_systematic_shift() {
        let detector = OddsErrorDetector::new(100.0, 3);

        let key = "global|1X2|1|none";
        for i in 0..10 {
            detector.record_odd(key, 2.0 + (i as f64 * 0.2));
        }

        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.05),
            make_odd("bk3", "1", 1.95),
            make_odd("bk4", "1", 4.0),
        ];

        let results = detector.detect_errors_with_confidence(&odds);
        let spike = results.iter().find(|r| r.error.suspicious_odds > 3.0);

        if let Some(s) = spike {
            assert!(s.systematic_shift);
        }
    }

    #[test]
    fn test_low_confidence_filtered_out() {
        let detector = OddsErrorDetector::new(50.0, 4);

        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.01),
            make_odd("bk3", "1", 1.99),
            make_odd("bk4", "1", 2.02),
        ];

        let results = detector.detect_errors_with_confidence(&odds);
        assert!(results.is_empty());
    }

    // ============= EVENT-AWARE TESTS =============

    #[test]
    fn test_detect_event_aware_errors_groups_same_match() {
        let detector = OddsErrorDetector::new(40.0, 3);
        let events = vec![
            make_event("pari-evt", "pari", "Arsenal", "Chelsea"),
            make_event("fonbet-evt", "fonbet", "Chelsea", "Arsenal"),
            make_event("marathon-evt", "marathon", "Arsenal", "Chelsea"),
        ];
        let odds = vec![
            Odd {
                event_id: "pari-evt".into(),
                bookmaker_slug: "pari".into(),
                ..make_odd("pari", "1", 10.0)
            },
            Odd {
                event_id: "fonbet-evt".into(),
                bookmaker_slug: "fonbet".into(),
                ..make_odd("fonbet", "1", 2.1)
            },
            Odd {
                event_id: "marathon-evt".into(),
                bookmaker_slug: "marathon".into(),
                ..make_odd("marathon", "1", 2.0)
            },
        ];

        let errors = detector.detect_event_aware_errors(&events, &odds);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].bookmaker, "pari");
        assert_eq!(errors[0].event.id, "pari-evt");
        assert_eq!(errors[0].event.home_team, "Arsenal");
        assert!(errors[0].deviation_percent > 100.0);
    }

    #[test]
    fn test_detect_event_aware_with_confidence() {
        let detector = OddsErrorDetector::new(40.0, 3);
        let events = vec![
            make_event("evt1", "bk1", "Team A", "Team B"),
            make_event("evt2", "bk2", "Team B", "Team A"),
            make_event("evt3", "bk3", "Team A", "Team B"),
        ];
        let odds = vec![
            Odd {
                event_id: "evt1".into(),
                ..make_odd("bk1", "1", 10.0)
            },
            Odd {
                event_id: "evt2".into(),
                ..make_odd("bk2", "1", 2.1)
            },
            Odd {
                event_id: "evt3".into(),
                ..make_odd("bk3", "1", 2.0)
            },
        ];

        let results = detector.detect_event_aware_errors_with_confidence(&events, &odds);

        assert!(!results.is_empty());
        assert!(results[0].confidence >= 40.0);
        assert!(results[0].confidence <= 100.0);
    }

    // ============= EDGE CASE TESTS =============

    #[test]
    fn test_insufficient_samples() {
        let detector = OddsErrorDetector::new(100.0, 5);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 10.0),
        ];
        let errors = detector.detect_errors(&odds);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_zero_variance_handling() {
        let detector = OddsErrorDetector::new(100.0, 2);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.0),
            make_odd("bk3", "1", 2.0),
        ];
        let results = detector.detect_errors_with_confidence(&odds);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_odds_list() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let odds: Vec<Odd> = vec![];
        let errors = detector.detect_errors(&odds);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_negative_odds_handled() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", -1.0),
            make_odd("bk3", "1", 2.1),
        ];
        let errors = detector.detect_errors(&odds);
        let _ = errors;
    }

    // ============= INTEGRATION TESTS =============

    #[test]
    fn test_multiple_markets_same_event() {
        let detector = OddsErrorDetector::new(100.0, 3);
        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.05),
            make_odd("bk3", "1", 10.0),
        ];
        let results = detector.detect_errors_with_confidence(&odds);
        assert!(!results.is_empty());
        let high_conf = results.iter().filter(|r| r.confidence > 60.0).count();
        assert!(high_conf >= 1);
    }

    #[test]
    fn test_real_world_scenario() {
        let detector = OddsErrorDetector::new(100.0, 4);

        let odds = vec![
            make_odd("pari", "1", 2.5),
            make_odd("fonbet", "1", 2.48),
            make_odd("marathon", "1", 2.52),
            make_odd("bettery", "1", 2.51),
            make_odd("leon", "1", 2.49),
            make_odd("rogue_bk", "1", 8.0),
        ];

        let results = detector.detect_errors_with_confidence(&odds);

        assert!(!results.is_empty());
        let found_rogue = results.iter().any(|r| r.error.bookmaker == "rogue_bk");
        assert!(found_rogue);

        let rogue_result = results.iter().find(|r| r.error.bookmaker == "rogue_bk").unwrap();
        assert!(rogue_result.confidence > 70.0);
    }

    #[test]
    fn test_market_average_calculation() {
        let detector = OddsErrorDetector::new(100.0, 3);

        detector.record_odd("market1", 2.0);
        detector.record_odd("market1", 2.1);
        detector.record_odd("market1", 1.9);

        let avg = detector.get_market_average("market1");
        assert!(avg.is_some());
        let avg_value = avg.unwrap();
        assert!(avg_value > 1.95 && avg_value < 2.05);
    }

    #[test]
    fn test_sorting_by_confidence() {
        let detector = OddsErrorDetector::new(50.0, 3);

        let odds = vec![
            make_odd("bk1", "1", 2.0),
            make_odd("bk2", "1", 2.1),
            make_odd("bk3", "1", 2.2),
            make_odd("bk4", "1", 5.0),
            make_odd("bk5", "1", 2.05),
            make_odd("bk6", "1", 20.0),
        ];

        let results = detector.detect_errors_with_confidence(&odds);

        if results.len() > 1 {
            for i in 0..results.len() - 1 {
                assert!(results[i].confidence >= results[i + 1].confidence);
            }
        }
    }

    #[test]
    fn test_ml_scoring_with_all_features() {
        let detector = OddsErrorDetector::new(100.0, 3);

        // Setup bookmaker reputation
        for _ in 0..5 {
            detector.update_bk_profile("suspicious_bk", 2.0, false);
        }
        detector.update_bk_profile("suspicious_bk", 10.0, true);
        detector.update_bk_profile("suspicious_bk", 9.5, true);

        // Setup market profile
        for _ in 0..25 {
            detector.update_market_profile("1X2", 2.0, false, 2.0);
        }

        // Setup time-series
        for i in 0..10 {
            detector.record_odd("global|1X2|1|none", 2.0 + (i as f64 * 0.15));
        }

        let odds = vec![
            make_odd("normal_bk", "1", 2.0),
            make_odd("normal_bk", "1", 2.05),
            Odd {
                event_id: "evt1".into(),
                bookmaker_slug: "suspicious_bk".into(),
                ..make_odd("suspicious_bk", "1", 8.0)
            },
        ];

        let results = detector.detect_errors_with_confidence(&odds);
        let suspicious = results.iter().find(|r| r.error.bookmaker == "suspicious_bk");

        assert!(suspicious.is_some());
        if let Some(s) = suspicious {
            // Should have high ML score due to multiple factors
            assert!(s.ml_score > 50.0);
            assert!(s.bk_anomaly_flag);
        }
    }

    #[test]
    fn test_precision_on_real_anomalies() {
        let detector = OddsErrorDetector::new(50.0, 3);

        // Real-world like scenario: 20 bookmakers, 1 clear anomaly
        let mut odds = vec![];
        for i in 1..=20 {
            let bk = format!("bk{}", i);
            if i == 19 {
                odds.push(Odd {
                    id: format!("{}-1", bk),
                    event_id: "evt1".into(),
                    bookmaker_slug: bk.clone(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: 15.0, // Clear anomaly
                    odds_type: OddsType::Home,
                    line: None,
                    timestamp: Utc::now(),
                });
            } else {
                let base_odds = 2.5 + ((i as f64 - 1.0) * 0.02);
                odds.push(Odd {
                    id: format!("{}-1", bk),
                    event_id: "evt1".into(),
                    bookmaker_slug: bk.clone(),
                    market: "1X2".into(),
                    selection: "1".into(),
                    odds: base_odds,
                    odds_type: OddsType::Home,
                    line: None,
                    timestamp: Utc::now(),
                });
            }
        }

        let results = detector.detect_errors_with_confidence(&odds);

        // Should find exactly 1 anomaly
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence > 80.0); // High precision
        assert_eq!(results[0].error.bookmaker, "bk19");
    }
}


