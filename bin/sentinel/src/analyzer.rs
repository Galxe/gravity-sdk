use anyhow::Result;
use regex::Regex;

pub struct Analyzer {
    error_regex: Regex,
}

impl Analyzer {
    pub fn new(error_pattern: &str) -> Result<Self> {
        let error_regex = Regex::new(error_pattern)?;
        Ok(Self { error_regex })
    }

    pub fn is_error(&self, line: &str) -> bool {
        self.error_regex.is_match(line)
    }
}
