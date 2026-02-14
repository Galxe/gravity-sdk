mod analyzer;
mod config;
mod notifier;
mod probe;
mod reader;
mod watcher;
mod whitelist;

use crate::{
    analyzer::Analyzer,
    config::Config,
    notifier::Notifier,
    probe::Probe,
    reader::Reader,
    watcher::Watcher,
    whitelist::{CheckResult, Whitelist},
};
use anyhow::{Context, Result};
use std::{env, time::Duration};
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <config.toml>", args[0]);
        std::process::exit(1);
    }

    let config_path = &args[1];

    println!("Loading config from {config_path}");
    let config = Config::load(config_path).context("Failed to load config")?;

    // Load whitelist if configured
    let mut whitelist = if let Some(ref path) = config.monitoring.whitelist_path {
        println!("Loading whitelist from {path}");
        Whitelist::load(path).context("Failed to load whitelist")?
    } else {
        Whitelist::default()
    };

    let mut watcher = Watcher::new(config.monitoring.clone());
    let mut reader = Reader::new()?;
    let analyzer = Analyzer::new(&config.monitoring.error_pattern)?;
    let notifier = Notifier::new(config.alerting.clone());

    // Verify webhook connectivity on startup
    notifier.verify_webhooks().await.context("Webhook verification failed")?;

    // Start Probe if configured
    if let Some(probe_config) = config.probe {
        let probe = Probe::new(probe_config, notifier.clone());
        println!("Starting health probe...");
        tokio::spawn(async move {
            probe.run().await;
        });
    }

    // Initial file discovery
    let files = watcher.discover()?;
    println!("Found {} files to monitor", files.len());
    for file in files {
        println!("Monitoring: {file:?}");
        reader.add_file(&file).await?;
    }

    let check_interval = Duration::from_millis(config.general.check_interval_ms);
    let mut interval = time::interval(check_interval);

    println!("Sentinel started...");

    loop {
        tokio::select! {
            // Event-driven: new log line from linemux
            Some(line_event) = reader.next_line() => {
                let line = line_event.line();
                let path = line_event.source();

                if !analyzer.is_error(line) {
                    continue;
                }

                let file_str = path.to_str().unwrap_or("unknown");

                match whitelist.check(line) {
                    CheckResult::Skip => {
                        // Matched whitelist rule but below threshold, skip
                        continue;
                    }
                    CheckResult::Alert { count } => {
                        // Matched whitelist rule and above threshold
                        let msg = format!("{line} [Frequency Alert: >{count}/5min]");
                        println!("Frequency Alert in {path:?}: {msg}");
                        if let Err(e) = notifier.alert(&msg, file_str).await {
                            eprintln!("Failed to send alert: {e:?}");
                        }
                    }
                    CheckResult::Normal => {
                        // No whitelist rule matched, alert directly
                        println!("Alert in {path:?}: {line}");
                        if let Err(e) = notifier.alert(line, file_str).await {
                            eprintln!("Failed to send alert: {e:?}");
                        }
                    }
                }
            }
            // Periodic: discover new files via glob
            _ = interval.tick() => {
                match watcher.discover() {
                    Ok(new_files) => {
                        for file in new_files {
                            println!("New file discovered: {file:?}");
                            if let Err(e) = reader.add_file(&file).await {
                                eprintln!("Failed to add file: {e:?}");
                            }
                        }
                    }
                    Err(e) => eprintln!("Discovery error: {e:?}"),
                }
            }
        }
    }
}
