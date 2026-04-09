/// Suggest a fix based on the error message content.
pub fn suggest_fix(err: &anyhow::Error) -> Option<String> {
    let msg = format!("{err:#}");
    let msg_lower = msg.to_lowercase();

    if msg_lower.contains("connection refused") || msg_lower.contains("error sending request") {
        return Some(
            "Check that the node is running and the URL is correct. \
             Use `gravity-cli node start` to start a node."
                .to_string(),
        );
    }

    if msg_lower.contains("is not a valid stakepool") || msg_lower.contains("not a valid pool") {
        return Some(
            "Verify the --stake-pool address. \
             Use `gravity-cli stake get --owner <addr>` to find your pools."
                .to_string(),
        );
    }

    if msg_lower.contains("not registered as a validator") {
        return Some("Register the validator first with `gravity-cli validator join`.".to_string());
    }

    if msg_lower.contains("failed to read private key") {
        return Some("Ensure you enter a valid hex-encoded private key.".to_string());
    }

    if msg_lower.contains("start script not found") || msg_lower.contains("stop script not found") {
        return Some(
            "Verify --deploy-path points to a valid deployment directory \
             created by deploy.sh."
                .to_string(),
        );
    }

    if msg_lower.contains("config.toml") || msg_lower.contains("--rpc-url is required") {
        return Some("Run `gravity-cli init` to set up your configuration file.".to_string());
    }

    if msg_lower.contains("--server-url is required") {
        return Some(
            "Set --server-url, GRAVITY_SERVER_URL env var, or run `gravity-cli init`.".to_string(),
        );
    }

    if msg_lower.contains("--deploy-path is required") {
        return Some(
            "Set --deploy-path, GRAVITY_DEPLOY_PATH env var, or run `gravity-cli init`."
                .to_string(),
        );
    }

    None
}
