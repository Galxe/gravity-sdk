use crate::{
    config::{ExplorerMonitorConfig, Priority},
    notifier::Notifier,
};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::{self, MissedTickBehavior};

/// Blockscout v2 `/api/v2/stats` response fragment. `total_blocks` is a
/// stringified integer in the Blockscout schema.
#[derive(Deserialize)]
struct StatsResp {
    total_blocks: String,
}

pub struct ExplorerMonitor {
    config: ExplorerMonitorConfig,
    client: Client,
    notifier: Notifier,
}

impl ExplorerMonitor {
    pub fn new(config: ExplorerMonitorConfig, notifier: Notifier) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { config, client, notifier }
    }

    pub fn tag(&self) -> String {
        self.config.tag.clone().unwrap_or_else(|| self.config.api_base.clone())
    }

    async fn fetch_height(&self) -> Result<u64> {
        let url = format!("{}/api/v2/stats", self.config.api_base.trim_end_matches('/'));
        let resp: StatsResp =
            self.client.get(&url).send().await?.error_for_status()?.json().await?;
        resp.total_blocks
            .parse::<u64>()
            .map_err(|e| anyhow!("invalid total_blocks {:?}: {e}", resp.total_blocks))
    }

    pub async fn run(self) {
        let tag = self.tag();
        let mut timer = time::interval(Duration::from_secs(self.config.poll_interval_seconds));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut last_height: Option<u64> = None;
        // Height for which a stall alert has already been sent; prevents
        // repeating the same alert every tick while the chain is stuck.
        let mut last_alerted_stall_height: Option<u64> = None;
        let mut api_failures: u32 = 0;
        let mut api_alert_sent = false;

        println!("Starting explorer monitor for {tag}");
        loop {
            timer.tick().await;
            match self.fetch_height().await {
                Ok(h) => {
                    if api_alert_sent {
                        println!("Explorer API recovered for {tag} (height={h})");
                        api_alert_sent = false;
                    }
                    api_failures = 0;

                    match last_height {
                        None => {
                            last_height = Some(h);
                        }
                        Some(prev) if h > prev => {
                            last_height = Some(h);
                            last_alerted_stall_height = None;
                        }
                        Some(prev) => {
                            // h <= prev: height did not advance within the poll window.
                            if last_alerted_stall_height != Some(prev) {
                                let msg = format!(
                                    "Explorer block height stalled on {tag}\n  \
                                     height: {prev} (no advance in {}s)\n  \
                                     api: {}",
                                    self.config.poll_interval_seconds, self.config.api_base,
                                );
                                println!("TRIGGERING ALERT: {msg}");
                                if let Err(e) = self
                                    .notifier
                                    .alert(&msg, "EXPLORER", self.config.priority)
                                    .await
                                {
                                    eprintln!("Failed to send explorer alert: {e:?}");
                                }
                                last_alerted_stall_height = Some(prev);
                            }
                            // Keep last_height unchanged so that when height finally
                            // advances we reset last_alerted_stall_height above.
                        }
                    }
                }
                Err(e) => {
                    api_failures += 1;
                    eprintln!(
                        "Explorer API error for {tag} ({api_failures}/{}): {e}",
                        self.config.api_failure_threshold
                    );
                    if !api_alert_sent && api_failures >= self.config.api_failure_threshold {
                        let msg = format!(
                            "Explorer API unreachable on {tag} ({} consecutive failures)\n  \
                             api: {}\n  last error: {e}",
                            api_failures, self.config.api_base,
                        );
                        println!("TRIGGERING ALERT: {msg}");
                        if let Err(err) = self.notifier.alert(&msg, "EXPLORER", Priority::P0).await
                        {
                            eprintln!("Failed to send explorer API alert: {err:?}");
                        }
                        api_alert_sent = true;
                    }
                }
            }
        }
    }
}
