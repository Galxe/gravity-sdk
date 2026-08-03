from gravity_e2e.utils import oracle_test_support as support


def _suite_dir(tmp_path):
    suite_dir = tmp_path / "gravity_e2e" / "cluster_test_cases" / "oracle_demo"
    suite_dir.mkdir(parents=True)
    return suite_dir


def test_contracts_repo_prefers_generated_local_checkout(tmp_path):
    suite_dir = _suite_dir(tmp_path)
    source_repo = tmp_path / "contracts_source"
    source_repo.mkdir()
    (suite_dir / "genesis.toml").write_text(
        "[dependencies.genesis_contracts]\n"
        f'path = "{source_repo}"\n'
        'ref = "deadbeef"\n'
    )
    generated_checkout = (
        tmp_path / "external" / "gravity_chain_core_contracts_local"
    )
    generated_checkout.mkdir(parents=True)

    assert support.contracts_repo_from_genesis(suite_dir) == generated_checkout


def test_contracts_repo_falls_back_to_configured_source(tmp_path):
    suite_dir = _suite_dir(tmp_path)
    source_repo = tmp_path / "contracts_source"
    source_repo.mkdir()
    (suite_dir / "genesis.toml").write_text(
        "[dependencies.genesis_contracts]\n"
        f'path = "{source_repo}"\n'
        'ref = "deadbeef"\n'
    )

    assert support.contracts_repo_from_genesis(suite_dir) == source_repo
