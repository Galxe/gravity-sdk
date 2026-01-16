use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::Path,
};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub seen_hashes: HashSet<String>,
}

impl State {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(State::default());
        }
        let content = fs::read_to_string(path)?;
        let state: State = serde_json::from_str(&content)?;
        Ok(state)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    pub fn is_new(&mut self, hash: String) -> bool {
        self.seen_hashes.insert(hash)
    }
}
