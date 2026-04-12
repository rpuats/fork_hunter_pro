use rand::Rng;
use reqwest::Client;
use std::collections::HashMap;

pub struct StealthConfig {
    pub user_agents: Vec<String>,
    pub viewports: Vec<(u32, u32)>,
    pub languages: Vec<String>,
    pub platforms: Vec<String>,
    pub screen_resolutions: Vec<(u32, u32)>,
    pub color_depths: Vec<u32>,
    pub pixel_ratios: Vec<f64>,
    pub timezones: Vec<String>,
}

impl StealthConfig {
    pub fn default() -> Self {
        Self {
            user_agents: vec![
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0".into(),
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
            ],
            viewports: vec![
                (1920, 1080),
                (1366, 768),
                (1536, 864),
                (1440, 900),
                (2560, 1440),
            ],
            languages: vec![
                "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7".into(),
                "en-US,en;q=0.9".into(),
            ],
            platforms: vec!["Win32".into(), "MacIntel".into(), "Linux x86_64".into()],
            screen_resolutions: vec![
                (1920, 1080),
                (1366, 768),
                (2560, 1440),
            ],
            color_depths: vec![24],
            pixel_ratios: vec![1.0, 1.25, 1.5, 2.0],
            timezones: vec![
                "Europe/Moscow".into(),
                "UTC".into(),
            ],
        }
    }

    pub fn random_ua(&self) -> &str {
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..self.user_agents.len());
        &self.user_agents[idx]
    }

    pub fn random_viewport(&self) -> (u32, u32) {
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..self.viewports.len());
        self.viewports[idx]
    }

    pub fn random_language(&self) -> &str {
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..self.languages.len());
        &self.languages[idx]
    }

    pub fn apply_to_client(&self, _client: &Client) {
        // In Rust/reqwest, stealth headers are set per-request
        // This is a placeholder for the concept
    }

    pub fn get_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), self.random_ua().to_string());
        headers.insert(
            "Accept-Language".to_string(),
            self.random_language().to_string(),
        );
        headers.insert(
            "Accept".to_string(),
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
                .to_string(),
        );
        headers.insert(
            "Accept-Encoding".to_string(),
            "gzip, deflate, br".to_string(),
        );
        headers.insert("Sec-Fetch-Dest".to_string(), "document".to_string());
        headers.insert("Sec-Fetch-Mode".to_string(), "navigate".to_string());
        headers.insert("Sec-Fetch-Site".to_string(), "none".to_string());
        headers.insert("Sec-Fetch-User".to_string(), "?1".to_string());
        headers.insert(
            "Sec-Ch-Ua".to_string(),
            "\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\""
                .to_string(),
        );
        headers.insert("Sec-Ch-Ua-Mobile".to_string(), "?0".to_string());
        headers.insert("Sec-Ch-Ua-Platform".to_string(), "\"Windows\"".to_string());
        headers.insert("Upgrade-Insecure-Requests".to_string(), "1".to_string());
        headers.insert("Cache-Control".to_string(), "max-age=0".to_string());
        headers
    }
}
