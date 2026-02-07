use anyhow::Result;
use linemux::{Line, MuxedLines};
use std::path::PathBuf;

pub struct Reader {
    lines: MuxedLines,
}

impl Reader {
    pub fn new() -> Result<Self> {
        Ok(Self { lines: MuxedLines::new()? })
    }

    pub async fn add_file(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        self.lines.add_file(path).await.map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    /// Returns next line with source path. Blocks until available.
    pub async fn next_line(&mut self) -> Option<Line> {
        self.lines.next_line().await.ok().flatten()
    }
}
