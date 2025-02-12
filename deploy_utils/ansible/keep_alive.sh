#!/bin/bash

if [ "$#" -lt 3 ]; then
  echo "Usage: $0 node=<node_name> host=<host_address> port=<port_number>"
  exit 1
fi

for arg in "$@"; do
  case $arg in
    node=*)
      node="${arg#*=}"
      ;;
    host=*)
      host="${arg#*=}"
      ;;
    port=*)
      port="${arg#*=}"
      ;;
    *)
      echo "Unknown parameter: $arg"
      echo "Usage: $0 node=<node_name> host=<host_address> port=<port_number>"
      exit 1
      ;;
  esac
done

if [ -z "$node" ] || [ -z "$host" ] || [ -z "$port" ]; then
  echo "Error: All parameters (node, host, port) must be provided."
  exit 1
fi

while true; do
  echo "Checking metrics at http://${host}:${port}/metrics..."
  
  response=$(curl -s --max-time 10 "http://${host}:${port}/metrics")
  if [ $? -ne 0 ] || [ -z "$response" ]; then
    echo "Metrics not available or request timed out. Restarting service for node $node..."
    
    ansible-playbook -i inventory.ini reth_start.yml --limit "$node"
    if [ $? -eq 0 ]; then
      echo "Service restarted successfully for node $node."
    else
      echo "Failed to restart service for node $node."
    fi
  else
    echo "Metrics check passed."
  fi

  sleep 60
done
