use anyhow::Result;
use regex::Regex;
use sha2::{Digest, Sha256};

pub struct Analyzer {
    error_regex: Regex,
    ignore_regex: Option<Regex>,
    // Regex for fingerprinting
    hex_regex: Regex,
    digit_regex: Regex,
    iso_date_regex: Regex,    // "2024-01-01T12:00:00" or similar
    simple_date_regex: Regex, // "2024/01/01"
}

impl Analyzer {
    pub fn new(error_pattern: &str, ignore_pattern: Option<String>) -> Result<Self> {
        let error_regex = Regex::new(error_pattern)?;
        let ignore_regex = match ignore_pattern {
            Some(p) => Some(Regex::new(&p)?),
            None => None,
        };

        Ok(Self {
            error_regex,
            ignore_regex,
            hex_regex: Regex::new(r"0x[a-fA-F0-9]+")?,
            digit_regex: Regex::new(r"\d+")?,
            iso_date_regex: Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?Z?")?,
            simple_date_regex: Regex::new(r"\d{4}/\d{2}/\d{2}-\d{2}:\d{2}:\d{2}(\.\d+)?")?,
        })
    }

    pub fn is_error(&self, line: &str) -> bool {
        if let Some(ignore) = &self.ignore_regex {
            if ignore.is_match(line) {
                return false;
            }
        }
        self.error_regex.is_match(line)
    }

    pub fn fingerprint(&self, line: &str) -> String {
        // 1. Remove dates
        let no_iso = self.iso_date_regex.replace_all(line, "");
        let no_date = self.simple_date_regex.replace_all(&no_iso, "");

        // 2. Mask hex
        let no_hex = self.hex_regex.replace_all(&no_date, "<HEX>");

        // 3. Mask digits (avoid masking the hex replacement)
        let normalized = self.digit_regex.replace_all(&no_hex, "<NUM>");

        // 4. Hash
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }
}
