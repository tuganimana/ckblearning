use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::config::fiber_client::{fiber_rpc_call, object_params, to_hex_u128_opt};

#[derive(Deserialize, ToSchema)]
pub struct SendFiberPaymentRequest {
    /// Encoded Fiber invoice to pay (`fibt…` / `fibb…` / `fibd…`).
    #[serde(default)]
    pub invoice: Option<String>,
    /// Target node pubkey (hex). Required for keysend / non-invoice payments.
    #[serde(default)]
    pub target_pubkey: Option<String>,
    /// Amount in shannons when not encoded in the invoice. Decimal or `0x` hex.
    #[serde(default)]
    pub amount: Option<String>,
    /// Payment hash when not taken from the invoice.
    #[serde(default)]
    pub payment_hash: Option<String>,
    /// Payment timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Max fee in shannons. Decimal or `0x` hex.
    #[serde(default)]
    pub max_fee_amount: Option<String>,
    /// Dry-run: build route and estimate fee without sending.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Keysend payment (no invoice).
    #[serde(default)]
    pub keysend: Option<bool>,
    /// Allow paying yourself through a circular route (rebalancing).
    #[serde(default)]
    pub allow_self_payment: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct SendFiberPaymentResponse {
    pub payment_hash: String,
    /// Created | Inflight | Success | Failed
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub raw: Option<Value>,
}

#[derive(Deserialize, ToSchema)]
pub struct GetFiberPaymentRequest {
    pub payment_hash: String,
}

/// Pays a Fiber invoice via the connected Fiber node (`send_payment`).
///
/// Typical CKB→Lightning flow after `/fiber/swap/ckb-to-lightning`:
/// pay the returned Fiber `incoming_invoice` with this endpoint; the CCH
/// settles the Lightning side atomically.
#[utoipa::path(
    post,
    path = "/fiber/payment/send",
    request_body = SendFiberPaymentRequest,
    responses(
        (status = 200, description = "Payment initiated", body = SendFiberPaymentResponse),
        (status = 400, description = "Invalid parameters"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn send_payment(
    Json(payload): Json<SendFiberPaymentRequest>,
) -> Result<Json<SendFiberPaymentResponse>, (StatusCode, String)> {
    let invoice = payload
        .invoice
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let target_pubkey = payload
        .target_pubkey
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if invoice.is_none() && target_pubkey.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "provide invoice and/or target_pubkey".to_string(),
        ));
    }

    let mut params = json!({});
    let obj = params.as_object_mut().expect("object");

    if let Some(invoice) = invoice {
        obj.insert("invoice".into(), json!(invoice));
    }
    if let Some(target_pubkey) = target_pubkey {
        obj.insert("target_pubkey".into(), json!(target_pubkey));
    }
    if let Some(amount) = to_hex_u128_opt(&payload.amount)? {
        obj.insert("amount".into(), json!(amount));
    }
    if let Some(payment_hash) = payload.payment_hash.filter(|h| !h.trim().is_empty()) {
        obj.insert("payment_hash".into(), json!(payment_hash));
    }
    if let Some(timeout) = payload.timeout {
        obj.insert("timeout".into(), json!(format!("0x{timeout:x}")));
    }
    if let Some(max_fee_amount) = to_hex_u128_opt(&payload.max_fee_amount)? {
        obj.insert("max_fee_amount".into(), json!(max_fee_amount));
    }
    if let Some(dry_run) = payload.dry_run {
        obj.insert("dry_run".into(), json!(dry_run));
    }
    if let Some(keysend) = payload.keysend {
        obj.insert("keysend".into(), json!(keysend));
    }
    if let Some(allow_self_payment) = payload.allow_self_payment {
        obj.insert("allow_self_payment".into(), json!(allow_self_payment));
    }

    let result: Value = fiber_rpc_call("send_payment", object_params(params)).await?;
    Ok(Json(map_payment_response(result)))
}

/// Retrieves Fiber payment status (`get_payment`).
#[utoipa::path(
    post,
    path = "/fiber/payment/get",
    request_body = GetFiberPaymentRequest,
    responses(
        (status = 200, description = "Payment status", body = SendFiberPaymentResponse),
        (status = 400, description = "Invalid payment hash"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn get_payment(
    Json(payload): Json<GetFiberPaymentRequest>,
) -> Result<Json<SendFiberPaymentResponse>, (StatusCode, String)> {
    let payment_hash = payload.payment_hash.trim();
    if payment_hash.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "payment_hash is required".to_string(),
        ));
    }

    let result: Value = fiber_rpc_call(
        "get_payment",
        object_params(json!({ "payment_hash": payment_hash })),
    )
    .await?;

    Ok(Json(map_payment_response(result)))
}

fn map_payment_response(result: Value) -> SendFiberPaymentResponse {
    SendFiberPaymentResponse {
        payment_hash: result
            .get("payment_hash")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        status: result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        failed_error: result
            .get("failed_error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        fee: result.get("fee").map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }),
        raw: Some(result),
    }
}
