mod analyzer;
mod chain_monitor;
mod config;
mod explorer_monitor;
mod notifier;
mod probe;
mod reader;
mod watcher;
mod whitelist;

use crate::{
    analyzer::Analyzer,
    config::Config,
    explorer_monitor::ExplorerMonitor,
    notifier::Notifier,
    probe::Probe,
    reader::Reader,
    watcher::Watcher,
    whitelist::{CheckResult, Whitelist},
};
use anyhow::{Context, Result};
use std::{env, time::Duration};
use tokio::time;

/// Spawn log monitoring as an independent task.
fn spawn_log_monitor(
    monitoring: config::MonitoringConfig,
    alerting: config::AlertingConfig,
    notifier: Notifier,
) -> Result<()> {
    let mut whitelist = if let Some(ref path) = monitoring.whitelist_path {
        println!("Loading whitelist from {path}");
        Whitelist::load(path).context("Failed to load whitelist")?
    } else {
        Whitelist::default()
    };

    let mut watcher = Watcher::new(monitoring.clone());
    let analyzer = Analyzer::new(&monitoring.error_pattern)?;

    let files = watcher.discover()?;
    println!("Found {} files to monitor", files.len());

    let check_interval_ms = monitoring.check_interval_ms;

    tokio::spawn(async move {
        let mut reader = Reader::new().expect("Failed to create reader");
        for file in files {
            println!("Monitoring: {file:?}");
            if let Err(e) = reader.add_file(&file).await {
                eprintln!("Failed to add file: {e:?}");
            }
        }

        // Periodic file discovery only if check_interval_ms is configured
        let mut discovery_interval =
            check_interval_ms.map(|ms| time::interval(Duration::from_millis(ms)));

        loop {
            tokio::select! {
                Some(line_event) = reader.next_line() => {
                    let line = line_event.line();
                    let path = line_event.source();

                    if !analyzer.is_error(line) {
                        continue;
                    }

                    let file_str = path.to_str().unwrap_or("unknown");

                    match whitelist.check(line, path) {
                        CheckResult::Skip => continue,
                        CheckResult::Alert { count, priority } => {
                            let msg = format!("{line} [Frequency Alert: >{count}/5min]");
                            println!("Frequency Alert in {path:?}: {msg}");
                            if let Err(e) = notifier.alert(&msg, file_str, priority).await {
                                eprintln!("Failed to send alert: {e:?}");
                            }
                        }
                        CheckResult::AlwaysAlert => {
                            println!("Alert in {path:?}: {line}");
                            if let Err(e) = notifier.alert(line, file_str, alerting.default_priority).await {
                                eprintln!("Failed to send alert: {e:?}");
                            }
                        }
                    }
                }
                _ = async { discovery_interval.as_mut().unwrap().tick().await }, if discovery_interval.is_some() => {
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
    });

    Ok(())
}

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

    let notifier = Notifier::new(config.alerting.clone());

    // Verify webhook connectivity on startup
    notifier.verify_webhooks().await.context("Webhook verification failed")?;

    // Start Probes
    for probe_config in config.probes {
        let probe = Probe::new(probe_config, notifier.clone());
        println!("Starting health probe for {}...", probe.url());
        tokio::spawn(async move {
            probe.run().await;
        });
    }

    // Start Chain Monitors (if configured)
    if let Some(chain_config) = config.chain_monitor {
        println!("Starting chain monitors...");
        chain_monitor::spawn_all(chain_config, notifier.clone())
            .await
            .context("Failed to start chain monitors")?;
    }

    // Start Explorer Monitor (if configured)
    if let Some(explorer_cfg) = config.explorer_monitor {
        let monitor = ExplorerMonitor::new(explorer_cfg, notifier.clone());
        println!("Starting explorer monitor for {}...", monitor.tag());
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    // Start Log Monitoring (if configured)
    if let Some(monitoring) = config.monitoring {
        println!("Starting log monitoring...");
        spawn_log_monitor(monitoring, config.alerting, notifier)?;
    }

    println!("Sentinel started...");

    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");

    Ok(())
}
