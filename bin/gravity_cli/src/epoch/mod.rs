use clap::Parser;

use crate::command::Executable;

pub mod status;

#[derive(Debug, Parser)]
pub struct EpochCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Parser)]
pub enum SubCommands {
    Status(status::StatusCommand),
}

impl Executable for EpochCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        match self.command {
            SubCommands::Status(status_cmd) => status_cmd.execute(),
        }
    }
}
