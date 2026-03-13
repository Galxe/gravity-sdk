use crate::{
    config::{Priority, ProbeConfig},
    notifier::Notifier,
};
use reqwest::Client;
use std::time::Duration;
use tokio::time;

pub struct Probe {
    config: ProbeConfig,
    client: Client,
    notifier: Notifier,
}

impl Probe {
    pub fn new(config: ProbeConfig, notifier: Notifier) -> Self {
        Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            notifier,
        }
    }

    pub fn url(&self) -> &str {
        &self.config.url
    }

    pub async fn run(self) {
        let mut failures = 0;
        let interval = Duration::from_secs(self.config.check_interval_seconds);
        let mut timer = time::interval(interval);

        // First tick completes immediately
        timer.tick().await;

        loop {
            timer.tick().await;
            match self.client.get(&self.config.url).send().await {
                Ok(_) => {
                    // Any HTTP response (even non-200) means the service is reachable
                    if failures > 0 {
                        println!("Probe recovered: {}", self.config.url);
                        failures = 0;
                    }
                }
                Err(e) => {
                    failures += 1;
                    println!(
                        "Probe failed (error): {} - {} (count: {})",
                        self.config.url, e, failures
                    );
                }
            }

            if failures >= self.config.failure_threshold {
                let msg = format!("Probe failed {} times for URL: {}", failures, self.config.url);
                println!("TRIGGERING ALERT: {msg}");
                // Probe alerts are always P0
                if let Err(e) = self.notifier.alert(&msg, "PROBE", Priority::P0).await {
                    eprintln!("Failed to send probe alert: {e:?}");
                }
                // Reset failures to avoid spamming every cycle
                // Let's reset to 0 to alert again if it persists for another N cycles.
                failures = 0;
            }
        }
    }
}
