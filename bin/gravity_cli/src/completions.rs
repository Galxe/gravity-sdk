use crate::command::{Command as GravityCommand, Executable};
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};

#[derive(Debug, Parser)]
pub struct CompletionsCommand {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
    #[clap(value_enum)]
    pub shell: Shell,
}

impl Executable for CompletionsCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let mut cmd = GravityCommand::command();
        generate(self.shell, &mut cmd, "gravity-cli", &mut std::io::stdout());
        Ok(())
    }
}
