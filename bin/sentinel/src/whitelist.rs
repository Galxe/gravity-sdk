use crate::config::Priority;
use anyhow::Result;
use csv::ReaderBuilder;
use regex::Regex;
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    path::{Path, PathBuf},
    time::Instant,
};

const WINDOW_SECONDS: u64 = 300; // 5 minutes

/// Result of checking a log line against whitelist rules.
#[derive(Debug)]
pub enum CheckResult {
    /// No rule matched, alert normally
    AlwaysAlert,
    /// Matched but below threshold, skip alerting
    Skip,
    /// Matched and above threshold, alert with frequency count and optional priority override
    Alert { count: u32, priority: Priority },
}

/// A whitelist rule with pattern, threshold, and optional priority override.
pub struct WhitelistRule {
    pattern: Regex,
    threshold: i32,
    /// Priority for alerts from this rule (default: P0).
    priority: Priority,
    /// Per-file-path timestamps for frequency counting within the sliding window.
    timestamps: HashMap<PathBuf, VecDeque<Instant>>,
}

impl WhitelistRule {
    pub fn new(pattern_str: &str, threshold: i32, priority: Priority) -> Result<Self> {
        // Try to compile as regex first, fallback to literal string if invalid
        let pattern = match Regex::new(pattern_str) {
            Ok(re) => re,
            Err(_) => {
                // Treat as literal string match (escape special chars)
                Regex::new(&regex::escape(pattern_str))?
            }
        };

        Ok(Self { pattern, threshold, priority, timestamps: HashMap::new() })
    }

    fn matches(&self, line: &str) -> bool {
        self.pattern.is_match(line)
    }
}

/// Whitelist checker that loads rules from CSV and checks log lines.
#[derive(Default)]
pub struct Whitelist {
    rules: Vec<WhitelistRule>,
}

impl Whitelist {
    /// Load whitelist rules from a CSV file.
    ///
    /// Format: Pattern,Threshold[,Priority]
    /// - Lines starting with # are comments
    /// - Threshold = -1: always ignore
    /// - Threshold > 0: alert if count > threshold in 5 minutes
    /// - Priority (optional): p0, p1, p2 — override default alert priority
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = ReaderBuilder::new()
            .has_headers(false)
            .comment(Some(b'#'))
            .flexible(true) // Allow flexible number of fields to handle optional priority
            .from_reader(file);

        let mut rules = Vec::new();

        for result in reader.records() {
            let record = result?;

            // Expected format: pattern, threshold [, priority]
            if record.len() < 2 {
                continue;
            }

            let pattern = record[0].trim();
            let threshold_str = record[1].trim();

            if pattern.is_empty() {
                continue;
            }

            let threshold: i32 = match threshold_str.parse() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Warning: Invalid threshold '{threshold_str}': {e}");
                    continue;
                }
            };

            // Parse optional priority (third column, default: P0)
            let priority: Priority = if record.len() >= 3 {
                let p_str = record[2].trim();
                match p_str.to_lowercase().as_str() {
                    "p1" => Priority::P1,
                    "p2" => Priority::P2,
                    _ => Priority::P0,
                }
            } else {
                Priority::P0
            };

            match WhitelistRule::new(pattern, threshold, priority) {
                Ok(rule) => {
                    println!("  Loaded rule: pattern='{pattern}', threshold={threshold}, priority={priority}");
                    rules.push(rule);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to compile pattern '{pattern}': {e}");
                }
            }
        }

        println!("Loaded {} whitelist rules", rules.len());
        Ok(Self { rules })
    }

    /// Check a log line against whitelist rules.
    /// Frequency thresholds are counted per source file path.
    pub fn check(&mut self, line: &str, source: &Path) -> CheckResult {
        for rule in &mut self.rules {
            if rule.matches(line) {
                // Always skip if threshold is -1
                if rule.threshold == -1 {
                    return CheckResult::Skip;
                }

                let now = Instant::now();
                let ts = match rule.timestamps.get_mut(source) {
                    Some(ts) => ts,
                    None => rule.timestamps.entry(source.to_path_buf()).or_default(),
                };

                // FIFO cleanup: remove expired timestamps
                while let Some(front) = ts.front() {
                    if now.duration_since(*front).as_secs() > WINDOW_SECONDS {
                        ts.pop_front();
                    } else {
                        break;
                    }
                }

                // Add current timestamp
                ts.push_back(now);

                let count = ts.len() as u32;

                // Check threshold
                if count > rule.threshold as u32 {
                    return CheckResult::Alert { count, priority: rule.priority };
                } else {
                    return CheckResult::Skip;
                }
            }
        }

        // No rule matched, always alert
        CheckResult::AlwaysAlert
    }
}
