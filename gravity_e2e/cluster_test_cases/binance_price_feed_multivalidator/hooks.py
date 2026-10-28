"""Suite-specific node logging and opt-in Byzantine Oracle QC fixture."""

from pathlib import Path


BAD_QC_E2E_ENV = "GRAVITY_ORACLE_BAD_QC_E2E"
RUNTIME_DIR = Path("/tmp/gravity-cluster-binance-price-feed-multivalidator")
BAD_QC_CONTROL_PATH = RUNTIME_DIR / "invalid_oracle_qc_control.json"
BAD_QC_EVIDENCE_PATH = RUNTIME_DIR / "invalid_oracle_qc_evidence.json"


def pre_deploy(test_dir, env, pytest_args):
    # Aptos uses RUST_LOG in preference to validator.yaml's logger level.
    # Keep observer, vote and SDK execution evidence visible even when the
    # caller runs the suite with RUST_LOG=warn.
    env["RUST_LOG"] = "info"
    if env.get(BAD_QC_E2E_ENV) == "1":
        # Only binaries built with `--features byzantine-test` read these
        # variables. The test creates its control file after governance setup.
        env["BFT_BYZANTINE_FIXTURE_ENABLED"] = "1"
        env["BFT_BYZANTINE_CONTROL_PATH"] = str(BAD_QC_CONTROL_PATH)
        env["BFT_BYZANTINE_EVIDENCE_PATH"] = str(BAD_QC_EVIDENCE_PATH)
