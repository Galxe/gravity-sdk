use anyhow::Result;
use regex::Regex;
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    time::Instant,
};

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
pub struct Whitelist {
    rules: Vec<WhitelistRule>,
}

impl Default for Whitelist {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
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
        let reader = BufReader::new(file);
        let mut rules = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Parse CSV line (simple parsing, handles quoted patterns)
            if let Some((pattern, threshold)) = parse_csv_line(trimmed) {
                match WhitelistRule::new(&pattern, threshold) {
                    Ok(rule) => {
                        println!("  Loaded rule: pattern='{}', threshold={}", pattern, threshold);
                        rules.push(rule);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to compile pattern '{}': {}", pattern, e);
                    }
                }
            } else {
                eprintln!("Warning: Invalid CSV line: {}", trimmed);
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

/// Parse a CSV line into (pattern, threshold).
/// Handles quoted patterns with escaped quotes ("").
fn parse_csv_line(line: &str) -> Option<(String, i32)> {
    let line = line.trim();

    if line.starts_with('"') {
        // Quoted pattern - find the closing quote
        // Handle escaped quotes ("") inside the pattern
        let mut pattern = String::new();
        let mut chars = line[1..].chars().peekable();
        let mut found_end = false;

        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote
                    pattern.push('"');
                    chars.next();
                } else {
                    // End of quoted string
                    found_end = true;
                    break;
                }
            } else {
                pattern.push(c);
            }
        }

        if !found_end {
            return None;
        }

        // Rest should be ,threshold
        let rest: String = chars.collect();
        let rest = rest.trim_start_matches(',').trim();
        let threshold: i32 = rest.parse().ok()?;

        Some((pattern, threshold))
    } else {
        // Unquoted - simple split by comma
        let parts: Vec<&str> = line.splitn(2, ',').collect();
        if parts.len() != 2 {
            return None;
        }

        let pattern = parts[0].trim().to_string();
        let threshold: i32 = parts[1].trim().parse().ok()?;

        Some((pattern, threshold))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line_unquoted() {
        let result = parse_csv_line("pattern,-1");
        assert_eq!(result, Some(("pattern".to_string(), -1)));
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let result = parse_csv_line("\"pattern with, comma\",-1");
        assert_eq!(result, Some(("pattern with, comma".to_string(), -1)));
    }

    #[test]
    fn test_parse_csv_line_escaped_quote() {
        let result = parse_csv_line("\"\"\"event\"\":\"\"Timeout\"\"\",-1");
        assert_eq!(result, Some(("\"event\":\"Timeout\"".to_string(), -1)));
    }
}
