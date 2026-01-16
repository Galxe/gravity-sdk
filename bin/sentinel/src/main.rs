mod analyzer;
mod config;
mod notifier;
mod reader;
mod state;
mod watcher;

use crate::analyzer::Analyzer;
use crate::config::Config;
use crate::notifier::Notifier;
use crate::reader::Reader;
use crate::state::State;
use crate::watcher::Watcher;
use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

const STATE_FILE: &str = "sentinel_state.json";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <config.toml> [state_file]", args[0]);
        std::process::exit(1);
    }
    
    let config_path = &args[1];
    let state_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from(STATE_FILE)
    };

    println!("Loading config from {}", config_path);
    let config = Config::load(config_path).context("Failed to load config")?;
    
    println!("Loading state from {:?}", state_path);
    let mut state = State::load(&state_path).context("Failed to load state")?;

    let mut watcher = Watcher::new(config.monitoring.clone());
    let mut reader = Reader::new()?;
    let analyzer = Analyzer::new(&config.monitoring.error_pattern, config.monitoring.ignore_pattern.clone())?;
    let notifier = Notifier::new(config.alerting.clone());

    // Initial discovery
    let files = watcher.discover()?;
    println!("Found {} files to monitor", files.len());
    for file in files {
        println!("Monitoring: {:?}", file);
        reader.add_file(file).await?;
    }

    let check_interval = Duration::from_millis(config.general.check_interval_ms);
    let mut interval = time::interval(check_interval);

    println!("Sentinel started...");

    loop {
        interval.tick().await;

        // Periodic discovery
        match watcher.discover() {
            Ok(new_files) => {
                for file in new_files {
                    println!("New file discovered: {:?}", file);
                    if let Err(e) = reader.add_file(&file).await {
                        eprintln!("Failed to watch file: {:?}", e);
                    }
                }
            }
            Err(e) => eprintln!("Discovery failed: {:?}", e),
        }
        
        // Periodic save state
        if let Err(e) = state.save(&state_path) {
            eprintln!("Failed to save state: {:?}", e);
        }

        // Poll files
        match reader.poll() {
            Ok(lines) => {
                for (path, line) in lines {
                    if analyzer.is_error(&line) {
                        let fingerprint = analyzer.fingerprint(&line);
                        if state.is_new(fingerprint) {
                            println!("New Error in {:?}: {}", path, line);
                            if let Err(e) = notifier.alert(&line, path.to_str().unwrap_or("unknown")).await {
                                eprintln!("Failed to send alert: {:?}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("Reader error: {:?}", e),
        }
    }
}
