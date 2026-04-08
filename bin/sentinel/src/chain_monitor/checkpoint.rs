use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Persistent state for chain monitors, saved as JSON.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Checkpoint {
    /// Last scanned Ethereum block number
    pub ethereum_block: u64,
    /// Last scanned Gravity block number
    pub gravity_block: u64,
    /// Pending bridge nonces: nonce -> ethereum block timestamp (unix seconds)
    pub pending_nonces: HashMap<u128, u64>,
    /// Last known vault balance (decimal string for U256 precision)
    pub last_vault_balance: Option<String>,
}

pub type SharedCheckpoint = Arc<Mutex<Checkpoint>>;

impl Checkpoint {
    /// Load checkpoint from file, or return default if file doesn't exist or is malformed.
    pub fn load_or_default(path: Option<&str>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        if !Path::new(path).exists() {
            return Ok(Self::default());
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(ckpt) => {
                    println!("Loaded checkpoint from {path}");
                    Ok(ckpt)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse checkpoint file {path}: {e}, starting fresh");
                    Ok(Self::default())
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read checkpoint file {path}: {e}, starting fresh");
                Ok(Self::default())
            }
        }
    }

    /// Save checkpoint to file using atomic write (temp file + rename).
    pub fn save(&self, path: &str) -> Result<()> {
        let tmp_path = format!("{path}.tmp");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

/// Background task that periodically flushes the checkpoint to disk.
pub async fn flush_loop(checkpoint: SharedCheckpoint, path: String) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        let ckpt = checkpoint.lock().unwrap().clone();
        if let Err(e) = ckpt.save(&path) {
            eprintln!("Failed to save checkpoint: {e:?}");
        }
    }
}
