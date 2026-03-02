use crate::config::AlertingConfig;
use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Notifier {
    client: Client,
    config: AlertingConfig,
    last_alert_time: std::sync::Arc<Mutex<Option<Instant>>>,
}

impl Notifier {
    pub fn new(config: AlertingConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            last_alert_time: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    /// Send a message to all configured webhooks.
    async fn send(&self, text: &str) -> Result<()> {
        if let Some(feishu) = &self.config.feishu_webhook {
            if !feishu.is_empty() {
                let payload = json!({
                    "msg_type": "text",
                    "content": { "text": text }
                });
                let resp = self.client.post(feishu).json(&payload).send().await?;
                anyhow::ensure!(
                    resp.status().is_success(),
                    "Feishu webhook failed with status: {}",
                    resp.status()
                );
            }
        }

        if let Some(slack) = &self.config.slack_webhook {
            if !slack.is_empty() {
                let payload = json!({
                    "text": text,
                    "channel": "#alerts-devops",
                    "username": "System-Monitor"
                });
                let resp = self.client.post(slack).json(&payload).send().await?;
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
        self.send("✅ Log Sentinel started and webhook is connected.").await
    }

    pub async fn alert(&self, message: &str, file: &str) -> Result<()> {
        // Rate limiting
        {
            let mut last_time = self.last_alert_time.lock().unwrap();
            let now = Instant::now();

            if let Some(last) = *last_time {
                if now.duration_since(last) < Duration::from_secs(self.config.min_alert_interval) {
                    return Ok(());
                }
            }
            *last_time = Some(now);
        }

        let text =
            format!("🚨 **Log Sentinel Alert** 🚨\nFile: `{file}`\nError:\n```\n{message}\n```");

        // Fire-and-forget: log but don't propagate send errors
        if let Err(e) = self.send(&text).await {
            eprintln!("Failed to send webhook: {e:?}");
        }

        Ok(())
    }
}
