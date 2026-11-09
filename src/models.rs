//! Data models for the VIPayment API.

use crate::error::{Result, VipaymentError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Generic response wrapper returned by the VIPayment API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: serde::de::Deserialize<'de>"))]
pub struct ApiResponse<T> {
    /// Indicates whether the API call was successful (`true` or `false`).
    #[serde(default)]
    pub result: bool,

    /// Descriptive message or status from VIPayment.
    #[serde(default)]
    pub message: String,

    /// The response payload data, if available.
    #[serde(default)]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Returns `true` if the API indicated a successful result.
    pub fn is_success(&self) -> bool {
        self.result
    }

    /// Unwraps the response data, returning an `Err(VipaymentError::ApiError)`
    /// if `result` is `false`, or `Err(VipaymentError::MissingData)` if `data` is `None`.
    pub fn into_result(self) -> Result<T> {
        if !self.result {
            Err(VipaymentError::ApiError {
                message: self.message,
            })
        } else {
            self.data.ok_or(VipaymentError::MissingData)
        }
    }
}

/// Account profile information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProfileData {
    pub username: Option<String>,
    pub name: Option<String>,
    pub balance: Option<serde_json::Value>,
    pub point: Option<serde_json::Value>,
    pub level: Option<String>,
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Result of a prepaid order transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrepaidOrderData {
    pub trxid: Option<String>,
    pub data: Option<String>,
    pub code: Option<String>,
    pub service: Option<String>,
    pub status: Option<String>,
    pub note: Option<String>,
    pub balance: Option<serde_json::Value>,
    pub price: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Prepaid order status record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrepaidStatusItem {
    pub trxid: Option<String>,
    pub data: Option<String>,
    pub code: Option<String>,
    pub service: Option<String>,
    pub status: Option<String>,
    pub note: Option<String>,
    pub sn: Option<String>,
    pub price: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Available prepaid catalog service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrepaidServiceItem {
    pub code: Option<String>,
    pub name: Option<String>,
    pub price: Option<serde_json::Value>,
    pub status: Option<String>,
    pub brand: Option<String>,
    #[serde(rename = "type")]
    pub service_type: Option<String>,
    pub note: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Result of a game/streaming order transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameOrderData {
    pub trxid: Option<String>,
    pub data: Option<String>,
    pub zone: Option<String>,
    pub code: Option<String>,
    pub service: Option<String>,
    pub status: Option<String>,
    pub note: Option<String>,
    pub balance: Option<serde_json::Value>,
    pub price: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Game/streaming order status record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameStatusItem {
    pub trxid: Option<String>,
    pub data: Option<String>,
    pub zone: Option<String>,
    pub code: Option<String>,
    pub service: Option<String>,
    pub status: Option<String>,
    pub note: Option<String>,
    pub sn: Option<String>,
    pub price: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Available game catalog service item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameServiceItem {
    pub code: Option<String>,
    pub game: Option<String>,
    pub name: Option<String>,
    pub price: Option<serde_json::Value>,
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
