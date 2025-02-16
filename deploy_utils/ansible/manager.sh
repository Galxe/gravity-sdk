#!/bin/bash

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 action=<deploy|stop> node=<node1,node2,...>"
    exit 1
fi

for arg in "$@"; do
    case $arg in
    action=*)
        action="${arg#*=}"
        ;;
    node=*)
        node="${arg#*=}"
        ;;
    *)
        echo "Unknown parameter: $arg"
        echo "Usage: $0 action=<deploy|stop> node=<node1,node2,...>"
        exit 1
        ;;
    esac
done

if [[ "$action" != "deploy" && "$action" != "stop" ]]; then
    echo "Error: Invalid action. Allowed values are 'deploy' or 'stop'."
    exit 1
fi

if [ -z "$node" ]; then
    echo "Error: node parameter cannot be empty."
    exit 1
fi

if [ "$action" == "deploy" ]; then
    playbook="reth_deploy.yml"
elif [ "$action" == "stop" ]; then
    pids=$(ps -ef | grep "keep_alive" | grep "$node" | grep -v "grep" | awk '{print $2}')
    for pid in $pids; do
        echo "Found keep_alive processes for node $node: $pid"
        kill "$pid"
        if [ $? -eq 0 ]; then
            echo "Process $pid killed successfully."
        else
            echo "Failed to kill process $pid."
        fi
    done
    playbook="reth_stop.yml"
    ansible-playbook -i inventory.ini "$playbook" --limit "$node"
    exit
fi

echo "Executing ansible-playbook with action=$action on nodes=$node..."
ansible-playbook -i inventory.ini "$playbook" --limit "$node"

if [ $? -eq 0 ]; then
    echo "Ansible playbook executed successfully."
else
    echo "Ansible playbook execution failed."
    exit 1
fi

if [ "$action" == "deploy" ]; then
    echo "Starting gravity_node on nodes=$node..."
    ansible-playbook -i inventory.ini reth_start.yml --limit "$node"

    if [ $? -eq 0 ]; then
        echo "Ansible playbook executed successfully."
    else
        echo "Ansible playbook execution failed."
        exit 1
    fi
fi

validator_file="./server_confs/${node}/genesis/validator.yaml"
inventory_file="./inventory.ini"

if [ ! -f "$validator_file" ]; then
    echo "Error: Validator file not found: $validator_file"
    exit 1
fi

port=$(grep -A 1 "inspection_service:" "$validator_file" | grep "port:" | awk '{print $2}')

if [ -z "$port" ]; then
    echo "Error: 'port' not found or 'inspection_service' not configured in $validator_file"
    exit 1
fi

echo "Port for inspection_service: $port"

if [ ! -f "$inventory_file" ]; then
    echo "Error: Inventory file not found: $inventory_file"
    exit 1
fi

ansible_host=$(grep -E "^${node}\s" "$inventory_file" | grep -oP "ansible_host=\K\S+")

if [ -z "$ansible_host" ]; then
    echo "Error: 'ansible_host' not configured for node '${node}' in $inventory_file"
    exit 1
fi

nohup bash keep_alive.sh node=${node} host=${ansible_host} port=${port} >/dev/null 2>&1 &
