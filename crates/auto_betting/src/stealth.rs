use rand::Rng;
use std::collections::HashMap;

pub struct StealthBetting {
    user_agents: Vec<String>,
    random_delays: bool,
    min_delay_ms: u64,
    max_delay_ms: u64,
    #[allow(dead_code)]
    session_headers: HashMap<String, String>,
}

impl StealthBetting {
    pub fn new() -> Self {
        Self {
            user_agents: vec![
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
            ],
            random_delays: true,
            min_delay_ms: 2000,
            max_delay_ms: 8000,
            session_headers: HashMap::new(),
        }
    }

    pub fn get_random_delay_ms(&self) -> u64 {
        if self.random_delays {
            let mut rng = rand::thread_rng();
            rng.gen_range(self.min_delay_ms..=self.max_delay_ms)
        } else {
            self.min_delay_ms
        }
    }

    pub fn get_random_user_agent(&self) -> &str {
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..self.user_agents.len());
        &self.user_agents[idx]
    }

    pub fn get_stealth_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(
            "User-Agent".into(),
            self.get_random_user_agent().to_string(),
        );
        headers.insert("Accept".into(), "application/json, text/plain, */*".into());
        headers.insert(
            "Accept-Language".into(),
            "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7".into(),
        );
        headers.insert("Accept-Encoding".into(), "gzip, deflate, br".into());
        headers.insert("Origin".into(), "https://example.com".into());
        headers.insert("Referer".into(), "https://example.com/sports".into());
        headers
    }

    pub async fn wait_stealth(&self) {
        let delay = self.get_random_delay_ms();
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
}

impl Default for StealthBetting {
    fn default() -> Self {
        Self::new()
    }
}
