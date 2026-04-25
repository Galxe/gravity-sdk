.PHONY: genesis init deploy start stop clean status restart faucet deploy_start inject_gcp_identity

# Generate genesis.json and waypoint.txt (only for new networks)
genesis:
	@bash genesis.sh

# Generate node identity keys
init:
	@bash init.sh

# Upload identity.yaml to GCP Secret Manager for nodes whose
# cluster.toml has identity.source = "gcp_secret".
# Pass flags via ARGS, e.g.:
#   make inject_gcp_identity ARGS="--dry-run"
#   make inject_gcp_identity ARGS="--update --nodes vfn1,vfn2"
inject_gcp_identity:
	@bash inject_gcp_identity.sh $(ARGS)

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
