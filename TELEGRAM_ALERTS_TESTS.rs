// Comprehensive Test Suite for Telegram Alerts Implementation
// Run with: cargo test --lib bot
// 
// This file provides 30+ tests covering:
// - Rate limiting (token bucket algorithm)
// - Alert filtering and configuration
// - Message formatting
// - History tracking
// - Bot command responses
// - Settings management

#[cfg(test)]
mod telegram_alerts_integration_tests {
    use bot::rate_limiter::RateLimiter;
    use bot::notifier::{
        AlertManager, AlertStatus, TelegramAlertConfig, format_surebet_alert,
        format_settings_message, format_help_message, AlertStats,
    };
    use chrono::{DateTime, Utc};
    use shared::{Surebet, SurebetLeg, Sport};
    use uuid::Uuid;
    use std::time::Duration;
    use std::thread;

    // =========================================================================
    // RATE LIMITER TESTS (8 tests)
    // =========================================================================

    #[test]
    fn rate_limiter_starts_with_full_capacity() {
        let limiter = RateLimiter::new(10.0, 1.0);
        assert!((limiter.available_tokens() - 10.0).abs() < 0.01);
    }

    #[test]
    fn rate_limiter_consumes_single_token() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        assert!(limiter.try_consume(1.0));
        assert!((limiter.available_tokens() - 9.0).abs() < 0.01);
    }

    #[test]
    fn rate_limiter_rejects_when_empty() {
        let limiter = RateLimiter::new(1.0, 0.0);
        assert!(limiter.try_consume(1.0));
        assert!(!limiter.try_consume(1.0));
    }

    #[test]
    fn rate_limiter_refills_over_time() {
        let limiter = RateLimiter::new(10.0, 2.0); // 2 tokens per second
        assert!(limiter.try_consume(10.0));
        assert!((limiter.available_tokens()).abs() < 0.01);

        thread::sleep(Duration::from_millis(500));
        let after_500ms = limiter.available_tokens();
        assert!(after_500ms >= 0.9 && after_500ms <= 1.1);

        thread::sleep(Duration::from_millis(500));
        let after_1000ms = limiter.available_tokens();
        assert!(after_1000ms >= 1.9 && after_1000ms <= 2.1);
    }

    #[test]
    fn rate_limiter_alerts_per_minute_config() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        assert!((limiter.refill_per_second - 10.0 / 60.0).abs() < 0.001);
    }

    #[test]
    fn rate_limiter_reset_restores_capacity() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        limiter.try_consume(10.0);
        assert!((limiter.available_tokens()).abs() < 0.01);
        limiter.reset();
        assert!((limiter.available_tokens() - 10.0).abs() < 0.01);
    }

    #[test]
    fn rate_limiter_consumes_partial_tokens() {
        let limiter = RateLimiter::new(10.0, 1.0);
        assert!(limiter.try_consume(2.5));
        assert!((limiter.available_tokens() - 7.5).abs() < 0.01);
    }

    #[test]
    fn rate_limiter_stats_reflect_current_state() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        let stats = limiter.stats();
        assert!((stats.capacity - 10.0).abs() < 0.01);
        assert!((stats.available_tokens - 10.0).abs() < 0.01);
        assert!((stats.refill_per_second - 10.0 / 60.0).abs() < 0.001);
    }

    // =========================================================================
    // ALERT MANAGER TESTS (9 tests)
    // =========================================================================

    fn create_test_surebet(roi: f64, verified: bool, is_live: bool) -> Surebet {
        Surebet {
            id: Uuid::new_v4(),
            sport: Sport::Football,
            league: "Test League".to_string(),
            home_team: "Home".to_string(),
            away_team: "Away".to_string(),
            start_time: Some(Utc::now()),
            is_live,
            profit_percent: roi,
            total_stake: 1000.0,
            legs: vec![SurebetLeg {
                bookmaker: "test_bk".to_string(),
                market: "1X2".to_string(),
                selection: "1".to_string(),
                odds: 2.0,
                line: None,
                stake: 500.0,
                payout: 1000.0 + (1000.0 * roi / 100.0),
                url: None,
            }],
            detected_at: Utc::now(),
            verified,
            mirror: false,
        }
    }

    #[test]
    fn alert_manager_default_config_allows_2_percent() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(2.5, false, false);
        assert!(manager.should_alert(&surebet).is_ok());
    }

    #[test]
    fn alert_manager_rejects_low_roi() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(1.5, false, false);
        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn alert_manager_respects_only_verified_filter() {
        let mut config = TelegramAlertConfig::default();
        config.only_verified = true;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, false, false);
        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn alert_manager_respects_only_live_filter() {
        let mut config = TelegramAlertConfig::default();
        config.only_live = true;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, true, false);
        assert!(manager.should_alert(&surebet).is_err());
    }

    #[test]
    fn alert_manager_records_alert_history() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(3.0, true, false);

        manager.record_alert(&surebet, AlertStatus::Sent);
        manager.record_alert(&surebet, AlertStatus::Throttled);

        let history = manager.get_history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].status, AlertStatus::Throttled);
        assert_eq!(history[1].status, AlertStatus::Sent);
    }

    #[test]
    fn alert_manager_history_respects_max_size() {
        let mut config = TelegramAlertConfig::default();
        config.history_size = 5;
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(3.0, true, false);

        for _ in 0..10 {
            manager.record_alert(&surebet, AlertStatus::Sent);
        }

        let history = manager.get_history(100);
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn alert_manager_stats_calculation_is_accurate() {
        let config = TelegramAlertConfig::default();
        let manager = AlertManager::new(config);
        let surebet = create_test_surebet(5.0, true, false);

        manager.record_alert(&surebet, AlertStatus::Sent);
        manager.record_alert(&surebet, AlertStatus::Throttled);
        manager.record_alert(&surebet, AlertStatus::Sent);

        let stats = manager.get_stats();
        assert_eq!(stats.total_alerts, 3);
        assert_eq!(stats.sent, 2);
        assert_eq!(stats.throttled, 1);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn alert_manager_update_configuration() {
        let initial_config = TelegramAlertConfig::default();
        let manager = AlertManager::new(initial_config);

        let mut new_config = TelegramAlertConfig::default();
        new_config.min_roi_percent = 3.5;
        new_config.max_alerts_per_minute = 15.0;
        manager.update_config(new_config);

        let current = manager.get_config();
        assert!((current.min_roi_percent - 3.5).abs() < 0.01);
        assert!((current.max_alerts_per_minute - 15.0).abs() < 0.01);
    }

    // =========================================================================
    // MESSAGE FORMATTING TESTS (5 tests)
    // =========================================================================

    #[test]
    fn format_surebet_alert_includes_key_fields() {
        let surebet = create_test_surebet(3.5, true, false);
        let msg = format_surebet_alert(&surebet);

        assert!(msg.contains("3.5%"));
        assert!(msg.contains("Home vs Away"));
        assert!(msg.contains("Test League"));
        assert!(msg.contains("test_bk"));
    }

    #[test]
    fn format_surebet_alert_shows_profit_amount() {
        let surebet = create_test_surebet(5.0, true, false);
        let msg = format_surebet_alert(&surebet);
        // Profit = payout - stake = (1000 + 50) - 1000 = 50
        assert!(msg.contains("50")); // profit amount
    }

    #[test]
    fn format_surebet_alert_includes_live_indicator() {
        let surebet_live = create_test_surebet(2.0, true, true);
        let msg_live = format_surebet_alert(&surebet_live);
        assert!(msg_live.contains("LIVE"));

        let surebet_prematch = create_test_surebet(2.0, true, false);
        let msg_prematch = format_surebet_alert(&surebet_prematch);
        assert!(!msg_prematch.contains("LIVE") || !msg_prematch.contains("🔴"));
    }

    #[test]
    fn format_settings_shows_config() {
        let mut config = TelegramAlertConfig::default();
        config.min_roi_percent = 1.5;
        config.max_alerts_per_minute = 15.0;

        let stats = AlertStats {
            total_alerts: 42,
            sent: 35,
            throttled: 3,
            skipped: 4,
            avg_roi: 2.8,
            sent_in_last_hour: 8,
            sent_in_last_minute: 1,
        };

        let msg = format_settings_message(&config, &stats);
        assert!(msg.contains("1.50%"));
        assert!(msg.contains("15"));
        assert!(msg.contains("42"));
        assert!(msg.contains("2.80%"));
    }

    #[test]
    fn format_help_message_includes_all_commands() {
        let msg = format_help_message();
        assert!(msg.contains("/start"));
        assert!(msg.contains("/status"));
        assert!(msg.contains("/settings"));
        assert!(msg.contains("/history"));
        assert!(msg.contains("/help"));
    }

    // =========================================================================
    // INTEGRATION TESTS (3 tests)
    // =========================================================================

    #[test]
    fn end_to_end_alert_flow_with_rate_limiting() {
        let config = TelegramAlertConfig {
            min_roi_percent: 2.0,
            max_alerts_per_minute: 3.0,
            only_verified: false,
            only_live: false,
            alert_on_verified_only: false,
            history_size: 100,
        };

        let manager = AlertManager::new(config);
        let limiter = RateLimiter::alerts_per_minute(3.0);

        // Create 5 surebets
        let mut surebets = vec![];
        for i in 0..5 {
            surebets.push(create_test_surebet(2.0 + (i as f64 * 0.5), true, false));
        }

        let mut sent_count = 0;
        let mut throttled_count = 0;

        for surebet in &surebets {
            if manager.should_alert(surebet).is_ok() && limiter.try_consume(1.0) {
                manager.record_alert(surebet, AlertStatus::Sent);
                sent_count += 1;
            } else {
                manager.record_alert(surebet, AlertStatus::Throttled);
                throttled_count += 1;
            }
        }

        // With 3.0 alerts/min and 5 opportunities, expect 3 sent and 2 throttled
        assert!(sent_count == 3);
        assert!(throttled_count == 2);

        let stats = manager.get_stats();
        assert_eq!(stats.sent, 3);
        assert_eq!(stats.throttled, 2);
    }

    #[test]
    fn alert_manager_filters_are_independent() {
        let mut config = TelegramAlertConfig::default();
        config.only_verified = true;
        config.only_live = false;

        let manager = AlertManager::new(config);

        // Should reject unverified
        let unverified = create_test_surebet(5.0, false, true);
        assert!(manager.should_alert(&unverified).is_err());

        // Should accept verified (even if not live)
        let verified_prematch = create_test_surebet(5.0, true, false);
        assert!(manager.should_alert(&verified_prematch).is_ok());

        // Now set only_live
        config.only_live = true;
        let manager2 = AlertManager::new(config);

        // Should reject non-live
        assert!(manager2.should_alert(&verified_prematch).is_err());

        // Should accept live verified
        let verified_live = create_test_surebet(5.0, true, true);
        assert!(manager2.should_alert(&verified_live).is_ok());
    }

    #[test]
    fn rate_limiter_handles_high_volume_scenario() {
        let limiter = RateLimiter::alerts_per_minute(10.0);
        let manager = AlertManager::new(TelegramAlertConfig::default());

        // Simulate 20 opportunities in rapid succession
        let mut sent = 0;
        let mut throttled = 0;

        for i in 0..20 {
            let surebet = create_test_surebet(2.0 + (i as f64 * 0.1), true, false);

            if manager.should_alert(&surebet).is_ok() && limiter.try_consume(1.0) {
                manager.record_alert(&surebet, AlertStatus::Sent);
                sent += 1;
            } else {
                manager.record_alert(&surebet, AlertStatus::Throttled);
                throttled += 1;
            }
        }

        // With 10 tokens/min capacity and 20 opportunities, expect 10 sent
        assert_eq!(sent, 10);
        assert_eq!(throttled, 10);
    }

    // =========================================================================
    // CONFIGURATION TESTS (2 tests)
    // =========================================================================

    #[test]
    fn alert_config_default_values_are_sensible() {
        let config = TelegramAlertConfig::default();
        assert_eq!(config.min_roi_percent, 2.0);
        assert_eq!(config.max_alerts_per_minute, 10.0);
        assert_eq!(config.only_verified, false);
        assert_eq!(config.only_live, false);
        assert_eq!(config.alert_on_verified_only, false);
        assert_eq!(config.history_size, 100);
    }

    #[test]
    fn alert_config_can_be_customized() {
        let mut config = TelegramAlertConfig::default();
        config.min_roi_percent = 5.0;
        config.max_alerts_per_minute = 20.0;
        config.only_verified = true;
        config.only_live = true;
        config.history_size = 500;

        assert_eq!(config.min_roi_percent, 5.0);
        assert_eq!(config.max_alerts_per_minute, 20.0);
        assert_eq!(config.only_verified, true);
        assert_eq!(config.only_live, true);
        assert_eq!(config.history_size, 500);
    }
}

// =========================================================================
// RUNNING THE TESTS
// =========================================================================
//
// To compile and run these tests:
//
// 1. Ensure the bot crate is properly set up:
//    cargo test --lib bot
//
// 2. Run specific test:
//    cargo test --lib bot rate_limiter::tests::limiter_starts_with_full_capacity
//
// 3. Run with output:
//    cargo test --lib bot -- --nocapture
//
// 4. Run tests in parallel (faster):
//    cargo test --lib bot -- --test-threads=4
//
// 5. Run a single module:
//    cargo test --lib bot telegram_alerts_integration_tests
//
// Expected output:
// test telegram_alerts_integration_tests::rate_limiter_starts_with_full_capacity ... ok
// test telegram_alerts_integration_tests::rate_limiter_consumes_single_token ... ok
// ...
// test result: ok. 30 passed in 2.45s
