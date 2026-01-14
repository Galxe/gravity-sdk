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
    
    validator_addresses = []
    consensus_keys = []
    voting_powers = []
    validator_network_addrs = []
    fullnode_network_addrs = []
    aptos_addresses = []

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
        # account_address is 32 bytes (64 hex chars)
        # See gravity_cli key.rs: hex::encode(&account_address.as_slice()[12..])
        val_addr = f"0x{raw_addr[-40:]}"
        aptos_addr = raw_addr
            
        consensus_pk = identity['consensus_public_key']
        if consensus_pk.startswith('0x'):
            consensus_pk = consensus_pk[2:]
            
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
        
        # Append
        validator_addresses.append(val_addr)
        consensus_keys.append(consensus_pk)
        voting_powers.append("20000")
        validator_network_addrs.append(val_net_addr)
        fullnode_network_addrs.append(vfn_net_addr)
        aptos_addresses.append(aptos_addr)

    # Construct final JSON
    output = {
        "validatorAddresses": validator_addresses,
        "consensusPublicKeys": consensus_keys,
        "votingPowers": voting_powers,
        "validatorNetworkAddresses": validator_network_addrs,
        "fullnodeNetworkAddresses": fullnode_network_addrs,
        "aptosAddresses": aptos_addresses
    }
    
    # Write to validator_genesis.json in base_dir
    output_path = os.path.join(base_dir, "validator_genesis.json")
    with open(output_path, 'w') as f:
        json.dump(output, f, indent=2)
        
    print(f"[Aggregator] Successfully wrote {output_path}")

if __name__ == "__main__":
    main()
