use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use serde_json::{json, Value};
use std::{borrow::Cow, str::FromStr};

use crate::{
    command::Executable,
    output::OutputFormat,
    tx::{
        common::parse_hex_data,
        trace::{render_trace, Tracer},
    },
};

#[derive(Debug, Parser)]
pub struct TraceCallCommand {
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    #[clap(long)]
    pub from: Option<String>,

    #[clap(long)]
    pub to: Option<String>,

    #[clap(long, default_value = "0x")]
    pub data: String,

    #[clap(long, default_value = "0")]
    pub value: String,

    #[clap(long, default_value = "latest")]
    pub block: String,

    #[clap(long, value_enum, default_value = "call")]
    pub tracer: Tracer,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

impl Executable for TraceCallCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl TraceCallCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.clone().ok_or_else(|| {
            anyhow::anyhow!("--rpc-url is required")
        })?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let tx_json = build_call_object(&self)?;
        let opts = match self.tracer.geth_name() {
            Some(name) => json!({ "tracer": name }),
            None => json!({}),
        };

        let params = json!([tx_json, self.block, opts]);
        let trace: Value = provider
            .client()
            .request(Cow::Borrowed("debug_traceCall"), params)
            .await?;

        render_trace(&trace, self.tracer, self.output_format);
        Ok(())
    }
}

fn build_call_object(cmd: &TraceCallCommand) -> Result<Value, anyhow::Error> {
    let mut obj = serde_json::Map::new();
    if let Some(from) = &cmd.from {
        let _ = Address::from_str(from).map_err(|e| anyhow::anyhow!("invalid --from: {e}"))?;
        obj.insert("from".into(), json!(from));
    }
    if let Some(to) = &cmd.to {
        let _ = Address::from_str(to).map_err(|e| anyhow::anyhow!("invalid --to: {e}"))?;
        obj.insert("to".into(), json!(to));
    }
    let data = parse_hex_data(&cmd.data)
        .ok_or_else(|| anyhow::anyhow!("--data is not valid hex"))?;
    obj.insert("data".into(), json!(format!("0x{}", hex::encode::<Bytes>(data))));
    let value =
        U256::from_str(&cmd.value).map_err(|e| anyhow::anyhow!("invalid --value: {e}"))?;
    obj.insert("value".into(), json!(format!("0x{value:x}")));
    Ok(Value::Object(obj))
}

