use clap::Parser;
use gravity_sdk::GravityNodeArgs;


#[tokio::main]
async fn main() {
    GravityNodeArgs::parse().run();
}
