// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

fn main() -> shadow_rs::SdResult<()> {
    // CARGO_CFG env vars don't make it to the program at runtime, so we
    // propagate it here by adding a new env var via this cargo directive.
    println!(
        "cargo:rustc-env=USING_TOKIO_UNSTABLE={}",
        std::env::var("CARGO_CFG_TOKIO_UNSTABLE").is_ok()
    );
    println!("cargo:rerun-if-changed=build.rs");
    // Check for this path first, otherwise it will force a rebuild every time
    // https://github.com/rust-lang/cargo/issues/4213
    let git_head = std::path::Path::new("../../.git/HEAD");
    if git_head.exists() {
        println!("cargo:rerun-if-changed=../../.git/HEAD");
    }
    // shadow-rs stamps BUILD_TIME with SOURCE_DATE_EPOCH when set, else the
    // wall clock. Default it to the HEAD commit time so rebuilding the same
    // commit embeds the same build time instead of a fresh timestamp; an
    // explicit SOURCE_DATE_EPOCH still wins, and builds without git (e.g.
    // docker) keep the wall clock.
    if std::env::var_os("SOURCE_DATE_EPOCH").is_none() {
        if let Some(epoch) = head_commit_epoch() {
            std::env::set_var("SOURCE_DATE_EPOCH", epoch);
        }
    }
    shadow_rs::ShadowBuilder::builder().deny_const(Default::default()).build()?;
    Ok(())
}

fn head_commit_epoch() -> Option<String> {
    let output =
        std::process::Command::new("git").args(["log", "-1", "--format=%ct"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let epoch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!epoch.is_empty()).then_some(epoch)
}
