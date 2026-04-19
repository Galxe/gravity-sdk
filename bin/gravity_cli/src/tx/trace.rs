use alloy_primitives::B256;
use alloy_provider::{Provider, ProviderBuilder};
use clap::{Parser, ValueEnum};
use serde_json::{json, Value};
use std::{borrow::Cow, str::FromStr};

use crate::{command::Executable, output::OutputFormat};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Tracer {
    /// Geth callTracer — returns a tree of CALL/STATICCALL/DELEGATECALL frames
    Call,
    /// Geth prestateTracer — returns pre-execution state of touched accounts
    Prestate,
    /// Geth 4byteTracer — returns histogram of 4-byte selectors invoked
    #[clap(name = "4byte")]
    FourByte,
    /// Opcode-level structured trace (default in reth / geth)
    Opcode,
    /// Geth noopTracer — for latency benchmarking
    Noop,
}

impl Tracer {
    /// Returns the geth-compatible tracer name, or None for the default
    /// (opcode-level) tracer which is indicated by omitting the tracer field.
    pub(crate) fn geth_name(self) -> Option<&'static str> {
        match self {
            Tracer::Call => Some("callTracer"),
            Tracer::Prestate => Some("prestateTracer"),
            Tracer::FourByte => Some("4byteTracer"),
            Tracer::Noop => Some("noopTracer"),
            Tracer::Opcode => None,
        }
    }
}

#[derive(Debug, Parser)]
pub struct TraceCommand {
    /// Transaction hash to trace
    #[clap(value_name = "TX_HASH")]
    pub tx_hash: String,

    /// RPC URL
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Tracer to use
    #[clap(long, value_enum, default_value = "call")]
    pub tracer: Tracer,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

impl Executable for TraceCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl TraceCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url =
            self.rpc_url.clone().ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let tx_hash =
            B256::from_str(&self.tx_hash).map_err(|e| anyhow::anyhow!("invalid tx hash: {e}"))?;

        let opts = match Tracer::geth_name(self.tracer) {
            Some(name) => json!({ "tracer": name }),
            None => json!({}),
        };
        let params = json!([tx_hash, opts]);
        let trace: Value =
            provider.client().request(Cow::Borrowed("debug_traceTransaction"), params).await?;

        render_trace(&trace, self.tracer, self.output_format);
        Ok(())
    }
}

pub fn render_trace(trace: &Value, tracer: Tracer, fmt: OutputFormat) {
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(trace).unwrap_or_default());
        }
        OutputFormat::Plain => match tracer {
            Tracer::Call => render_call_tree(trace, 0),
            _ => {
                // Other tracers have varied shapes; pretty-print the JSON.
                println!("{}", serde_json::to_string_pretty(trace).unwrap_or_default());
            }
        },
    }
}

/// Render a geth callTracer frame as an indented tree:
///   CALL  from -> to  value=N gas=N/N
///     CALL ...
fn render_call_tree(frame: &Value, depth: usize) {
    let pad = "  ".repeat(depth);
    let type_ = frame.get("type").and_then(Value::as_str).unwrap_or("?");
    let from = frame.get("from").and_then(Value::as_str).unwrap_or("?");
    let to = frame.get("to").and_then(Value::as_str).unwrap_or("?");
    let value = frame.get("value").and_then(Value::as_str).unwrap_or("0x0");
    let gas = frame.get("gas").and_then(Value::as_str).unwrap_or("?");
    let gas_used = frame.get("gasUsed").and_then(Value::as_str).unwrap_or("?");
    let error = frame.get("error").and_then(Value::as_str);
    let reverted = frame.get("revertReason").and_then(Value::as_str);

    let tag = if reverted.is_some() || error.is_some() { "✗" } else { "•" };
    print!("{pad}{tag} {type_} {from} → {to}");
    if value != "0x0" {
        print!(" value={value}");
    }
    print!(" gas={gas_used}/{gas}");
    if let Some(e) = error {
        print!(" error={e}");
    }
    if let Some(r) = reverted {
        print!(" revert=\"{r}\"");
    }
    println!();

    if let Some(calls) = frame.get("calls").and_then(Value::as_array) {
        for call in calls {
            render_call_tree(call, depth + 1);
        }
    }
}
