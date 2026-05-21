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
    ///
    /// `Ok(None)` from the patched linemux fork signals that `watched_files`
    /// went empty — a state the upstream crate would have busy-spun on (see
    /// <https://github.com/jmagnuson/linemux/issues/57>). The periodic
    /// `add_file()` re-attach in `spawn_log_monitor` should keep this from
    /// ever happening; if it does, exit so systemd restarts us rather than
    /// risk silently losing monitoring.
    pub async fn next_line(&mut self) -> Option<Line> {
        match self.lines.next_line().await {
            Ok(Some(line)) => Some(line),
            Ok(None) => {
                eprintln!("FATAL: linemux exhausted (watched_files empty) — exiting for restart");
                std::process::exit(2);
            }
            Err(e) => {
                eprintln!("linemux error: {e:?}");
                None
            }
        }
    }
}
