"""Keep validator Oracle and consensus logs visible in this E2E suite."""


def pre_deploy(test_dir, env, pytest_args):
    env["RUST_LOG"] = "info"
