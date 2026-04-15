use crate::{
    config::{Priority, ProbeConfig, ProbeMode},
    notifier::Notifier,
};
use reqwest::Client;
use std::time::Duration;
use tokio::{net::TcpStream, time};

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

            let ok = match self.config.mode {
                ProbeMode::Http => self.check_http().await,
                ProbeMode::Tcp => self.check_tcp().await,
            };

            if ok {
                if failures > 0 {
                    println!("Probe recovered: {}", self.config.url);
                    failures = 0;
                }
            } else {
                failures += 1;
            }

            if failures >= self.config.failure_threshold {
                let context = self.config.tag.as_deref().unwrap_or("No context provided");
                let mode_label = match self.config.mode {
                    ProbeMode::Http => "HTTP",
                    ProbeMode::Tcp => "TCP",
                };
                let msg = format!(
                    "{mode_label} probe failed {} times for: {} (Context: {})",
                    failures, self.config.url, context
                );
                println!("TRIGGERING ALERT: {msg}");
                // Probe alerts are always P0
                if let Err(e) = self.notifier.alert(&msg, "PROBE", Priority::P0).await {
                    eprintln!("Failed to send probe alert: {e:?}");
                }
                failures = 0;
            }
        }
    }

    async fn check_http(&self) -> bool {
        match self.client.get(&self.config.url).send().await {
            Ok(_) => true,
            Err(e) => {
                println!(
                    "Probe failed (HTTP): {} - {} ",
                    self.config.url, e
                );
                false
            }
        }
    }

    async fn check_tcp(&self) -> bool {
        // url format for TCP: "ip:port" or "host:port"
        let addr = self.config.url.trim_start_matches("tcp://");
        match time::timeout(Duration::from_secs(5), TcpStream::connect(addr)).await {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => {
                println!(
                    "Probe failed (TCP connect): {} - {}",
                    self.config.url, e
                );
                false
            }
            Err(_) => {
                println!(
                    "Probe failed (TCP timeout): {}",
                    self.config.url
                );
                false
            }
        }
    }
}
