import json
import os
import sys

def parse_simple_yaml(path):
    """
    Parses a simple key: value YAML file.
    Assumes no nesting and standard formatting from gravity_cli.
    """
    data = {}
    with open(path, 'r') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            if ':' in line:
                key, value = line.split(':', 1)
                key = key.strip()
                value = value.strip()
                # Remove quotes if present
                if (value.startswith('"') and value.endswith('"')) or \
                   (value.startswith("'") and value.endswith("'")):
                    value = value[1:-1]
                data[key] = value
    return data

def get_genesis_defaults():
    """Returns default genesis configuration values."""
    return {
        "chainId": 1337,  # Default value, can be overridden in genesis.toml
        "epochIntervalMicros": 7200000000,
        "majorVersion": 1,
        "consensusConfig": "0x00",
        "executionConfig": "0x00",
        "validatorConfig": {
            "minimumBond": "1000000000000000000",
            "maximumBond": "1000000000000000000000000",
            "unbondingDelayMicros": 604800000000,
            "allowValidatorSetChange": True,
            "votingPowerIncreaseLimitPct": 20,
            "maxValidatorSetSize": "100"
        },
        "stakingConfig": {
            "minimumStake": "1000000000000000000",
            "lockupDurationMicros": 86400000000,
            "unbondingDelayMicros": 86400000000,
            "minimumProposalStake": "10000000000000000000"
        },
        "governanceConfig": {
            "minVotingThreshold": "1000000000000000000",
            "requiredProposerStake": "10000000000000000000",
            "votingDurationMicros": 604800000000
        },
        "randomnessConfig": {
            "variant": 0,
            "configV2": {
                "secrecyThreshold": 0,
                "reconstructionThreshold": 0,
                "fastPathSecrecyThreshold": 0
            }
        },
        "oracleConfig": {
            "sourceTypes": [1],
            "callbacks": ["0x00000000000000000000000000000001625F2018"],
            "bridgeConfig": None,
            "tasks": []
        },
        "jwkConfig": {
            "issuers": ["0x68747470733a2f2f6163636f756e74732e676f6f676c652e636f6d"],
            "jwks": [[{
                "kid": "f5f4c0ae6e6090a65ab0a694d6ba6f19d5d0b4e6",
                "kty": "RSA",
                "alg": "RS256",
                "e": "AQAB",
                "n": "2K7epoJWl_aBoYGpXmDBBiEnwQ0QdVRU1gsbGXNrEbrZEQdY5KjH5P5gZMq3d3KvT1j5KsD2tF_9jFMDLqV4VWDNJRLgSNJxhJuO_oLO2BXUSL9a7fLHxnZCUfJvT2K-O8AXjT3_ZM8UuL8d4jBn_fZLzdEI4MHrZLVSaHDvvKqL_mExQo6cFD-qyLZ-T6aHv2x8R7L_3X7E1nGMjKVVZMveQ_HMeXvnGxKf5yfEP0hIQlC_kFm4L_1kV1S0UPmMptZL2qI4VnXqmqI6TZJyE-3VXHgNn1Z1O_9QZlPC0fF0spLHf2S3nNqI0v3k2E7q3DkqxVf5xvn7q_X-gPqzVE9Jw"
            }]]
        }
    }

def build_genesis_config(config, genesis_cfg):
    """Build genesis config from cluster config, using defaults where not specified."""
    defaults = get_genesis_defaults()
    
    # Override with values from cluster.toml if present
    result = {}
    
    # Top-level fields
    result["chainId"] = genesis_cfg.get("chain_id", defaults["chainId"])
    result["epochIntervalMicros"] = genesis_cfg.get("epoch_interval_micros", defaults["epochIntervalMicros"])
    result["majorVersion"] = genesis_cfg.get("major_version", defaults["majorVersion"])
    result["consensusConfig"] = genesis_cfg.get("consensus_config", defaults["consensusConfig"])
    result["executionConfig"] = genesis_cfg.get("execution_config", defaults["executionConfig"])
    
    # validatorConfig
    vc = genesis_cfg.get("validator_config", {})
    result["validatorConfig"] = {
        "minimumBond": vc.get("minimum_bond", defaults["validatorConfig"]["minimumBond"]),
        "maximumBond": vc.get("maximum_bond", defaults["validatorConfig"]["maximumBond"]),
        "unbondingDelayMicros": vc.get("unbonding_delay_micros", defaults["validatorConfig"]["unbondingDelayMicros"]),
        "allowValidatorSetChange": vc.get("allow_validator_set_change", defaults["validatorConfig"]["allowValidatorSetChange"]),
        "votingPowerIncreaseLimitPct": vc.get("voting_power_increase_limit_pct", defaults["validatorConfig"]["votingPowerIncreaseLimitPct"]),
        "maxValidatorSetSize": vc.get("max_validator_set_size", defaults["validatorConfig"]["maxValidatorSetSize"])
    }
    
    # stakingConfig
    sc = genesis_cfg.get("staking_config", {})
    result["stakingConfig"] = {
        "minimumStake": sc.get("minimum_stake", defaults["stakingConfig"]["minimumStake"]),
        "lockupDurationMicros": sc.get("lockup_duration_micros", defaults["stakingConfig"]["lockupDurationMicros"]),
        "unbondingDelayMicros": sc.get("unbonding_delay_micros", defaults["stakingConfig"]["unbondingDelayMicros"]),
        "minimumProposalStake": sc.get("minimum_proposal_stake", defaults["stakingConfig"]["minimumProposalStake"])
    }
    
    # governanceConfig
    gc = genesis_cfg.get("governance_config", {})
    result["governanceConfig"] = {
        "minVotingThreshold": gc.get("min_voting_threshold", defaults["governanceConfig"]["minVotingThreshold"]),
        "requiredProposerStake": gc.get("required_proposer_stake", defaults["governanceConfig"]["requiredProposerStake"]),
        "votingDurationMicros": gc.get("voting_duration_micros", defaults["governanceConfig"]["votingDurationMicros"])
    }
    
    # randomnessConfig
    rc = genesis_cfg.get("randomness_config", {})
    result["randomnessConfig"] = {
        "variant": rc.get("variant", defaults["randomnessConfig"]["variant"]),
        "configV2": {
            "secrecyThreshold": rc.get("secrecy_threshold", defaults["randomnessConfig"]["configV2"]["secrecyThreshold"]),
            "reconstructionThreshold": rc.get("reconstruction_threshold", defaults["randomnessConfig"]["configV2"]["reconstructionThreshold"]),
            "fastPathSecrecyThreshold": rc.get("fast_path_secrecy_threshold", defaults["randomnessConfig"]["configV2"]["fastPathSecrecyThreshold"])
        }
    }
    
    # oracleConfig
    oc = genesis_cfg.get("oracle_config", {})
    result["oracleConfig"] = {
        "sourceTypes": oc.get("source_types", defaults["oracleConfig"]["sourceTypes"]),
        "callbacks": oc.get("callbacks", defaults["oracleConfig"]["callbacks"])
    }
    
    # bridgeConfig (optional)
    bc = oc.get("bridge_config", {})
    if bc:
        result["oracleConfig"]["bridgeConfig"] = {
            "deploy": bc.get("deploy", False),
            "trustedBridge": bc.get("trusted_bridge", "0x0000000000000000000000000000000000000000")
        }
    
    # tasks (optional)
    tasks_cfg = oc.get("tasks", [])
    if tasks_cfg:
        result["oracleConfig"]["tasks"] = [
            {
                "sourceType": t.get("source_type"),
                "sourceId": t.get("source_id"),
                "taskName": t.get("task_name"),
                "config": t.get("config")
            }
            for t in tasks_cfg
        ]
    
    # jwkConfig
    jc = genesis_cfg.get("jwk_config", {})
    if jc:
        jwks_list = jc.get("jwks", [])
        # Convert TOML array of tables to nested array format
        jwks_formatted = [[jwk] for jwk in jwks_list] if jwks_list else defaults["jwkConfig"]["jwks"]
        result["jwkConfig"] = {
            "issuers": jc.get("issuers", defaults["jwkConfig"]["issuers"]),
            "jwks": jwks_formatted
        }
    else:
        result["jwkConfig"] = defaults["jwkConfig"]
    
    return result

def main():
    if len(sys.argv) < 2:
        print("Usage: aggregate_genesis.py <config_json_string> [--genesis-mode]")
        sys.exit(1)

    # Read config from JSON string argument
    config_json_str = sys.argv[1]
    genesis_mode = '--genesis-mode' in sys.argv
    
    try:
        config = json.loads(config_json_str)
    except json.JSONDecodeError as e:
        print(f"Error parsing config JSON: {e}")
        sys.exit(1)
    
    genesis_cfg = config.get('genesis', {})
    
    # Determine directories
    if genesis_mode:
        # In genesis mode:
        #   - output_dir: where init.sh put identity keys and where we write output
        output_dir = os.environ.get('GRAVITY_ARTIFACTS_DIR', os.path.join(os.path.dirname(__file__), '..', 'output'))
        output_dir = os.path.abspath(output_dir)
        # Read genesis_validators directly from genesis.toml
        genesis_nodes = config.get('genesis_validators', [])
        print(f"[Aggregator] Genesis mode: processing {len(genesis_nodes)} genesis validators...")
    else:
        # Legacy mode: read from cluster.toml
        output_dir = config['cluster']['base_dir']
        nodes = config['nodes']
        # Filter to only genesis nodes
        genesis_nodes = [n for n in nodes if n.get('role') == 'genesis']
        print(f"[Aggregator] Processing {len(genesis_nodes)} genesis nodes for initial validator set (skipping {len(nodes) - len(genesis_nodes)} non-genesis nodes)...")
    
    validators = []

    for node in genesis_nodes:
        node_id = node['id']
        # Identity keys are at: output/nodeX/config/identity.yaml
        data_dir = node.get('data_dir') or os.path.join(output_dir, node_id)
        identity_path = os.path.join(data_dir, "config", "identity.yaml")
        
        if not os.path.exists(identity_path):
            print(f"Error: Identity file not found: {identity_path}")
            sys.exit(1)
            
        identity = parse_simple_yaml(identity_path)
        
        # Validation
        required_keys = ['account_address', 'consensus_public_key', 'network_public_key']
        for k in required_keys:
            if k not in identity:
                print(f"Error: Missing '{k}' in {identity_path}")
                print("Make sure gravity_cli is updated to output public keys.")
                sys.exit(1)
        
        # Get validator address from config (required)
        val_addr = node.get('address')
        if not val_addr:
            print(f"Error: Node {node_id} must specify 'address' in genesis.toml")
            sys.exit(1)
        
        # Validate ETH address format
        if not val_addr.startswith('0x') or len(val_addr) != 42:
            print(f"Error: Node {node_id}: address must be 0x-prefixed 20-byte ETH address")
            sys.exit(1)
        
        # Get stake_amount and voting_power from config (required)
        stake_amount = node.get('stake_amount')
        voting_power = node.get('voting_power')
        
        if not stake_amount:
            print(f"Error: Node {node_id} must specify 'stake_amount' in genesis.toml")
            sys.exit(1)
        if not voting_power:
            print(f"Error: Node {node_id} must specify 'voting_power' in genesis.toml")
            sys.exit(1)
        
        # Validate voting_power >= stake_amount
        if int(voting_power) < int(stake_amount):
            print(f"Error: Node {node_id}: voting_power ({voting_power}) must be >= stake_amount ({stake_amount})")
            sys.exit(1)
            
        consensus_pk = identity['consensus_public_key']
        if not consensus_pk.startswith('0x'):
            consensus_pk = f"0x{consensus_pk}"
            
        network_pk = identity['network_public_key']
        if network_pk.startswith('0x'):
            network_pk = network_pk[2:]

        # Network info
        host = node['host']
        p2p_port = node['p2p_port']
        vfn_port = node['vfn_port']
        
        # Build addresses
        val_net_addr = f"/ip4/{host}/tcp/{p2p_port}/noise-ik/{network_pk}/handshake/0"
        vfn_net_addr = f"/ip4/{host}/tcp/{vfn_port}/noise-ik/{network_pk}/handshake/0"
        
        # Create validator entry
        validator = {
            "operator": val_addr,
            "owner": val_addr,
            "stakeAmount": stake_amount,
            "moniker": f"validator-{len(validators) + 1}",
            "consensusPubkey": consensus_pk,
            "consensusPop": "0x",
            "networkAddresses": val_net_addr,
            "fullnodeAddresses": vfn_net_addr,
            "votingPower": voting_power
        }
        validators.append(validator)

    # Build complete genesis config (matching GenesisConfig struct in genesis.rs)
    output = build_genesis_config(config, genesis_cfg)
    output["validators"] = validators
    
    # Write to validator_genesis.json in output_dir
    output_path = os.path.join(output_dir, "validator_genesis.json")
    with open(output_path, 'w') as f:
        json.dump(output, f, indent=2)
        
    print(f"[Aggregator] Successfully wrote {output_path}")
    print(f"[Aggregator] Configured {len(validators)} validators")

    # Extract and write faucet allocation
    faucet_cfg = genesis_cfg.get("faucet")
    faucet_alloc = {}
    
    if faucet_cfg:
        private_key = faucet_cfg.get("private_key")
        balance = faucet_cfg.get("balance")
        
        if private_key and balance:
            # Derive address from private key (simple approach: use last 40 chars of keccak hash)
            # For proper derivation, would need eth_account library
            # Here we use a placeholder - in production use: Account.from_key(private_key).address
            # For now, we'll include the private_key in the output for manual handling
            address = faucet_cfg.get("address")
            if not address:
                # If no address provided, we can't auto-derive without eth_account
                # Output the private_key for downstream tools to derive
                print(f"[Aggregator] Faucet private_key provided, balance: {balance}")
                print(f"[Aggregator] Note: Derive address externally or add 'address' field to genesis.toml")
            else:
                faucet_alloc[address] = {"balance": balance}
        
        # Also support explicit address/balance format
        if "address" in faucet_cfg and "balance" in faucet_cfg:
            faucet_alloc[faucet_cfg["address"]] = {"balance": faucet_cfg["balance"]}
                
    if faucet_alloc:
        faucet_alloc_path = os.path.join(output_dir, "faucet_alloc.json")
        with open(faucet_alloc_path, 'w') as f:
            json.dump(faucet_alloc, f, indent=2)
        print(f"[Aggregator] Exported faucet allocation ({len(faucet_alloc)} accounts) to {faucet_alloc_path}")

if __name__ == "__main__":
    main()

