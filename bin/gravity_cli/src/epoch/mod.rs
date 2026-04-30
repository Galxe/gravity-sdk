use clap::Parser;

use crate::command::Executable;

pub mod next;
pub mod status;
pub mod watch;

// TODO: consensus-layer queries for the remaining `/consensus/*` HTTP endpoints
//       (QC at epoch/round, ledger_info by epoch, validator_count by epoch,
//       block by epoch/round) are intentionally not wired into `gravity-cli`
//       yet. The blocker is that `crates/api/src/consensus_api.rs` only mounts
//       those routes under `#[cfg(debug_assertions)]`, so they are missing in
//       release builds. Once that is lifted (or moved behind an opt-in flag),
//       add subcommands like `epoch qc <epoch> <round>` that call
//       `GET <server_url>/consensus/qc/:epoch/:round`. DKG coverage can
//       likewise be expanded beyond the existing `dkg status` / `dkg
//       randomness` commands.

#[derive(Debug, Parser)]
pub struct EpochCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Parser)]
pub enum SubCommands {
    /// Show detailed current epoch timing (running/remaining/overdue).
    Status(status::StatusCommand),
    /// Print a one-liner predicting when the next epoch transition happens.
    Next(next::NextCommand),
    /// Poll until an epoch transition is observed (logs each change).
    Watch(watch::WatchCommand),
}

impl Executable for EpochCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        match self.command {
            SubCommands::Status(c) => c.execute(),
            SubCommands::Next(c) => c.execute(),
            SubCommands::Watch(c) => c.execute(),
        }
    }
}
