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
            "callbacks": ["0x00000000000000000000000000000001625F2018"]
        },
        "jwkConfig": {
            "issuers": ["0x68747470733a2f2f6163636f756e74732e676f6f676c652e636f6d"],
            "jwks": [[{
                "kid": "f5f4c0ae6e6090a65ab0a694d6ba6f19d5d0b4e6",
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
        print("Usage: aggregate_genesis.py <cluster_config_json_string>")
        sys.exit(1)

    # Read config from JSON string argument
    config_json_str = sys.argv[1]
    try:
        config = json.loads(config_json_str)
    except json.JSONDecodeError as e:
        print(f"Error parsing config JSON: {e}")
        sys.exit(1)
        
    base_dir = config['cluster']['base_dir']
    genesis_cfg = config.get('genesis', {})
    
    validators = []
    nodes = config['nodes']
    print(f"[Aggregator] Processing {len(nodes)} nodes for genesis configuration...")

    for node in nodes:
        node_id = node['id']
        data_dir = node.get('data_dir') or os.path.join(base_dir, node_id)
        identity_path = os.path.join(data_dir, "config", "validator-identity.yaml")
        
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
        
        # Extract fields
        account_addr = identity['account_address']
        if account_addr.startswith('0x'):
            raw_addr = account_addr[2:]
        else:
            raw_addr = account_addr
            
        # validatorAddresses: ETH-style, last 20 bytes (40 hex chars)
        val_addr = f"0x{raw_addr[-40:]}"
            
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
        
        # Create validator entry in new format
        validator = {
            "operator": val_addr,
            "owner": val_addr,
            "stakeAmount": "2000000000000000000",
            "moniker": f"validator-{len(validators) + 1}",
            "consensusPubkey": consensus_pk,
            "consensusPop": "0x",
            "networkAddresses": val_net_addr,
            "fullnodeAddresses": vfn_net_addr,
            "votingPower": "2000000000000000000"
        }
        validators.append(validator)

    # Build parallel arrays for gravity_cli compatibility
    # TestConfig struct requirements:
    # validatorAddresses (Vec<String>)
    # consensusPublicKeys (Vec<String>) - hex decoded, so no 0x
    # votingPowers (Vec<String>)
    # validatorNetworkAddresses (Vec<String>)
    # fullnodeNetworkAddresses (Vec<String>)
    # aptosAddresses (Vec<String>) - prepended with 0x in rust, so no 0x here

    validator_addresses = []
    consensus_public_keys = []
    voting_powers = []
    validator_network_addresses = []
    fullnode_network_addresses = []
    aptos_addresses = []

    for v in validators:
        validator_addresses.append(v["operator"]) # 0x...
        
        # Strip 0x for hex::decode
        cpk = v["consensusPubkey"]
        if cpk.startswith("0x"):
            cpk = cpk[2:]
        consensus_public_keys.append(cpk)
        
        voting_powers.append(v["votingPower"])
        validator_network_addresses.append(v["networkAddresses"])
        fullnode_network_addresses.append(v["fullnodeAddresses"])
        
        # operator is 0x..., aptosAddresses needs raw hex (rust prepends 0x)
        aptos_addr = v["operator"]
        if aptos_addr.startswith("0x"):
            aptos_addr = aptos_addr[2:]
        aptos_addresses.append(aptos_addr)

    # Build complete genesis config
    output = build_genesis_config(config, genesis_cfg)
    output["validators"] = validators
    
    # Add legacy fields
    output["validatorAddresses"] = validator_addresses
    output["consensusPublicKeys"] = consensus_public_keys
    output["votingPowers"] = voting_powers
    output["validatorNetworkAddresses"] = validator_network_addresses
    output["fullnodeNetworkAddresses"] = fullnode_network_addresses
    output["aptosAddresses"] = aptos_addresses
    
    # Write to validator_genesis.json in base_dir
    output_path = os.path.join(base_dir, "validator_genesis.json")
    with open(output_path, 'w') as f:
        json.dump(output, f, indent=2)
        
    print(f"[Aggregator] Successfully wrote {output_path}")
    print(f"[Aggregator] Configured {len(validators)} validators")

if __name__ == "__main__":
    main()
