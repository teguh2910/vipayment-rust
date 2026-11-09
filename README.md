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

## License

This project is licensed under the [MIT License](LICENSE).
