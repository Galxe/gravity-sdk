.PHONY: genesis init deploy start stop clean status restart faucet deploy_start

# Generate genesis.json and waypoint.txt (only for new networks)
genesis:
	@bash genesis.sh

# Generate node identity keys (per-node, per cluster.toml's identity.source).
# - source = file       → writes identity.yaml + identity.public.yaml
# - source = gcp_secret → pushes identity to Secret Manager (zero disk),
#                          writes identity.public.yaml only
init:
	@bash init.sh

# Deploy artifacts to running directory (cleans old data)
deploy:
	@bash deploy.sh

# Start the cluster
start:
	@bash start.sh

# Stop the cluster
stop:
	@bash stop.sh

# Check cluster status
status:
	@bash status.sh

# Run faucet initialization
faucet:
	@bash faucet.sh

# Clean generated artifacts
clean:
	@rm -rf output
	@echo "Artifacts cleaned."

# Full redeploy cycle (Stop -> Deploy -> Start)
restart: stop deploy start

# Convenience: deploy and start
deploy_start: deploy start
