# Observability: Grafana & Prometheus Deployment

### Goal

This guide explains how to quickly deploy Grafana + Prometheus (with optional node-exporter) on a Linux server using Docker Compose, and import the standard Reth dashboard JSONs.

> If you already have a Prometheus instance (or another data source), you can skip deploying Prometheus and simply add it as a data source inside Grafana.

---

### Prerequisites

- A Linux machine (4 vCPU / 8 GB RAM or more recommended)
- Docker and Docker Compose v2 installed
- Network access from the monitoring machine to your scrape targets (e.g. metrics ports on validator nodes)

Open the following ports as needed:

- `3000` — Grafana web UI
- `9090` — Prometheus web UI (optional)

---

### Recommended Directory Layout

Create the following structure on your server:

```
/opt/monitoring/
├── docker-compose.yml
├── prometheus/
│   ├── prometheus.yml
│   └── rules/          # optional alerting rules
└── grafana/
    ├── provisioning/
    │   ├── datasources/
    │   └── dashboards/
    └── dashboards/     # place dashboard JSON files here
```

---

### Deployment: Docker Compose

#### 1) `docker-compose.yml`

Save the following as `/opt/monitoring/docker-compose.yml`:

```yaml
services:
  prometheus:
    image: prom/prometheus:v2.50.1
    container_name: prometheus
    restart: unless-stopped
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - ./prometheus/rules:/etc/prometheus/rules:ro
      - prometheus_data:/prometheus
    command:
      - --config.file=/etc/prometheus/prometheus.yml
      - --storage.tsdb.path=/prometheus
      - --web.enable-lifecycle
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana:10.4.2
    container_name: grafana
    restart: unless-stopped
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning
      - ./grafana/dashboards:/var/lib/grafana/dashboards
    ports:
      - "3000:3000"
    depends_on:
      - prometheus

  node-exporter:
    image: prom/node-exporter:v1.8.1
    container_name: node-exporter
    restart: unless-stopped
    pid: host
    network_mode: host
    command:
      - --path.rootfs=/host
    volumes:
      - /:/host:ro,rslave

volumes:
  prometheus_data:
  grafana_data:
```

> **Warning**: In production, change the Grafana admin password or put Grafana behind a reverse proxy with SSO.

#### 2) Prometheus Configuration — `prometheus.yml`

Create `/opt/monitoring/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - /etc/prometheus/rules/*.yml

scrape_configs:
  # Reth / chain node metrics — adjust IPs and ports to match your setup
  - job_name: "reth"
    metrics_path: /metrics
    static_configs:
      - targets:
          - "10.0.0.10:9001"
          - "10.0.0.11:9001"
```

Replace the `targets` entries with the actual `IP:port` of your testnet validator nodes.

#### 3) Start the Stack

From `/opt/monitoring/`, run:

```bash
docker compose up -d
```

Verify all containers are running:

```bash
docker compose ps
```

Access the UIs:

- Grafana: `http://<server-ip>:3000` (default credentials: `admin` / `admin`)
- Prometheus: `http://<server-ip>:9090`

---

### Grafana Provisioning (Auto-load Data Sources & Dashboards)

Provisioning lets you pre-configure Grafana so that data sources and dashboards are ready the moment the container starts.

#### 1) Prometheus Data Source

Create `/opt/monitoring/grafana/provisioning/datasources/datasource.yml`:

```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
```

#### 2) Dashboard Auto-loader

Create `/opt/monitoring/grafana/provisioning/dashboards/dashboard.yml`:

```yaml
apiVersion: 1

dashboardProviders:
  - name: "default"
    orgId: 1
    folder: ""
    type: file
    disableDeletion: false
    editable: true
    options:
      path: /var/lib/grafana/dashboards
```

Place dashboard JSON files in `/opt/monitoring/grafana/dashboards/`, then restart Grafana:

```bash
docker compose restart grafana
```

---

### Importing the Reth Dashboard

The official reth repository includes a set of pre-built Grafana dashboards:

- Dashboard JSONs: [https://github.com/paradigmxyz/reth/tree/main/etc/grafana/dashboards](https://github.com/paradigmxyz/reth/tree/main/etc/grafana/dashboards)

**Method A — Manual import:**

1. Open Grafana → **Dashboards** → **New** → **Import**
2. Paste the JSON content or upload the file
3. Select `Prometheus` as the data source

**Method B — Provisioning (recommended):**

1. Download the desired JSON files to `/opt/monitoring/grafana/dashboards/`
2. Restart Grafana: `docker compose restart grafana`

---

### Troubleshooting

| Symptom | What to check |
|---------|---------------|
| Can't reach Grafana UI | Confirm port `3000` is not blocked by a firewall or security group; check logs: `docker logs grafana --tail=200` |
| Prometheus targets show as Down | Go to **Prometheus → Status → Targets** for the error; verify connectivity: `curl http://<ip>:<port>/metrics`; if using container networking, prefer fixed IPs or host networking |
| Prometheus config changes not picked up | Hot-reload: `curl -X POST http://<server-ip>:9090/-/reload`, or `docker compose restart prometheus` |

---

### Security & Operations Tips

- Put Grafana behind an Nginx reverse proxy with HTTPS enabled.
- Restrict Prometheus and Grafana to internal network access only.
- Set up a backup strategy for the Prometheus data volume, or configure remote storage.
- Tune `scrape_interval` and data retention (`--storage.tsdb.retention.time`) based on your load-test intensity — high-frequency scraping increases Prometheus memory and CPU usage.
