use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::config::fiber_client::{
    fiber_currency, fiber_rpc_call, object_params, to_hex_u128, to_hex_u128_opt,
};

#[derive(Deserialize, ToSchema)]
pub struct NewFiberInvoiceRequest {
    /// Invoice amount in shannons (1 CKB = 10^8 shannons). Decimal or `0x` hex.
    pub amount: String,
    /// Human-readable description (max 639 chars).
    #[serde(default)]
    pub description: Option<String>,
    /// Fiber currency. Defaults from `CKB_NETWORK`: Fibb/Fibt/Fibd.
    #[serde(default)]
    pub currency: Option<String>,
    /// Validity period in seconds. Decimal or `0x` hex.
    #[serde(default)]
    pub expiry: Option<String>,
    /// Optional CKB on-chain fallback address.
    #[serde(default)]
    pub fallback_address: Option<String>,
    /// Hash algorithm: `ckb_hash` (default) or `sha256`.
    /// Use `sha256` for Lightning cross-chain swaps.
    #[serde(default)]
    pub hash_algorithm: Option<String>,
    /// Payment preimage (`0x` + 64 hex). Mutually exclusive with `payment_hash`.
    #[serde(default)]
    pub payment_preimage: Option<String>,
    /// Payment hash only — creates a hold invoice. Mutually exclusive with `payment_preimage`.
    #[serde(default)]
    pub payment_hash: Option<String>,
    /// Final TLC expiry delta in milliseconds. Decimal or `0x` hex.
    #[serde(default)]
    pub final_expiry_delta: Option<String>,
    /// Enable multi-part payments.
    #[serde(default)]
    pub allow_mpp: Option<bool>,
    /// Enable trampoline routing.
    #[serde(default)]
    pub allow_trampoline_routing: Option<bool>,
    /// Optional UDT type script for non-CKB assets (raw CKB Script JSON).
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub udt_type_script: Option<Value>,
}

#[derive(Serialize, ToSchema)]
pub struct NewFiberInvoiceResponse {
    /// Bech32m-encoded Fiber invoice (`fibb…` / `fibt…` / `fibd…`).
    pub invoice_address: String,
    /// Parsed invoice object from Fiber.
    #[schema(value_type = Object)]
    pub invoice: Value,
}

#[derive(Deserialize, ToSchema)]
pub struct ParseFiberInvoiceRequest {
    /// Encoded Fiber invoice string.
    pub invoice: String,
}

#[derive(Serialize, ToSchema)]
pub struct ParseFiberInvoiceResponse {
    #[schema(value_type = Object)]
    pub invoice: Value,
}

#[derive(Deserialize, ToSchema)]
pub struct GetFiberInvoiceRequest {
    /// Payment hash (`0x` + 64 hex).
    pub payment_hash: String,
}

#[derive(Serialize, ToSchema)]
pub struct GetFiberInvoiceResponse {
    pub invoice_address: String,
    #[schema(value_type = Object)]
    pub invoice: Value,
    /// Open | Received | Paid | Cancelled | Expired
    pub status: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CancelFiberInvoiceRequest {
    pub payment_hash: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SettleFiberInvoiceRequest {
    pub payment_hash: String,
    pub payment_preimage: String,
}

#[derive(Serialize, ToSchema)]
pub struct SettleFiberInvoiceResponse {
    pub settled: bool,
}

/// Creates a Fiber Network invoice (`new_invoice`).
/// Share `invoice_address` with the payer; they settle it via `/fiber/payment/send`.
#[utoipa::path(
    post,
    path = "/fiber/invoice/new",
    request_body = NewFiberInvoiceRequest,
    responses(
        (status = 200, description = "Fiber invoice created", body = NewFiberInvoiceResponse),
        (status = 400, description = "Invalid parameters"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn new_invoice(
    Json(payload): Json<NewFiberInvoiceRequest>,
) -> Result<Json<NewFiberInvoiceResponse>, (StatusCode, String)> {
    let amount = to_hex_u128(&payload.amount)?;
    let currency = payload
        .currency
        .clone()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(fiber_currency);

    let mut params = json!({
        "amount": amount,
        "currency": currency,
    });

    let obj = params.as_object_mut().expect("object");

    if let Some(description) = payload.description.filter(|d| !d.is_empty()) {
        if description.len() > 639 {
            return Err((
                StatusCode::BAD_REQUEST,
                "description must be at most 639 characters".to_string(),
            ));
        }
        obj.insert("description".into(), json!(description));
    }
    if let Some(expiry) = to_hex_u128_opt(&payload.expiry)? {
        obj.insert("expiry".into(), json!(expiry));
    }
    if let Some(fallback_address) = payload.fallback_address.filter(|a| !a.is_empty()) {
        obj.insert("fallback_address".into(), json!(fallback_address));
    }
    if let Some(hash_algorithm) = payload.hash_algorithm.filter(|h| !h.is_empty()) {
        // Fiber expects CkbHash | Sha256 (serde rename_all = snake_case → ckb_hash | sha256)
        let normalized = match hash_algorithm.to_lowercase().as_str() {
            "ckb_hash" | "ckbhash" => "ckb_hash",
            "sha256" | "sha_256" => "sha256",
            other => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("hash_algorithm must be ckb_hash or sha256, got {other}"),
                ));
            }
        };
        obj.insert("hash_algorithm".into(), json!(normalized));
    }
    if let Some(preimage) = payload.payment_preimage.filter(|p| !p.is_empty()) {
        obj.insert("payment_preimage".into(), json!(preimage));
    }
    if let Some(hash) = payload.payment_hash.filter(|h| !h.is_empty()) {
        obj.insert("payment_hash".into(), json!(hash));
    }
    if let Some(delta) = to_hex_u128_opt(&payload.final_expiry_delta)? {
        obj.insert("final_expiry_delta".into(), json!(delta));
    }
    if let Some(allow_mpp) = payload.allow_mpp {
        obj.insert("allow_mpp".into(), json!(allow_mpp));
    }
    if let Some(allow_trampoline_routing) = payload.allow_trampoline_routing {
        obj.insert(
            "allow_trampoline_routing".into(),
            json!(allow_trampoline_routing),
        );
    }
    if let Some(udt) = payload.udt_type_script {
        obj.insert("udt_type_script".into(), udt);
    }

    let result: Value = fiber_rpc_call("new_invoice", object_params(params)).await?;

    let invoice_address = result
        .get("invoice_address")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let invoice = result.get("invoice").cloned().unwrap_or(Value::Null);

    Ok(Json(NewFiberInvoiceResponse {
        invoice_address,
        invoice,
    }))
}

/// Parses a Fiber invoice without paying it (`parse_invoice`).
#[utoipa::path(
    post,
    path = "/fiber/invoice/parse",
    request_body = ParseFiberInvoiceRequest,
    responses(
        (status = 200, description = "Parsed Fiber invoice", body = ParseFiberInvoiceResponse),
        (status = 400, description = "Invalid invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn parse_invoice(
    Json(payload): Json<ParseFiberInvoiceRequest>,
) -> Result<Json<ParseFiberInvoiceResponse>, (StatusCode, String)> {
    if payload.invoice.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "invoice is required".to_string()));
    }

    let result: Value = fiber_rpc_call(
        "parse_invoice",
        object_params(json!({ "invoice": payload.invoice })),
    )
    .await?;

    let invoice = result.get("invoice").cloned().unwrap_or(result);

    Ok(Json(ParseFiberInvoiceResponse { invoice }))
}

/// Returns invoice status by payment hash (`get_invoice`).
#[utoipa::path(
    post,
    path = "/fiber/invoice/get",
    request_body = GetFiberInvoiceRequest,
    responses(
        (status = 200, description = "Invoice status", body = GetFiberInvoiceResponse),
        (status = 400, description = "Invalid payment hash"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn get_invoice(
    Json(payload): Json<GetFiberInvoiceRequest>,
) -> Result<Json<GetFiberInvoiceResponse>, (StatusCode, String)> {
    let payment_hash = normalize_hash(&payload.payment_hash)?;

    let result: Value = fiber_rpc_call(
        "get_invoice",
        object_params(json!({ "payment_hash": payment_hash })),
    )
    .await?;

    Ok(Json(GetFiberInvoiceResponse {
        invoice_address: result
            .get("invoice_address")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        invoice: result.get("invoice").cloned().unwrap_or(Value::Null),
        status: result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }))
}

/// Cancels an unpaid Fiber invoice (`cancel_invoice`).
#[utoipa::path(
    post,
    path = "/fiber/invoice/cancel",
    request_body = CancelFiberInvoiceRequest,
    responses(
        (status = 200, description = "Invoice cancelled", body = GetFiberInvoiceResponse),
        (status = 400, description = "Cannot cancel invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn cancel_invoice(
    Json(payload): Json<CancelFiberInvoiceRequest>,
) -> Result<Json<GetFiberInvoiceResponse>, (StatusCode, String)> {
    let payment_hash = normalize_hash(&payload.payment_hash)?;

    let result: Value = fiber_rpc_call(
        "cancel_invoice",
        object_params(json!({ "payment_hash": payment_hash })),
    )
    .await?;

    Ok(Json(GetFiberInvoiceResponse {
        invoice_address: result
            .get("invoice_address")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        invoice: result.get("invoice").cloned().unwrap_or(Value::Null),
        status: result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }))
}

/// Settles a hold invoice with the payment preimage (`settle_invoice`).
#[utoipa::path(
    post,
    path = "/fiber/invoice/settle",
    request_body = SettleFiberInvoiceRequest,
    responses(
        (status = 200, description = "Invoice settled", body = SettleFiberInvoiceResponse),
        (status = 400, description = "Cannot settle invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC error")
    ),
    tag = "fiber"
)]
pub async fn settle_invoice(
    Json(payload): Json<SettleFiberInvoiceRequest>,
) -> Result<Json<SettleFiberInvoiceResponse>, (StatusCode, String)> {
    let payment_hash = normalize_hash(&payload.payment_hash)?;
    let payment_preimage = normalize_hash(&payload.payment_preimage)?;

    let _: Value = fiber_rpc_call(
        "settle_invoice",
        object_params(json!({
            "payment_hash": payment_hash,
            "payment_preimage": payment_preimage,
        })),
    )
    .await?;

    Ok(Json(SettleFiberInvoiceResponse { settled: true }))
}

fn normalize_hash(value: &str) -> Result<String, (StatusCode, String)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "payment_hash/preimage is required".to_string(),
        ));
    }
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "hash must be 32 bytes (64 hex chars), optionally 0x-prefixed".to_string(),
        ));
    }
    Ok(format!("0x{}", hex.to_lowercase()))
}
