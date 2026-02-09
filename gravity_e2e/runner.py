import os
import sys
import subprocess
import glob
import time
import signal
import logging
import argparse
import shutil
from pathlib import Path

try:
    import tomli
except ImportError:
    # Fallback to toml if tomli is not available, or just fail if requirements not met
    # Assuming tomli is installed via requirements.txt
    import tomli

# Configure logging
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger("GravityRunner")

PROJECT_ROOT = Path(__file__).resolve().parent.parent
E2E_ROOT = PROJECT_ROOT / "gravity_e2e"
TESTS_ROOT = E2E_ROOT / "cluster_test_cases"
CLUSTER_SCRIPTS_DIR = PROJECT_ROOT / "cluster"


def run_command(command, cwd=None, env=None, check=True, stream_output=True):
    """Run a shell command and optionally stream output in real-time."""
    logger.info(f"Running: {' '.join(command)}")
    try:
        if stream_output:
            # Stream output in real-time (inherits parent's stdout/stderr)
            result = subprocess.run(command, cwd=cwd, env=env, check=check, text=True)
            return result.returncode == 0
        else:
            # Capture output (for cases where we need to process it)
            result = subprocess.run(
                command, cwd=cwd, env=env, check=check, capture_output=True, text=True
            )
            # Print stdout if any
            if result.stdout:
                for line in result.stdout.strip().split("\n"):
                    if line:
                        print(line)
            return result.returncode == 0
    except subprocess.CalledProcessError as e:
        logger.error(f"Command failed with exit code {e.returncode}")
        if e.stdout:
            logger.info("--- STDOUT ---")
            for line in e.stdout.strip().split("\n"):
                logger.info(line)
        if e.stderr:
            logger.error("--- STDERR ---")
            for line in e.stderr.strip().split("\n"):
                logger.error(line)
        if check:
            raise
        return False
    except Exception as e:
        logger.error(f"Unexpected error running command: {type(e).__name__}: {e}")
        import traceback

        logger.error(traceback.format_exc())
        raise


def init_faucet_if_needed(test_dir: Path, cluster_config_path: Path, env: dict):
    """
    Check if cluster.toml requests faucet init, and run faucet.sh if so.
    """
    # Simply call the shell script. It handles the parsing and logic itself.
    # But we want to avoid calling it if not needed?
    # Actually, the shell script checks for num_accounts > 0 and exits cleanly if not match.
    # So we can just call it. But to avoid process overhead, we can check basic condition here if needed.
    # However, for consistency and simplicity, let's just calling it.

    faucet_script = CLUSTER_SCRIPTS_DIR / "faucet.sh"
    if not faucet_script.exists():
        logger.warning("cluster/faucet.sh not found.")
        return

    logger.info("Checking/Running faucet init...")
    # Passing the config file is key
    run_command(
        ["bash", str(faucet_script), str(cluster_config_path)],
        cwd=CLUSTER_SCRIPTS_DIR,
        env=env,
        check=True,
    )


def cleanup_cluster():
    """Kill any running gravity_node processes."""
    logger.info("Cleaning up running nodes...")
    # This is a bit aggressive but necessary for clean slate in docker
    subprocess.run(["pkill", "-9", "gravity_node"], check=False)


def run_test_suite(
    test_dir: Path,
    no_cleanup: bool = False,
    pytest_args: list = None,
    force_init: bool = False,
):
    """
    Run tests in a specific directory.
    """
    cluster_config = test_dir / "cluster.toml"
    genesis_config = test_dir / "genesis.toml"

    if not cluster_config.exists():
        logger.warning(f"No cluster.toml found in {test_dir}, skimming...")
        return

    logger.info(f"===== Running Suite: {test_dir.name} =====")

    # Always clean start unless specifically investigating
    cleanup_cluster()

    # Remove stale cluster data directory.
    # deploy.sh has an interactive prompt that defaults to N in
    # non-interactive mode, so we must clean it here.
    import shutil
    try:
        import tomllib
    except ImportError:
        import toml as tomllib
    with open(cluster_config, "rb") as f:
        toml_data = tomllib.load(f) if hasattr(tomllib, 'load') else {}
    base_dir = toml_data.get("cluster", {}).get("base_dir", "")
    if base_dir and os.path.exists(base_dir):
        logger.info(f"Removing stale cluster data at {base_dir}")
        shutil.rmtree(base_dir)

    # Define artifact paths
    # We now instruct init.sh/deploy.sh to use this directory directly
    suite_artifacts_dir = test_dir / "artifacts"
    env = os.environ.copy()
    env["GRAVITY_ARTIFACTS_DIR"] = str(suite_artifacts_dir)
    
    # Set genesis config path if it exists
    if genesis_config.exists():
        env["GENESIS_CONFIG_FILE"] = str(genesis_config)

    # 1. Init Cluster (with Caching)
    should_run_init = True

    # Check if valid artifacts exist
    if (
        not force_init
        and suite_artifacts_dir.exists()
        and (suite_artifacts_dir / "genesis.json").exists()
    ):
        logger.info(f"Found cached artifacts in {suite_artifacts_dir}. Using cache.")
        should_run_init = False

    if should_run_init:
        logger.info(
            f"Initializing cluster (Generating artifacts in {suite_artifacts_dir})..."
        )
        
        # Step 1: init.sh (generate identity keys)
        init_script = CLUSTER_SCRIPTS_DIR / "init.sh"
        if genesis_config.exists():
            run_command(
                ["bash", str(init_script), str(genesis_config)],
                cwd=CLUSTER_SCRIPTS_DIR,
                env=env,
            )
        else:
            # Fallback to cluster.toml for backwards compatibility
            run_command(
                ["bash", str(init_script), str(cluster_config)],
                cwd=CLUSTER_SCRIPTS_DIR,
                env=env,
            )
        
        # Step 2: genesis.sh (generate genesis.json)
        genesis_script = CLUSTER_SCRIPTS_DIR / "genesis.sh"
        if genesis_script.exists() and genesis_config.exists():
            logger.info("Generating genesis...")
            run_command(
                ["bash", str(genesis_script), str(genesis_config)],
                cwd=CLUSTER_SCRIPTS_DIR,
                env=env,
            )

    # 2. Deploy Cluster
    logger.info("Deploying cluster...")
    deploy_script = CLUSTER_SCRIPTS_DIR / "deploy.sh"

    # Auto-detect per-test-case relayer config
    custom_relayer_config = test_dir / "relayer_config.json"
    if custom_relayer_config.exists():
        env["RELAYER_CONFIG_TPL"] = str(custom_relayer_config)
        logger.info(f"Using custom relayer config: {custom_relayer_config}")

    run_command(
        ["bash", str(deploy_script), str(cluster_config)],
        cwd=CLUSTER_SCRIPTS_DIR,
        env=env,
    )

    # 2.5 Pre-start hook (e.g. start MockAnvil before node)
    hooks = None
    hooks_path = test_dir / "hooks.py"
    if hooks_path.exists():
        import importlib.util
        spec = importlib.util.spec_from_file_location("hooks", hooks_path)
        hooks = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(hooks)
        if hasattr(hooks, "pre_start"):
            logger.info(f"Running pre_start hook from {hooks_path}")
            hooks.pre_start(test_dir, env, pytest_args or [])

    # 3. Start Nodes
    start_script = CLUSTER_SCRIPTS_DIR / "start.sh"
    if start_script.exists():
        logger.info("Starting cluster nodes...")
        # start.sh doesn't mostly need artifacts dir (it uses config from deploy),
        # but passing env doesn't hurt.
        run_command(
            ["bash", str(start_script), "--config", str(cluster_config)],
            cwd=CLUSTER_SCRIPTS_DIR,
            env=env,
        )

        logger.info("Waiting 5s for nodes to warmup...")
        time.sleep(5)
    else:
        logger.error("cluster/start.sh missing, cannot start nodes!")
        raise RuntimeError("Missing start.sh")

    # 3.5 Faucet Initialization
    init_faucet_if_needed(test_dir, cluster_config, env)

    # 4. Run Pytests
    logger.info(f"Running pytst in {test_dir}...")
    success = False
    try:
        # We need to make sure gravity_e2e is in python path
        # env already has GRAVITY_ARTIFACTS_DIR, but pytest might not need it
        env["PYTHONPATH"] = f"{E2E_ROOT}:{env.get('PYTHONPATH', '')}"
        env["GRAVITY_CLUSTER_CONFIG"] = str(cluster_config)

        # Build pytest command
        cmd = ["python3", "-m", "pytest", "-s", str(test_dir)]
        if pytest_args:
            cmd.extend(pytest_args)

        run_command(cmd, cwd=E2E_ROOT, env=env)
        logger.info(f"Suite {test_dir.name} PASSED")
        success = True

    except Exception as e:
        logger.error(f"Suite {test_dir.name} FAILED: {e}")
        raise
    finally:
        # 5. Teardown
        if no_cleanup and not success:
            logger.warning(
                f"Test failed and --no-cleanup set. Cluster left running using config: {cluster_config}"
            )
            logger.warning("Run 'shutdown_cluster()' or kill manually when done.")
        else:
            stop_script = CLUSTER_SCRIPTS_DIR / "stop.sh"
            if stop_script.exists():
                run_command(
                    ["bash", str(stop_script), "--config", str(cluster_config)],
                    cwd=CLUSTER_SCRIPTS_DIR,
                    env=env,
                    check=False,
                )

            cleanup_cluster()

        # Post-stop hook ALWAYS runs (e.g. stop MockAnvil).
        # MockAnvil runs as a daemon thread in this process — it dies
        # when we exit anyway, but explicit cleanup is cleaner and
        # prevents the relayer from crashing on connection loss.
        if hooks and hasattr(hooks, "post_stop"):
            logger.info("Running post_stop hook...")
            try:
                hooks.post_stop(test_dir, env)
            except Exception as e:
                logger.warning(f"post_stop hook failed: {e}")


def main():
    parser = argparse.ArgumentParser(description="Gravity E2E Runner")
    parser.add_argument(
        "--no-cleanup",
        action="store_true",
        help="Leave cluster running if tests fail (for debugging)",
    )
    parser.add_argument(
        "--force-init",
        action="store_true",
        help="Force regeneration of cluster artifacts (ignore cache)",
    )
    # We use parse_known_args to let everything else slide through as potential pytest args or suites
    args, unknown = parser.parse_known_args()

    try:
        # Discover test directories
        all_test_dirs = [p for p in TESTS_ROOT.iterdir() if p.is_dir()]

        # Smart separation of suites vs pytest flags
        # Anything starting with '-' is a pytest flag.
        # Anything matching a directory name is a suite.
        # Anything else is likely a pytest arg (e.g. regex for -k)

        suites_to_run = []
        pytest_args = []

        # Iterate over unknown args (positionals + unparsed flags)
        # Note: argparsing is tricky. We'll iterate simply.

        for arg in unknown:
            if arg.startswith("-"):
                pytest_args.append(arg)
                continue

            # Check if it matches a suite name
            matched_suite = None
            for p in all_test_dirs:
                if p.name == arg:
                    matched_suite = p
                    break

            if matched_suite:
                suites_to_run.append(matched_suite)
            else:
                # If not a suite, assume it's an argument to a previous flag (like 'test_foo' after '-k')
                pytest_args.append(arg)

        # If no specific suites named, run all
        if not suites_to_run:
            test_dirs = all_test_dirs
        else:
            test_dirs = suites_to_run

        if not test_dirs:
            logger.error("No valid test suites found to run.")
            sys.exit(1)

        logger.info(
            f"Running {len(test_dirs)} test directories: {[t.name for t in test_dirs]}"
        )
        if pytest_args:
            logger.info(f"Forwarding args to pytest: {pytest_args}")

        failed_suites = []

        for test_dir in sorted(test_dirs):
            # Skip __pycache__ etc
            if test_dir.name.startswith("__") or test_dir.name.startswith("."):
                continue

            try:
                run_test_suite(
                    test_dir,
                    no_cleanup=args.no_cleanup,
                    pytest_args=pytest_args,
                    force_init=args.force_init,
                )
            except Exception:
                failed_suites.append(test_dir.name)

        if failed_suites:
            logger.error(f"The following suites failed: {failed_suites}")
            sys.exit(1)

        logger.info("All suites passed!")

    except Exception as e:
        logger.exception("Global runner failure")
        sys.exit(1)


if __name__ == "__main__":
    main()
