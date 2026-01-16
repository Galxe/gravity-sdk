use crate::config::AlertingConfig;
use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub struct Notifier {
    client: Client,
    config: AlertingConfig,
}

impl Notifier {
    pub fn new(config: AlertingConfig) -> Self {
        Self { client: Client::new(), config }
    }

    pub async fn alert(&self, message: &str, file: &str) -> Result<()> {
        let text = format!(
            "🚨 **Log Sentinel Alert** 🚨\nFile: `{}`\nError:\n```\n{}\n```",
            file, message
        );

        if let Some(feishu) = &self.config.feishu_webhook {
            if !feishu.is_empty() {
                let payload = json!({
                    "msg_type": "text",
                    "content": {
                        "text": text
                    }
                });
                let _ = self.client.post(feishu).json(&payload).send().await;
            }
        }

        if let Some(slack) = &self.config.slack_webhook {
            if !slack.is_empty() {
                let payload = json!({
                    "text": text
                });
                let _ = self.client.post(slack).json(&payload).send().await;
            }
        }

        Ok(())
    }
}
