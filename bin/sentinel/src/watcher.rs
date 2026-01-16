use crate::config::MonitoringConfig;
use anyhow::Result;
use glob::glob;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Watcher {
    config: MonitoringConfig,
    known_files: HashSet<PathBuf>,
}

impl Watcher {
    pub fn new(config: MonitoringConfig) -> Self {
        Self { config, known_files: HashSet::new() }
    }

    pub fn discover(&mut self) -> Result<Vec<PathBuf>> {
        let mut new_files = Vec::new();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        for pattern in &self.config.file_patterns {
            for entry in glob(pattern)? {
                match entry {
                    Ok(path) => {
                        if self.should_monitor(&path, now) {
                            if self.known_files.insert(path.clone()) {
                                new_files.push(path);
                            }
                        }
                    }
                    Err(e) => eprintln!("Glob error: {:?}", e),
                }
            }
        }
        Ok(new_files)
    }

    fn should_monitor(&self, path: &Path, now: u64) -> bool {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                    let diff = now.saturating_sub(since_epoch.as_secs());
                    return diff <= self.config.recent_file_threshold_seconds;
                }
            }
        }
        false
    }
}
