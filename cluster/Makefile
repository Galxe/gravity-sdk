.PHONY: init deploy start stop clean status restart

# Initialize artifacts (Keys, Genesis, Waypoint) into ./output
init:
	@bash init.sh

# Deploy artifacts to running directory (cleans old data)
deploy:
	@bash deploy.sh

# Start the cluster
start:
	@bash start.sh

# Run faucet initialization
faucet:
	@bash faucet.sh

# Stop the cluster
stop:
	@bash stop.sh

# Check cluster status
status:
	@bash status.sh

# Clean generated artifacts
clean:
	@rm -rf output
	@echo "Artifacts cleaned."

# Full redeploy cycle (Stop -> Deploy -> Start)
restart: stop deploy start

deploy_start: deploy start
