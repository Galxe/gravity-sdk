use build_info::{
    build_information, BUILD_BRANCH, BUILD_CLEAN_CHECKOUT, BUILD_COMMIT_HASH, BUILD_OS,
    BUILD_PKG_VERSION, BUILD_PROFILE_NAME, BUILD_RUST_VERSION, BUILD_TAG, BUILD_TIME,
};
use gaptos::aptos_metrics_core::{register_int_gauge_vec, IntGaugeVec};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
};
use tracing::warn;

static GRAVITY_NODE_BUILD_INFO: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "gravity_node_build_info",
        "Static gravity_node build and binary information",
        &[
            "package_version",
            "commit_hash",
            "branch",
            "tag",
            "build_time",
            "build_os",
            "rust_version",
            "profile",
            "clean_checkout",
            "binary_sha256",
        ],
    )
    .unwrap()
});

pub(crate) fn register_binary_info_metrics() {
    let build_info = build_information!();
    let binary_sha256 = current_binary_sha256().unwrap_or_else(|err| {
        warn!("Failed to compute gravity_node binary sha256: {err}");
        "unknown".to_string()
    });

    GRAVITY_NODE_BUILD_INFO
        .with_label_values(&[
            build_info_value(&build_info, BUILD_PKG_VERSION),
            build_info_value(&build_info, BUILD_COMMIT_HASH),
            build_info_value(&build_info, BUILD_BRANCH),
            build_info_value(&build_info, BUILD_TAG),
            build_info_value(&build_info, BUILD_TIME),
            build_info_value(&build_info, BUILD_OS),
            build_info_value(&build_info, BUILD_RUST_VERSION),
            build_info_value(&build_info, BUILD_PROFILE_NAME),
            build_info_value(&build_info, BUILD_CLEAN_CHECKOUT),
            binary_sha256.as_str(),
        ])
        .set(1);
}

fn build_info_value<'a>(build_info: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    build_info.get(key).map(String::as_str).unwrap_or("unknown")
}

fn current_binary_sha256() -> io::Result<String> {
    let mut file = File::open(std::env::current_exe()?)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}
