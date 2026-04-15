use crate::config::{AlertingConfig, Priority};
use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Notifier {
    client: Client,
    config: AlertingConfig,
    /// Per-priority rate limiting.
    last_alert_times: std::sync::Arc<Mutex<HashMap<Priority, Instant>>>,
}

impl Notifier {
    pub fn new(config: AlertingConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            last_alert_times: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send a message to the webhooks for the given priority.
    async fn send(&self, text: &str, priority: Priority) -> Result<()> {
        let (feishu, slack) = self.config.get_webhooks(priority);

        if let Some(feishu_url) = feishu {
            if !feishu_url.is_empty() {
                let payload = json!({
                    "msg_type": "text",
                    "content": { "text": text }
                });
                let resp = self.client.post(feishu_url).json(&payload).send().await?;
                anyhow::ensure!(
                    resp.status().is_success(),
                    "Feishu webhook failed with status: {}",
                    resp.status()
                );
            }
        }

        if let Some(slack_url) = slack {
            if !slack_url.is_empty() {
                let payload = json!({
                    "text": text,
                    "channel": "#alerts-devops",
                    "username": "System-Monitor"
                });
                let resp = self.client.post(slack_url).json(&payload).send().await?;
                anyhow::ensure!(
                    resp.status().is_success(),
                    "Slack webhook failed with status: {}",
                    resp.status()
                );
            }
        }

        Ok(())
    }

    /// Send a startup message to verify all configured webhooks are reachable.
    pub async fn verify_webhooks(&self) -> Result<()> {
        let all = self.config.all_webhooks();
        if all.is_empty() {
            anyhow::bail!("No webhooks configured");
        }

        for (kind, url) in &all {
            let payload = match *kind {
                "feishu" => json!({
                    "msg_type": "text",
                    "content": { "text": "✅ Sentinel started and webhook is connected." }
                }),
                _ => json!({
                    "text": "✅ Sentinel started and webhook is connected.",
                    "channel": "#alerts-devops",
                    "username": "System-Monitor"
                }),
            };
            let resp = self.client.post(*url).json(&payload).send().await?;
            anyhow::ensure!(
                resp.status().is_success(),
                "{kind} webhook verification failed with status: {}",
                resp.status()
            );
        }

        Ok(())
    }

    pub async fn alert(&self, message: &str, file: &str, priority: Priority) -> Result<()> {
        // Per-priority rate limiting
        {
            let mut times = self.last_alert_times.lock().unwrap();
            let now = Instant::now();

            if let Some(last) = times.get(&priority) {
                if now.duration_since(*last) < Duration::from_secs(self.config.min_alert_interval) {
                    return Ok(());
                }
            }
            times.insert(priority, now);
        }

        let text = format!(
            "🚨 **Log Sentinel Alert** [{priority}] 🚨\nFile: `{file}`\nError:\n```\n{message}\n```"
        );

        // Fire-and-forget: log but don't propagate send errors
        if let Err(e) = self.send(&text, priority).await {
            eprintln!("Failed to send webhook: {e:?}");
        }

        Ok(())
    }
}
