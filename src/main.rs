use clap::Parser;
use gravity_sdk::GravityNodeArgs;

fn main() {
    GravityNodeArgs::parse().run();
}
