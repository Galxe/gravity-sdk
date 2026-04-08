use clap::Parser;
use std::{collections::HashMap, fs, io::Write};

use crate::{
    command::Executable,
    config::{GravityConfig, ProfileConfig},
};

#[derive(Debug, Parser)]
pub struct InitCommand {
    /// Skip interactive prompts and use defaults
    #[clap(long)]
    pub non_interactive: bool,
}

impl Executable for InitCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let config_path = GravityConfig::config_path();

        if config_path.exists() && !self.non_interactive {
            eprintln!("Config file already exists at: {}", config_path.display());
            eprint!("Overwrite? [y/N]: ");
            std::io::stderr().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        let profile_name;
        let rpc_url;
        let server_url;
        let deploy_path;

        if self.non_interactive {
            profile_name = "default".to_string();
            rpc_url = "http://127.0.0.1:8545".to_string();
            server_url = "127.0.0.1:1024".to_string();
            deploy_path = String::new();
        } else {
            profile_name = prompt("Profile name", "default")?;
            rpc_url = prompt("RPC URL", "http://127.0.0.1:8545")?;
            server_url = prompt("DKG server URL", "127.0.0.1:1024")?;
            deploy_path = prompt("Deploy path (leave empty to skip)", "")?;
        }

        let mut profile = ProfileConfig {
            rpc_url: Some(rpc_url),
            server_url: Some(server_url),
            deploy_path: None,
            gas_limit: None,
            gas_price: None,
        };

        if !deploy_path.is_empty() {
            profile.deploy_path = Some(deploy_path);
        }

        let mut profiles = HashMap::new();
        profiles.insert(profile_name.clone(), profile);

        let config = GravityConfig {
            active_profile: profile_name,
            profiles,
        };

        // Ensure directory exists
        let config_dir = GravityConfig::config_dir();
        fs::create_dir_all(&config_dir)?;

        let toml_str = toml::to_string_pretty(&config)?;
        fs::write(&config_path, &toml_str)?;

        println!("\nConfig written to: {}", config_path.display());
        println!("\nYou can now run commands without specifying --rpc-url each time:");
        println!("  gravity-cli validator list");
        println!("  gravity-cli epoch status");
        println!("  gravity-cli status");
        println!("\nTo switch profiles, use --profile <name> or set GRAVITY_PROFILE env var.");

        Ok(())
    }
}

fn prompt(question: &str, default: &str) -> Result<String, anyhow::Error> {
    if default.is_empty() {
        eprint!("{question}: ");
    } else {
        eprint!("{question} [{default}]: ");
    }
    std::io::stderr().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    Ok(if input.is_empty() { default.to_string() } else { input.to_string() })
}
