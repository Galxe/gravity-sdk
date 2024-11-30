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
server1 ansible_host=34.19.64.87 ansible_user=jingyue
```

### Adding New Servers

To add new servers, follow these rules:
1. Define the server under the `[reth_servers]` group.
2. Specify `ansible_host` (the server's IP or hostname).
3. Specify `ansible_user` (the SSH user for that server).

For example, to add another server:
```ini
[reth_servers]
server1 ansible_host=34.19.64.87 ansible_user=jingyue
server2 ansible_host=192.168.1.100 ansible_user=admin
```

### Server-Specific Variables

Each server can have its own variables. These are defined in `host_vars/<server_name>.yml`. For instance, for `server1`, create a file `host_vars/server1.yml` with contents like:

```yaml
reth_compile_options: "--release --features=grevm"
reth_node_arg: "--cluster"
reth_root_dir: "/home/jingyue/projects/reth"
```

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
