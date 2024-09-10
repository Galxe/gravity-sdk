use clap::Parser;
use gravity_consensus::GravityNodeArgs;

#[tokio::main]
async fn main() {
    GravityNodeArgs::parse().run();
}
