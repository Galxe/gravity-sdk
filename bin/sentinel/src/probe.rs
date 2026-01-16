use crate::config::ProbeConfig;
use crate::notifier::Notifier;
 // Not unused anymore? Wait, run() returns nothing. 
// Ah, run() is async fn run(self). 
// Let's check probe.rs content again. I used it in probe.rs but maybe just imported it and didn't use it in function signature?
// "warning: unused import: `anyhow::Result`"
// I will remove it.
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

    pub async fn run(self) {
        let mut failures = 0;
        let interval = Duration::from_secs(self.config.check_interval_seconds);
        let mut timer = time::interval(interval);

        // First tick completes immediately, skip it or wait? Usually standard is wait first.
        timer.tick().await; 

        loop {
            timer.tick().await;
            match self.client.get(&self.config.url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if failures > 0 {
                            // Recovered
                            println!("Probe recovered: {}", self.config.url);
                            failures = 0;
                            // Optionally alert on recovery? For now, silence is golden.
                        }
                    } else {
                        failures += 1;
                        println!("Probe failed (status {}): {} (count: {})", resp.status(), self.config.url, failures);
                    }
                }
                Err(e) => {
                    failures += 1;
                    println!("Probe failed (error): {} - {} (count: {})", self.config.url, e, failures);
                }
            }

            if failures >= self.config.failure_threshold {
                let msg = format!("Probe failed {} times for URL: {}", failures, self.config.url);
                println!("TRIGGERING ALERT: {msg}");
                if let Err(e) = self.notifier.alert(&msg, "PROBE").await {
                    eprintln!("Failed to send probe alert: {e:?}");
                }
                // Reset failures to avoid spamming every cycle? 
                // Or keep alerting? "timer post ... if several times ... then error"
                // Usually we alert once per threshold breach or distinct incident.
                // Let's reset to 0 to alert again if it persists for another N cycles.
                failures = 0;
            }
        }
    }
}
