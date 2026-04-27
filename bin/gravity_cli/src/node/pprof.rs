use clap::{Parser, Subcommand};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::command::Executable;

#[derive(Debug, Parser)]
pub struct PprofCommand {
    #[command(subcommand)]
    pub command: PprofSubCommands,
}

#[derive(Debug, Subcommand)]
pub enum PprofSubCommands {
    /// Download an on-demand CPU profile from the node's pprof HTTP server.
    /// Requires the node was started with `--pprof_addr <addr>`.
    Cpu(CpuCommand),
}

#[derive(Debug, Parser)]
pub struct CpuCommand {
    /// pprof HTTP server address (e.g. 127.0.0.1:6060). Must match the node's
    /// `--pprof_addr` flag.
    #[clap(long, default_value = "127.0.0.1:6060", env = "GRAVITY_PPROF_ADDR")]
    pub addr: String,

    /// Profile duration in seconds (1..=300)
    #[clap(long, default_value = "30")]
    pub duration: u64,

    /// Sampling frequency in Hz (default 99)
    #[clap(long, default_value = "99")]
    pub frequency: u32,

    /// Output file path. Use `-` to write protobuf bytes to stdout.
    #[clap(long = "output-file", default_value = "cpu.pb")]
    pub output_file: String,
}

impl Executable for CpuCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl CpuCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let url = format!(
            "http://{}/debug/pprof/profile?seconds={}&frequency={}",
            self.addr.trim_start_matches("http://").trim_start_matches("https://"),
            self.duration,
            self.frequency,
        );
        // Server will hold the connection open for `duration` seconds — give
        // it headroom plus a cushion for TCP / HTTP framing.
        let timeout = Duration::from_secs(self.duration + 15);
        let client = reqwest::Client::builder().timeout(timeout).build()?;

        eprintln!("fetching {} (~{}s)…", url, self.duration);
        let start = Instant::now();
        let resp = client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_else(|_| "<unreadable>".into());
            return Err(anyhow::anyhow!("pprof server returned HTTP {status}: {body}"));
        }
        let bytes = resp.bytes().await?;
        let elapsed = start.elapsed();

        if self.output_file == "-" {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
        } else {
            let path = PathBuf::from(&self.output_file);
            fs::write(&path, &bytes)
                .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))?;
            eprintln!(
                "wrote {} bytes to {} in {:.1}s",
                bytes.len(),
                path.display(),
                elapsed.as_secs_f64()
            );
            eprintln!("inspect with: go tool pprof -http=:8080 {}", path.display());
        }
        Ok(())
    }
}
