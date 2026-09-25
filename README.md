# nut-monitor-web

A lightweight web dashboard and alerting monitor for UPS (Uninterruptible Power Supply) devices. Written in **Rust** with the **Axum** framework, it polls your UPS via [Network UPS Tools](https://networkupstools.org/) (`upsc`), renders a responsive dark-themed live dashboard, and sends push notifications to mobile devices via **Firebase Cloud Messaging (FCM) v1 API** when critical thresholds are breached.

---

## Features

- 📊 **Live Dashboard** — Real-time view of UPS load, battery charge, voltage, runtime, and device metadata.
- 🔔 **Smart Alerting** — Push notifications sent to registered mobile devices when:
  - UPS status changes (e.g. Online → On Battery → Low Battery)
  - Battery charge drops below **50%**
  - UPS load exceeds **80%**
  - Remaining runtime falls below **15 minutes** (while on battery)
- 📱 **Device Registration** — Mobile clients register their FCM tokens via a REST API; tokens are stored in a local SQLite database.
- 📜 **Status History** — Every UPS status transition is persisted and queryable via the API.
- 🐳 **Multi-arch Docker Image** — CI/CD pipeline publishes `linux/amd64` and `linux/arm64` images to GHCR on every push.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust 2021 Edition |
| Web Framework | [Axum](https://github.com/tokio-rs/axum) 0.7 + Tokio 1.0 |
| Templates | [Askama](https://github.com/djc/askama) 0.13 (compiled at build time) |
| Database | SQLite via [`rusqlite`](https://github.com/rusqlite/rusqlite) 0.31 (bundled) |
| Push Notifications | Firebase Cloud Messaging v1 API (JWT/RS256 via `jsonwebtoken`) |
| HTTP Client | [`reqwest`](https://github.com/seanmonstar/reqwest) 0.12 with `rustls-tls` |
| Logging | `tracing` + `tracing-subscriber` + `tower-http` |
| UPS Interface | System `upsc` binary (NUT client) |

---

## API Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/` | Renders the live HTML dashboard |
| `GET` | `/api/status` | Returns raw UPS metrics as JSON |
| `GET` | `/api/status-history` | Returns UPS status transition history |
| `POST` | `/api/register` | Registers or updates a mobile device token |
| `GET` | `/api/devices` | Lists all registered devices |
| `DELETE` | `/api/devices/:id` | Removes a device by ID |
| `POST` | `/api/test-fcm` | Triggers a test push notification to all devices |

---

## Configuration

All configuration is done via environment variables:

| Variable | Default | Description |
|---|---|---|
| `UPS_NAME` | `ups` | NUT device name (e.g. `eaton`, `apc`) |
| `UPS_HOST` | `localhost` | Hostname/IP of the `upsd` server |
| `MONITOR_INTERVAL_SECS` | `10` | Polling interval in seconds |
| `DATABASE_PATH` | `/data/devices.db` | Path to the SQLite database file |
| `FCM_PROJECT_ID` | *(optional)* | Google Cloud Project ID |
| `FCM_CLIENT_EMAIL` | *(optional)* | Service Account client email |
| `FCM_PRIVATE_KEY` | *(optional)* | Service Account private key (literal `\n` formatting supported) |

> **Note:** If any `FCM_*` variable is absent, push notifications are silently disabled. The dashboard and API continue to work normally.

---

## Getting Started

### Prerequisites

- **Rust** stable toolchain
- **NUT client** (`upsc`) installed and reachable from the runtime environment
- A running `upsd` instance with a configured UPS device

### Run Locally

```bash
# Basic run (targets a UPS named "ups" on a remote host)
UPS_NAME=ups UPS_HOST=192.168.1.100 cargo run

# With FCM push notifications enabled
UPS_NAME=ups UPS_HOST=192.168.1.100 \
  FCM_PROJECT_ID=my-project \
  FCM_CLIENT_EMAIL=sa@my-project.iam.gserviceaccount.com \
  FCM_PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----\n..." \
  cargo run
```

The server listens on **http://0.0.0.0:3000** by default.

### Build Release Binary

```bash
cargo build --release
# Output: target/release/nut-monitor-web
```

### Run Tests

```bash
cargo test
```

---

## Docker

### Build and Run with Docker Compose

Edit `docker-compose.yml` to set your `UPS_NAME` and `UPS_HOST`, then:

```bash
docker compose up -d
```

The service will be available at **http://localhost:3000**.

### Build Image Manually

```bash
docker build -t nut-monitor-web:latest .
```

### Pull from GHCR

Pre-built multi-platform images (`linux/amd64`, `linux/arm64`) are published automatically on every push to `master` or on a semver tag release:

```bash
docker pull ghcr.io/banshee-lab/nut-monitor:latest
```

---

## Project Structure

```
.
├── src/
│   ├── main.rs         # Entrypoint: config, DB init, router, background task
│   ├── metrics.rs      # UPS metrics structs, upsc parser, watt calculation
│   ├── alerts.rs       # Threshold evaluation loop, FCM notification client
│   ├── dashboard.rs    # Askama template struct and HTML handler
│   └── web.rs          # REST API handlers (devices, status, test FCM)
├── templates/
│   └── template.html   # Dark-themed dashboard (compiled at build time)
├── Dockerfile          # Multi-stage build (builder + slim runtime)
├── docker-compose.yml
└── Cargo.toml
```

---

## Alert Thresholds

| Condition | Threshold | Behavior |
|---|---|---|
| Status change | Any transition | Notifies once per transition |
| Battery charge | < 50% | Notifies once; resets when charge recovers |
| UPS load | > 80% | Notifies once; resets when load drops |
| Runtime remaining | < 15 min (on battery only) | Notifies once; resets when back on AC |

---

## CI/CD

GitHub Actions (`.github/workflows/build.yml`) automatically:

1. Builds a multi-platform Docker image (`linux/amd64` + `linux/arm64`) using QEMU + Buildx.
2. Pushes tagged images to **GitHub Container Registry (GHCR)** on:
   - Every push to `master`
   - Semver release tags (e.g. `v0.2.0`)

---

## License

This project is open source. See the repository for license details.
