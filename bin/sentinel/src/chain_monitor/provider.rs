use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::Filter;
use anyhow::Result;

// Use the full filler type returned by `connect_http`.
pub type HttpProvider = alloy_provider::fillers::FillProvider<
    alloy_provider::fillers::JoinFill<
        alloy_provider::Identity,
        alloy_provider::fillers::JoinFill<
            alloy_provider::fillers::GasFiller,
            alloy_provider::fillers::JoinFill<
                alloy_provider::fillers::BlobGasFiller,
                alloy_provider::fillers::JoinFill<
                    alloy_provider::fillers::NonceFiller,
                    alloy_provider::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    alloy_provider::RootProvider,
>;

/// Build an alloy HTTP provider for the given RPC URL.
pub fn build_provider(rpc_url: &str) -> Result<HttpProvider> {
    let url = rpc_url.parse().map_err(|e| anyhow::anyhow!("Invalid RPC URL '{rpc_url}': {e}"))?;
    let provider = ProviderBuilder::new().connect_http(url);
    Ok(provider)
}

/// Get the latest block number, with optional confirmation offset for reorg safety.
pub async fn get_safe_block_number(
    provider: &HttpProvider,
    confirmation_blocks: u64,
) -> Result<u64> {
    let latest = provider.get_block_number().await?;
    Ok(latest.saturating_sub(confirmation_blocks))
}

/// Fetch logs matching the given filter.
pub async fn get_logs(
    provider: &HttpProvider,
    filter: Filter,
) -> Result<Vec<alloy_rpc_types::Log>> {
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}
