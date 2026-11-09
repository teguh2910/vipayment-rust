//! # VIPayment Rust SDK
//!
//! Official-style Rust SDK for the [VIPayment / VIP Reseller](https://vip-reseller.co.id) API.
//!
//! Provides asynchronous and synchronous clients for:
//! - Account profile inquiry & balance checking
//! - Prepaid orders (PLN tokens, Pulsa, data packages, vouchers)
//! - Prepaid order status check & catalog services
//! - Game & streaming orders (Mobile Legends, Free Fire, Genshin, etc.)
//! - Game order status check & game catalog services
//!
//! ## Quickstart
//!
//! Add `vipayment` and `tokio` to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! vipayment = "0.1.0"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! ### Asynchronous Example
//!
//! ```no_run
//! use vipayment::VipaymentClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = VipaymentClient::new("your_api_id", "your_api_key");
//!
//!     let profile = client.profile().await?;
//!     if profile.is_success() {
//!         println!("Profile: {:?}", profile.data);
//!     } else {
//!         eprintln!("Failed: {}", profile.message);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Synchronous (Blocking) Example
//!
//! Enable the `blocking` feature in `Cargo.toml`:
//! ```toml
//! [dependencies]
//! vipayment = { version = "0.1.0", features = ["blocking"] }
//! ```
//!
//! ```ignore
//! use vipayment::blocking::BlockingVipaymentClient;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = BlockingVipaymentClient::new("your_api_id", "your_api_key");
//!     let profile = client.profile()?;
//!     println!("Profile: {:?}", profile);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod models;

#[cfg(feature = "blocking")]
pub mod blocking;

// Re-export primary types
pub use client::{generate_signature, VipaymentClient, VipaymentClientBuilder};
pub use error::{Result, VipaymentError};
pub use models::*;

#[cfg(feature = "blocking")]
pub use blocking::{BlockingVipaymentClient, BlockingVipaymentClientBuilder};
