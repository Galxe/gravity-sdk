use alloy_provider::ProviderBuilder;
use clap::Parser;
use colored::Colorize;
use std::time::Duration;

use crate::{command::Executable, epoch::next::fetch_epoch_timing};

#[derive(Debug, Parser)]
pub struct WatchCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Polling interval in seconds (default 5)
    #[clap(long, default_value = "5")]
    pub interval_secs: u64,

    /// Emit a status line every poll even when the epoch hasn't changed.
    /// Default: only print transitions.
    #[clap(long)]
    pub verbose: bool,
}

impl Executable for WatchCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl WatchCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self
            .rpc_url
            .ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let interval = Duration::from_secs(self.interval_secs.max(1));
        let mut last_seen: Option<u64> = None;

        println!(
            "{} watching epoch transitions every {}s (Ctrl+C to stop)",
            "[epoch watch]".cyan(),
            interval.as_secs()
        );

        loop {
            match fetch_epoch_timing(&provider).await {
                Ok((current_epoch, last_time_micros, interval_micros, block_ts)) => {
                    let predicted = (last_time_micros + interval_micros) / 1_000_000;
                    let delta: i64 = predicted as i64 - block_ts as i64;

                    match last_seen {
                        None => {
                            println!(
                                "{} initial: epoch {current_epoch}, next in {}",
                                "[epoch watch]".cyan(),
                                signed_hms(delta)
                            );
                            last_seen = Some(current_epoch);
                        }
                        Some(prev) if prev != current_epoch => {
                            println!(
                                "{} {} {prev} → {current_epoch}, next in {}",
                                "[epoch watch]".cyan(),
                                "transition:".green().bold(),
                                signed_hms(delta),
                            );
                            last_seen = Some(current_epoch);
                        }
                        _ => {
                            if self.verbose {
                                println!(
                                    "{} epoch {current_epoch}, next in {}",
                                    "[epoch watch]".dimmed(),
                                    signed_hms(delta)
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{} rpc error (will retry): {e}",
                        "[epoch watch]".yellow()
                    );
                }
            }
            tokio::time::sleep(interval).await;
        }
    }
}

fn signed_hms(delta: i64) -> String {
    if delta >= 0 {
        super::next::format_hms(delta as u64)
    } else {
        format!("overdue {}", super::next::format_hms((-delta) as u64))
    }
}
