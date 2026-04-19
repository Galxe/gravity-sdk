use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use clap::Parser;
use serde::Serialize;
use std::str::FromStr;

use crate::{command::Executable, output::OutputFormat, tx::common::{decode_revert, parse_hex_data}};

#[derive(Debug, Parser)]
pub struct SimulateCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Sender address (msg.sender for the simulated call)
    #[clap(long)]
    pub from: Option<String>,

    /// Destination address (None = contract creation)
    #[clap(long)]
    pub to: Option<String>,

    /// Hex-encoded calldata (empty for plain transfer)
    #[clap(long, default_value = "0x")]
    pub data: String,

    /// Value in wei (not ETH)
    #[clap(long, default_value = "0")]
    pub value: String,

    /// Block tag or number (latest, pending, earliest, finalized, safe, or a decimal/hex number)
    #[clap(long, default_value = "latest")]
    pub block: String,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct SimulateResult {
    success: bool,
    return_data: Option<String>,
    gas_estimate: Option<u64>,
    revert_reason: Option<String>,
    error: Option<String>,
}

impl Executable for SimulateCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl SimulateCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "--rpc-url is required. Set via CLI flag, GRAVITY_RPC_URL env var, or ~/.gravity/config.toml"
            )
        })?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let tx = build_tx_request(&self)?;
        let block = parse_block_tag(&self.block)?;

        // Attempt eth_call first — this gives us return data or revert reason.
        let call_res = provider.call(tx.clone()).block(block.into()).await;
        // Then estimate gas — only meaningful if the call succeeded, but we try
        // regardless so the user sees the estimate when possible.
        let gas_res = provider.estimate_gas(tx).block(block.into()).await;

        let result = match call_res {
            Ok(bytes) => SimulateResult {
                success: true,
                return_data: Some(format!("0x{}", hex::encode(&bytes))),
                gas_estimate: gas_res.ok(),
                revert_reason: None,
                error: None,
            },
            Err(e) => {
                let revert = extract_revert_reason(&e);
                SimulateResult {
                    success: false,
                    return_data: None,
                    gas_estimate: None,
                    revert_reason: revert,
                    error: Some(e.to_string()),
                }
            }
        };

        emit_result(&result, self.output_format);
        if !result.success {
            std::process::exit(1);
        }
        Ok(())
    }
}

fn build_tx_request(cmd: &SimulateCommand) -> Result<TransactionRequest, anyhow::Error> {
    let from = cmd
        .from
        .as_deref()
        .map(Address::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("invalid --from: {e}"))?;
    let to_kind = match cmd.to.as_deref() {
        Some(addr) => {
            let parsed =
                Address::from_str(addr).map_err(|e| anyhow::anyhow!("invalid --to: {e}"))?;
            TxKind::Call(parsed)
        }
        None => TxKind::Create,
    };
    let data = parse_hex_data(&cmd.data)
        .ok_or_else(|| anyhow::anyhow!("--data is not valid hex"))?;
    let value = U256::from_str(&cmd.value).map_err(|e| anyhow::anyhow!("invalid --value: {e}"))?;

    Ok(TransactionRequest {
        from,
        to: Some(to_kind),
        input: TransactionInput::new(Bytes::from(data)),
        value: Some(value),
        ..Default::default()
    })
}

fn parse_block_tag(s: &str) -> Result<BlockNumberOrTag, anyhow::Error> {
    BlockNumberOrTag::from_str(s).map_err(|e| anyhow::anyhow!("invalid --block: {e}"))
}

/// Try to pull revert bytes out of a transport error and decode them.
fn extract_revert_reason<E: std::error::Error + 'static>(err: &E) -> Option<String> {
    // Walk the error chain looking for a serde representation that contains
    // JSON-RPC error payload with a `data` field — most alloy transport errors
    // serialize this way in Display. As a fallback, scan the Display string.
    let msg = err.to_string();
    // Look for the common `data: 0x…` or `data: "0x…"` pattern that alloy emits.
    if let Some(idx) = msg.find("data: ") {
        let tail = &msg[idx + "data: ".len()..];
        let end = tail.find(|c: char| c == ',' || c == '}' || c == ')').unwrap_or(tail.len());
        let raw = &tail[..end];
        if let Some(bytes) = parse_hex_data(raw) {
            return Some(decode_revert(&bytes));
        }
    }
    // Fallback: many reverts surface as `execution reverted: <reason>` in the message.
    if let Some(idx) = msg.find("execution reverted") {
        let tail = &msg[idx..];
        let line_end = tail.find('\n').unwrap_or(tail.len());
        return Some(tail[..line_end].trim().to_string());
    }
    None
}

fn emit_result(r: &SimulateResult, fmt: OutputFormat) {
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(r).unwrap());
        }
        OutputFormat::Plain => {
            if r.success {
                println!("success");
                if let Some(ref data) = r.return_data {
                    println!("return: {data}");
                }
                if let Some(gas) = r.gas_estimate {
                    println!("gas:    {gas}");
                }
            } else {
                println!("reverted");
                if let Some(ref reason) = r.revert_reason {
                    println!("reason: {reason}");
                }
                if let Some(ref err) = r.error {
                    println!("rpc:    {err}");
                }
            }
        }
    }
}
