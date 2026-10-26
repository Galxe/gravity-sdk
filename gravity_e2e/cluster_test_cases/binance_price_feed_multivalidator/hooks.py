"""Suite-specific node logging needed for Oracle quorum assertions."""


def pre_deploy(test_dir, env, pytest_args):
    # Aptos uses RUST_LOG in preference to validator.yaml's logger level.
    # Keep observer, vote and SDK execution evidence visible even when the
    # caller runs the suite with RUST_LOG=warn.
    env["RUST_LOG"] = "info"
