mod common;
mod simulate;
mod trace;
mod trace_call;

use clap::{Parser, Subcommand};

pub use simulate::SimulateCommand;
pub use trace::TraceCommand;
pub use trace_call::TraceCallCommand;

#[derive(Debug, Parser)]
pub struct TxCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    /// Simulate a call via eth_call + eth_estimateGas (no signing, no submission)
    Simulate(SimulateCommand),
    /// Trace an already-mined transaction via debug_traceTransaction
    Trace(TraceCommand),
    /// Trace a hypothetical call via debug_traceCall (no submission)
    TraceCall(TraceCallCommand),
}
