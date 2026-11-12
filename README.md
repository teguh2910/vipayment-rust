# VIPayment Rust SDK

[![Crates.io](https://img.shields.io/badge/crates.io-v0.1.0-orange.svg)](https://crates.io/crates/vipayment)
[![Documentation](https://docs.rs/vipayment/badge.svg)](https://docs.rs/vipayment)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Official-style Rust SDK for [VIPayment / VIP Reseller](https://vip-reseller.co.id) API integration.

Provides both **asynchronous** (Tokio + Reqwest) and **synchronous (blocking)** clients for managing account balance, prepaid transactions (PLN, Pulsa, data), game top-ups, order statuses, and catalog services.

---

## Features

- ⚡ **Async-First**: Built with `reqwest` and `tokio` for high performance and low resource overhead.
- 🔒 **Type-Safe**: Complete typed models for all requests and responses, with resilient fallback (`ApiResponse<T>`).
- 🔐 **Automated Signatures**: MD5 signature (`md5(api_id + api_key)`) generated and attached automatically.
- 🛡️ **Blocking Support**: Optional `blocking` feature flag for non-async applications and simple scripts.
- 📦 **Zero External Server Needed for Testing**: Mock integration tests using `wiremock`.
- 💻 **Cross-Platform CLI**: Built-in `vipayment` CLI with automated multi-OS release builds (Linux, Windows, macOS).

---

## Pre-built Binaries

Pre-compiled standalone executables are automatically generated on every push for:
- **Windows (x64)**: `vipayment-x86_64-pc-windows-msvc.zip`
- **Linux (glibc / Ubuntu/Debian)**: `vipayment-x86_64-unknown-linux-gnu.tar.gz`
- **Linux (musl / Alpine/Docker)**: `vipayment-x86_64-unknown-linux-musl.tar.gz`
- **macOS Apple Silicon (M1/M2/M3/M4)**: `vipayment-aarch64-apple-darwin.tar.gz`
- **macOS Intel (x86_64)**: `vipayment-x86_64-apple-darwin.tar.gz`

Download the latest release package from [GitHub Releases](https://github.com/teguh2910/vipayment-rust/releases).

---

## Command-Line Interface (CLI)

The package includes a standalone CLI binary:

```bash
# Set credentials
export VIPAYMENT_API_ID="your_api_id"
export VIPAYMENT_API_KEY="your_api_key"

# Show help
vipayment help

# Check balance & profile
vipayment profile

# Query prepaid services catalog
vipayment prepaid-services type pulsa-reguler

# Order prepaid service
vipayment prepaid-order PLN5 1234567890

# Check order status
vipayment prepaid-status TRX123456789

# Query game catalog
vipayment game-services "Mobile Legends" available

# Order game top-up
vipayment game-order ML5 12345678 1234
```

---

## Installation

Add `vipayment` to your `Cargo.toml`:

```toml
[dependencies]
vipayment = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

### Synchronous (Blocking) Mode

If you do not use Tokio or async Rust, enable the `blocking` feature:

```toml
[dependencies]
vipayment = { version = "0.1.0", features = ["blocking"] }
```

---

## Quickstart

### 1. Initialization

```rust
use vipayment::VipaymentClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_id = "your_api_id";
    let api_key = "your_api_key";

    let client = VipaymentClient::new(api_id, api_key);

    // Or configure with custom options (timeout, proxy, base URL)
    // let client = VipaymentClient::builder(api_id, api_key)
    //     .timeout(std::time::Duration::from_secs(30))
    //     .build()?;

    Ok(())
}
```

### 2. Get Profile & Balance

```rust
let response = client.profile().await?;

if response.is_success() {
    let profile = response.into_result()?;
    println!("Username: {:?}", profile.username);
    println!("Balance: {:?}", profile.balance);
} else {
    eprintln!("Error: {}", response.message);
}
```

---

## Prepaid Services (Pulsa, PLN, Data)

### Query Prepaid Services Catalog

```rust
// Fetch all services
let all_services = client.service_prepaid(None, None).await?;

// Filter by type (e.g., 'pulsa-reguler')
let pulsa = client.service_prepaid(Some("type"), Some("pulsa-reguler")).await?;

if let Ok(services) = pulsa.into_result() {
    for service in services {
        println!("[{}] {:?} - {:?}", 
            service.code.as_deref().unwrap_or("-"),
            service.name, 
            service.price
        );
    }
}
```

### Order Prepaid Service

```rust
let order = client.order_prepaid("PLN5", "1234567890").await?;

if order.is_success() {
    let data = order.into_result()?;
    println!("Order Placed! Transaction ID: {:?}", data.trxid);
} else {
    eprintln!("Order failed: {}", order.message);
}
```

### Check Prepaid Order Status

```rust
let status = client.status_order_prepaid("TRX123456789", None).await?;

if let Ok(records) = status.into_result() {
    for record in records {
        println!("Status: {:?}, SN: {:?}", record.status, record.sn);
    }
}
```

---

## Game & Streaming Top-up

### Query Game Catalog

```rust
// Filter by game and availability
let games = client.service_game(Some("Mobile Legends"), Some("available")).await?;

if let Ok(items) = games.into_result() {
    for item in items {
        println!("[{}] {:?} - {:?}", item.code, item.name, item.price);
    }
}
```

### Order Game Top-up

```rust
// service_code, user_id, zone_id (optional)
let order = client.order_game("ML5", "12345678", Some("1234")).await?;

if order.is_success() {
    let data = order.into_result()?;
    println!("Game Order Placed! TrxID: {:?}", data.trxid);
}
```

### Check Game Order Status

```rust
let status = client.status_order_game("TRX987654321", None).await?;

if let Ok(records) = status.into_result() {
    for record in records {
        println!("Game Status: {:?}, SN: {:?}", record.status, record.sn);
    }
}
```

---

## Synchronous (Blocking) Usage

```rust
#[cfg(feature = "blocking")]
use vipayment::blocking::BlockingVipaymentClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = BlockingVipaymentClient::new("your_api_id", "your_api_key");

    let profile = client.profile()?;
    println!("Profile: {:?}", profile.data);

    let services = client.service_prepaid(Some("type"), Some("pulsa-reguler"))?;
    println!("Services: {:?}", services.data);

    Ok(())
}
```

---

## Running Examples

Set your credentials as environment variables (optional):

```bash
export VIPAYMENT_API_ID="your_api_id"
export VIPAYMENT_API_KEY="your_api_key"
```

Then run any of the provided examples:

```bash
cargo run --example 01_profile
cargo run --example 02_prepaid
cargo run --example 03_game
```

---

## Running Tests

Run the full automated test suite with WireMock:

```bash
cargo test --all-features
```

---

## Web Top-Up App (`apps/api` + `web`)

This repository also ships a self-hosted web top-up app built on top of the SDK:

```
web/ (Next.js)  ──►  apps/api/ (axum + SQLite)  ──►  VIP Reseller API
                                                     (via the `vipayment` SDK)
```

### What it does

- Syncs the full provider catalog into SQLite every 30 minutes:
  - `/game-feature` groups (games, apps, streaming, vouchers) — 98 groups;
  - `/prepaid` groups (pulsa, paket data, telepon, PLN, e-wallet, TV, voucher) grouped by type + brand.
  `MARKUP_PERCENT` is applied to every product. Provider prices arrive as tier objects (`{"basic": ..., "premium": ..., "special": ...}`); the `basic` tier is used as the cost basis.
- Guest checkout for every category: pick a product, fill the target (game ID + zone, phone number, or meter number), receive an invoice.
- Pluggable payment adapter:
  - `manual` — static QRIS / bank transfer, customer uploads proof, admin approves it.
  - `simulation` — payment is auto-confirmed after a few seconds (local testing, no real money).
- Once payment is confirmed, a background worker submits the order to the right provider endpoint (`/game-feature` for game/app groups, `/prepaid` for prepaid groups), polls its status, and stores the serial number.
- Invoice tracking page, admin review panel, and a status timeline.
- Group logos are resolved from the official game/brand website (Google favicon service), downloaded once during sync, and served locally from `/logos/...` — nothing is hotlinked from a third party. Groups without a mapped domain fall back to an initials tile.

The provider API key never reaches the browser, and order prices are always read from the database (never from the client).

### Quick start

```bash
# 1. Configure the API (the provided .env is gitignored)
cp apps/api/.env.example apps/api/.env
# then edit apps/api/.env: provider credentials, ADMIN_TOKEN, payment mode

# 2. Run the API from apps/api (paths are relative to that directory)
cd apps/api
cargo run

# 3. Run the web app
cd ../web
npm install
npm run dev
# open http://localhost:3000  (admin: http://localhost:3000/admin)
```

> **IP allowlist:** VIP Reseller only accepts requests from allowlisted IPs. Add
> your server's public IP in the VIP Reseller dashboard first. Otherwise catalog
> sync and orders fail with `IP ... tidak diizinkan` (visible in
> `GET /api/v1/health` as `last_sync_error`).

### Docker

The repository ships a Compose setup with two services:

| File | Purpose |
| --- | --- |
| `apps/api/Dockerfile` | Multi-stage Rust build; runs as a non-root user on `debian:bookworm-slim` |
| `web/Dockerfile` | Next.js standalone build; runs as a non-root user on `node:24-alpine` |
| `docker-compose.yml` | `api` on `:8080`, `web` on `:3000`, named volume `api-data` |

```bash
# 1. API environment (provider credentials, ADMIN_TOKEN, payment mode)
cp apps/api/.env.example apps/api/.env

# 2. Build and start
docker compose up --build -d

# 3. Open
# storefront  http://localhost:3000
# admin       http://localhost:3000/admin
# api health  http://localhost:8080/api/v1/health
```

The API container listens on `0.0.0.0:8080` (Compose overrides `BIND_ADDR`, which
is `127.0.0.1` for local `cargo run`). SQLite data and uploaded payment proofs
are stored in the `api-data` volume:

```bash
docker compose logs -f api web   # follow logs
docker compose ps                # service status
docker compose down              # stop (keeps the volume)
docker compose down -v           # stop and delete database + proofs
```

`NEXT_PUBLIC_API_URL` is inlined into the browser bundle at build time, so it
must be the URL the **browser** can reach (not a container hostname). Override it
when building for a real domain:

```bash
NEXT_PUBLIC_API_URL=https://api.example.com docker compose build web
NEXT_PUBLIC_API_URL=https://api.example.com docker compose up -d
```

For a public deployment also set `WEB_ORIGIN` in `apps/api/.env` to the public
storefront URL (CORS allowlist), set a strong `ADMIN_TOKEN`, switch to
`PAYMENT_PROVIDER=manual` with real QRIS/bank accounts, and make sure the
provider allowlist includes the server's public IP.

### Configuration (`apps/api/.env`)

| Variable | Default | Notes |
| --- | --- | --- |
| `PROVIDER_VIPRESELLER_ENABLED` | `true` | `false` rejects new orders |
| `PROVIDER_VIPRESELLER_API_ID` / `_API_KEY` | required | falls back to `VIPAYMENT_API_ID` / `VIPAYMENT_API_KEY` |
| `PROVIDER_VIPRESELLER_SIGN` | optional | checked against `md5(API_ID + API_KEY)`; the SDK recomputes it |
| `PROVIDER_VIPRESELLER_BASE_URL` | `https://vip-reseller.co.id/api` | point at a mock for testing |
| `PROVIDER_TIMEOUT_SECONDS` | `20` | provider request timeout |
| `PROVIDER_FORCE_IPV4` | `true` | bind outbound requests to IPv4 (use when only the IPv4 is allowlisted) |
| `DATABASE_URL` | `sqlite://data/app.db` | SQLite file (created automatically) |
| `BIND_ADDR` | `127.0.0.1:8080` | API listen address |
| `WEB_ORIGIN` | `http://localhost:3000` | CORS allowlist |
| `ADMIN_TOKEN` | required | `Authorization: Bearer <token>` for `/api/v1/admin/*` |
| `PAYMENT_PROVIDER` | `manual` | `manual` or `simulation` |
| `PAYMENT_EXPIRY_MINUTES` | `1440` | invoice payment window |
| `MARKUP_PERCENT` | `5` | `sell = ceil(base * (100 + p) / 100)` |
| `SIMULATION_AUTO_PAY_SECONDS` | `5` | simulation mode only |
| `MANUAL_QRIS_IMAGE_URL` | empty | optional QRIS image URL |
| `MANUAL_BANK_ACCOUNTS` | empty | JSON array, single-quoted in `.env` |
| `GAMES_CONFIG_PATH` | `config/games.json` | optional game metadata overrides |
| `LOGOS_CONFIG_PATH` | `config/logos.json` | group/brand → official domain mapping |
| `LOGO_DIR` | `data/logos` | cached logo files served at `/logos` |

`config/games.json` is **optional metadata overrides** for game groups (display
name, slug, icon, zone requirement, input labels, sort order). All groups are
discovered from the provider; a game without an override gets a generated slug,
and any game group that is not listed there still accepts an optional
zone/server field. Provider group names removed from the response are
deactivated on the next sync (their historical orders stay intact).

`config/logos.json` maps a provider group name (or prepaid brand) to its official
website domain, for example `"Mobile Legends B": "mobilelegends.com"`. During a
sync the API downloads the site icon once, caches it under `LOGO_DIR`
(`data/logos`), and serves it from `GET /logos/{slug}.png`. Groups without a
mapping keep the initials tile. To refresh a logo, edit the mapping and delete
the cached file, then run `POST /api/v1/admin/sync-products`.

### API surface

Public (`/api/v1`): `GET /health`, `GET /games`, `GET /games/{slug}/products`,
`POST /orders`, `GET /orders/{invoice}`, `POST /orders/{invoice}/payment-proof`.

Admin (Bearer `ADMIN_TOKEN`): `GET /admin/payments`, `POST /admin/payments/{invoice}/approve`,
`POST /admin/payments/{invoice}/reject`, `GET /admin/orders`, `POST /admin/orders/{invoice}/retry`,
`POST /admin/orders/{invoice}/refunded`, `POST /admin/sync-products`, `GET /admin/profile`.

Order lifecycle: `pending_payment → paid → submitting → processing → success`
(with `failed` / `refund_pending` / `refunded` / `expired` / `needs_attention` branches).

### Testing

```bash
cargo test --all-features          # SDK
cargo test -p vipayment-api        # API unit + wiremock order-flow tests
cargo clippy -p vipayment-api --all-targets -- -D warnings
cd web && npm run build            # typecheck + production build
```

---

## License

This project is licensed under the [MIT License](LICENSE).
