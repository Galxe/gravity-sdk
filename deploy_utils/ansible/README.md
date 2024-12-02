# README: Ansible Scaffolding for Reth Deployment and Management

This repository provides an **Ansible-based automation framework** for managing Reth-related services, including compilation, deployment, startup, and shutdown on multiple servers.

---

## Prerequisites

Before using this tool, you must install **Ansible** on your local machine. Follow the installation instructions below for your operating system.

### Installing Ansible

#### Ubuntu
1. Update your package manager:
   ```bash
   sudo apt update
   ```
2. Install Ansible:
   ```bash
   sudo apt install -y ansible
   ```
3. Verify installation:
   ```bash
   ansible --version
   ```

#### macOS
1. Install Ansible using Homebrew:
   ```bash
   brew install ansible
   ```
2. Verify installation:
   ```bash
   ansible --version
   ```

---

## Inventory File: `inventory.ini`

The inventory file defines the servers managed by Ansible. Below is an example configuration:

```ini
[reth_servers]
server1 ansible_host=${HOST} ansible_user=${USER}
```

### Adding New Servers

To add new servers, follow these rules:
1. Define the server under the `[reth_servers]` group.
2. Specify `ansible_host` (the server's IP or hostname).
3. Specify `ansible_user` (the SSH user for that server).

For example, to add another server:
```ini
[reth_servers]
server1 ansible_host=${HOST} ansible_user=${USER}
server2 ansible_host=${HOST} ansible_user=${USER}
```

### Server-Specific Variables

Each server can have its own variables. These are defined in `host_vars/<server_name>.yml`. For instance, for `server1`, create a file `host_vars/server1.yml` with contents like:

```yaml
reth_compile_options: "FEATURE=grevm"
reth_node_arg: "--mode cluster --node node4 --bin_version release"
reth_root_dir: "/home/${USER}/projects/reth"
reth_node_id: "node1"
reth_env: "EVM_DISABLE_GREVM=1"
```

#### Variable Descriptions

1. **`reth_compile_options`**  
   - **Definition:** Specifies the options used when compiling Reth.  
   - **Example Usage:** Options like `FEATURE=grevm` are passed to the `Makefile`.  
   - **Reference:** Consult the project's `Makefile` for all supported build options.

2. **`reth_node_arg`**  
   - **Definition:** Command-line arguments required during the deployment and startup of Reth.  
   - **Example Usage:** `--mode --cluster` might indicate a specific mode or configuration for the node.

3. **`reth_root_dir`**  
   - **Definition:** The root directory of the Reth project on the server.  
   - **Example Usage:** This is used as the working directory (`chdir`) for tasks like building and starting the Reth binary.

4. **`reth_node_id`**  
   - **Definition:** A unique identifier for the Reth node, used for tracking and configuration.  
   - **Example Usage:** Useful for cluster setups to differentiate between nodes.

5. **`reth_env`**  
   - **Definition:** A dictionary of environment variables required when running the Reth binary.  

This structured configuration allows each server to have its customized settings while maintaining a clear and organized approach to defining variables.

---

## Supported Features

### Compile, Deploy, and Start Reth

- To **compile, deploy, and start Reth on a single server** (e.g., `server1`):
  ```bash
  ansible-playbook -i inventory.ini reth_deploy.yml --limit server1
  ```
- To **compile, deploy, and start Reth on all servers**:
  ```bash
  ansible-playbook -i inventory.ini reth_deploy.yml
  ```

### Config reth node

**Attention**, reth currently doesn't support specify some config like `rpc.max_connection` using config file, you should directly specify it as command line args instead. So you could sepcify your customized conf in the `start.sh` and then ansible would copy this file to remote server.

### Stop Reth Service

- To **stop the Reth service on a single server** (e.g., `server1`):
  ```bash
  ansible-playbook -i inventory.ini reth_stop.yml --limit server1
  ```
- To **stop the Reth service on all servers**:
  ```bash
  ansible-playbook -i inventory.ini reth_stop.yml
  ```

---

## Notes

- Ensure that the Ansible control machine can SSH into all servers defined in the inventory file.
- Use SSH key-based authentication for smoother execution.
- Modify the playbooks as needed to fit your deployment requirements.

---

Enjoy effortless automation with Ansible! 🎉
