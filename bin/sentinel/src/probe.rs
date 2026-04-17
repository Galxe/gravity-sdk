use crate::{
    config::{Priority, ProbeConfig},
    notifier::Notifier,
};
use reqwest::Client;
use std::{error::Error as _, time::Duration};
use tokio::time;

fn classify(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_redirect() {
        "redirect"
    } else if e.is_body() {
        "body"
    } else if e.is_decode() {
        "decode"
    } else if e.is_request() {
        "request"
    } else if e.is_builder() {
        "builder"
    } else {
        "other"
    }
}

fn format_error(e: &reqwest::Error) -> String {
    let mut causes = vec![e.to_string()];
    let mut src = e.source();
    while let Some(s) = src {
        causes.push(s.to_string());
        src = s.source();
    }
    // Dedupe consecutive identical messages (reqwest often wraps the same text)
    causes.dedup();
    format!("[{}] {}", classify(e), causes.join(" -> "))
}

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
        let mut failures: u32 = 0;
        let mut recent_errors: Vec<String> = Vec::new();
        let interval = Duration::from_secs(self.config.check_interval_seconds);
        let mut timer = time::interval(interval);

        // First tick completes immediately
        timer.tick().await;

        loop {
            timer.tick().await;
            let started = std::time::Instant::now();
            match self.client.get(&self.config.url).send().await {
                Ok(_) => {
                    // Any HTTP response (even non-200) means the service is reachable
                    if failures > 0 {
                        println!(
                            "Probe recovered: {} (after {} failures)",
                            self.config.url, failures
                        );
                        failures = 0;
                        recent_errors.clear();
                    }
                }
                Err(e) => {
                    failures += 1;
                    let elapsed_ms = started.elapsed().as_millis();
                    let detail = format_error(&e);
                    println!(
                        "Probe failed: {} after {}ms - {} (count: {})",
                        self.config.url, elapsed_ms, detail, failures
                    );
                    recent_errors.push(format!("#{failures} ({elapsed_ms}ms) {detail}"));
                }
            }

            if failures >= self.config.failure_threshold {
                let context = self.config.tag.as_deref().unwrap_or("No context provided");
                let errors_block = if recent_errors.is_empty() {
                    String::from("(none captured)")
                } else {
                    recent_errors.join("\n  ")
                };
                let msg = format!(
                    "Probe failed {} times for URL: {} (Context: {})\nRecent errors:\n  {}",
                    failures, self.config.url, context, errors_block
                );
                println!("TRIGGERING ALERT: {msg}");
                // Probe alerts are always P0
                if let Err(e) = self.notifier.alert(&msg, "PROBE", Priority::P0).await {
                    eprintln!("Failed to send probe alert: {e:?}");
                }
                // Reset failures to avoid spamming every cycle
                // Let's reset to 0 to alert again if it persists for another N cycles.
                failures = 0;
                recent_errors.clear();
            }
        }
    }
}
