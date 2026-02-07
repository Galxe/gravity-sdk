use anyhow::Result;
use csv::ReaderBuilder;
use regex::Regex;
use std::{collections::VecDeque, fs::File, path::Path, time::Instant};

const WINDOW_SECONDS: u64 = 300; // 5 minutes

/// Result of checking a log line against whitelist rules.
#[derive(Debug)]
pub enum CheckResult {
    /// No rule matched, alert normally
    Normal,
    /// Matched but below threshold, skip alerting
    Skip,
    /// Matched and above threshold, alert with frequency count
    Alert { count: u32 },
}

/// A whitelist rule with pattern and threshold.
pub struct WhitelistRule {
    pattern: Regex,
    threshold: i32,
    timestamps: VecDeque<Instant>,
}

impl WhitelistRule {
    pub fn new(pattern_str: &str, threshold: i32) -> Result<Self> {
        // Try to compile as regex first, fallback to literal string if invalid
        let pattern = match Regex::new(pattern_str) {
            Ok(re) => re,
            Err(_) => {
                // Treat as literal string match (escape special chars)
                Regex::new(&regex::escape(pattern_str))?
            }
        };

        Ok(Self { pattern, threshold, timestamps: VecDeque::new() })
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
    /// Format: Pattern,Threshold
    /// - Lines starting with # are comments
    /// - Threshold = -1: always ignore
    /// - Threshold > 0: alert if count > threshold in 5 minutes
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = ReaderBuilder::new()
            .has_headers(false)
            .comment(Some(b'#'))
            .flexible(true) // Allow flexible number of fields to handle potential issues gracefully
            .from_reader(file);

        let mut rules = Vec::new();

        for result in reader.records() {
            let record = result?;

            // Expected format: pattern, threshold
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

            match WhitelistRule::new(pattern, threshold) {
                Ok(rule) => {
                    println!("  Loaded rule: pattern='{pattern}', threshold={threshold}");
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
    pub fn check(&mut self, line: &str) -> CheckResult {
        for rule in &mut self.rules {
            if rule.matches(line) {
                // Always skip if threshold is -1
                if rule.threshold == -1 {
                    return CheckResult::Skip;
                }

                let now = Instant::now();

                // FIFO cleanup: remove expired timestamps
                while let Some(front) = rule.timestamps.front() {
                    if now.duration_since(*front).as_secs() > WINDOW_SECONDS {
                        rule.timestamps.pop_front();
                    } else {
                        break;
                    }
                }

                // Add current timestamp
                rule.timestamps.push_back(now);

                let count = rule.timestamps.len() as u32;

                // Check threshold
                if count > rule.threshold as u32 {
                    return CheckResult::Alert { count };
                } else {
                    return CheckResult::Skip;
                }
            }
        }

        // No rule matched, alert normally
        CheckResult::Normal
    }
}
