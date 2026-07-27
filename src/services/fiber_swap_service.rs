use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::config::fiber_client::{fiber_currency, fiber_rpc_call, object_params};

#[derive(Deserialize, ToSchema)]
pub struct CkbToLightningRequest {
    /// Bitcoin Lightning BOLT11 invoice to pay with Fiber (CKB / wrapped BTC) funds.
    pub btc_pay_req: String,
    /// Fiber currency for the generated CKB-side invoice. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct BtcToCkbRequest {
    /// Fiber invoice (`fibt…`) that should be paid when BTC Lightning settles.
    /// For Lightning compatibility this invoice must use `hash_algorithm: sha256`
    /// (create via `/fiber/invoice/new` with that flag).
    pub fiber_pay_req: String,
}

#[derive(Deserialize, ToSchema)]
pub struct GetCchOrderRequest {
    /// Shared HTLC payment hash linking both sides of the swap.
    pub payment_hash: String,
}

#[derive(Serialize, ToSchema)]
pub struct CchOrderResponse {
    pub payment_hash: String,
    /// Fiber or Lightning invoice the user (or counterparty) must pay.
    pub incoming_invoice: String,
    /// Whether `incoming_invoice` is Fiber or Lightning.
    pub incoming_network: String,
    /// The opposite-network pay request that the hub will settle.
    pub outgoing_pay_req: String,
    /// Amount required in satoshis (including fee), as returned by Fiber (often hex).
    pub amount_sats: String,
    /// Hub fee in satoshis.
    pub fee_sats: String,
    /// Pending | IncomingAccepted | OutgoingInFlight | OutgoingSuccess | Success | Failed
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_delta_seconds: Option<String>,
    /// Full Fiber CCH order payload for advanced clients.
    #[schema(value_type = Object)]
    pub raw: Value,
}

/// Swap CKB (Fiber) → Bitcoin Lightning: pay a BOLT11 invoice with Fiber funds.
///
/// Calls Fiber CCH `send_btc`. Returns a Fiber invoice in `incoming_invoice` —
/// pay it with `/fiber/payment/send` to complete the atomic swap.
#[utoipa::path(
    post,
    path = "/fiber/swap/ckb-to-lightning",
    request_body = CkbToLightningRequest,
    responses(
        (status = 200, description = "CCH order created; pay the Fiber incoming_invoice", body = CchOrderResponse),
        (status = 400, description = "Invalid Lightning invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn ckb_to_lightning(
    Json(payload): Json<CkbToLightningRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    let btc_pay_req = payload.btc_pay_req.trim();
    if btc_pay_req.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "btc_pay_req is required".to_string(),
        ));
    }
    if !looks_like_bolt11(btc_pay_req) {
        return Err((
            StatusCode::BAD_REQUEST,
            "btc_pay_req does not look like a BOLT11 Lightning invoice".to_string(),
        ));
    }

    let currency = payload
        .currency
        .clone()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(fiber_currency);

    let result: Value = fiber_rpc_call(
        "send_btc",
        object_params(json!({
            "btc_pay_req": btc_pay_req,
            "currency": currency,
        })),
    )
    .await?;

    Ok(Json(map_cch_order(result)))
}

/// Swap Bitcoin Lightning → CKB (Fiber): receive BTC LN payment into Fiber.
///
/// Calls Fiber CCH `receive_btc`. Returns a Lightning invoice in
/// `incoming_invoice` for the BTC payer. When they pay it, the hub settles
/// your Fiber `fiber_pay_req`.
#[utoipa::path(
    post,
    path = "/fiber/swap/btc-to-ckb",
    request_body = BtcToCkbRequest,
    responses(
        (status = 200, description = "CCH order created; share the Lightning incoming_invoice", body = CchOrderResponse),
        (status = 400, description = "Invalid Fiber invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn btc_to_ckb(
    Json(payload): Json<BtcToCkbRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    let fiber_pay_req = payload.fiber_pay_req.trim();
    if fiber_pay_req.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "fiber_pay_req is required".to_string(),
        ));
    }
    if !looks_like_fiber_invoice(fiber_pay_req) {
        return Err((
            StatusCode::BAD_REQUEST,
            "fiber_pay_req does not look like a Fiber invoice (expected fibb/fibt/fibd prefix)"
                .to_string(),
        ));
    }

    let result: Value = fiber_rpc_call(
        "receive_btc",
        object_params(json!({ "fiber_pay_req": fiber_pay_req })),
    )
    .await?;

    Ok(Json(map_cch_order(result)))
}

/// Poll CCH cross-chain order status by payment hash (`get_cch_order`).
#[utoipa::path(
    post,
    path = "/fiber/swap/order",
    request_body = GetCchOrderRequest,
    responses(
        (status = 200, description = "CCH order status", body = CchOrderResponse),
        (status = 400, description = "Invalid payment hash"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn get_cch_order(
    Json(payload): Json<GetCchOrderRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    let payment_hash = payload.payment_hash.trim();
    if payment_hash.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "payment_hash is required".to_string(),
        ));
    }

    let result: Value = fiber_rpc_call(
        "get_cch_order",
        object_params(json!({ "payment_hash": payment_hash })),
    )
    .await?;

    Ok(Json(map_cch_order(result)))
}

fn map_cch_order(result: Value) -> CchOrderResponse {
    let (incoming_invoice, incoming_network) = extract_incoming_invoice(&result);

    CchOrderResponse {
        payment_hash: result
            .get("payment_hash")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        incoming_invoice,
        incoming_network,
        outgoing_pay_req: result
            .get("outgoing_pay_req")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        amount_sats: value_as_string(result.get("amount_sats")),
        fee_sats: value_as_string(result.get("fee_sats")),
        status: result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        timestamp: result.get("timestamp").map(value_to_owned_string),
        expiry_delta_seconds: result
            .get("expiry_delta_seconds")
            .map(value_to_owned_string),
        raw: result,
    }
}

/// Fiber returns `incoming_invoice` as `{ "Fiber": "…" }` or `{ "Lightning": "…" }`.
fn extract_incoming_invoice(result: &Value) -> (String, String) {
    let Some(invoice) = result.get("incoming_invoice") else {
        return (String::new(), String::new());
    };

    if let Some(s) = invoice.as_str() {
        let network = if looks_like_fiber_invoice(s) {
            "Fiber"
        } else {
            "Lightning"
        };
        return (s.to_string(), network.to_string());
    }

    if let Some(obj) = invoice.as_object() {
        if let Some(fiber) = obj.get("Fiber").and_then(|v| v.as_str()) {
            return (fiber.to_string(), "Fiber".to_string());
        }
        if let Some(ln) = obj.get("Lightning").and_then(|v| v.as_str()) {
            return (ln.to_string(), "Lightning".to_string());
        }
    }

    (String::new(), String::new())
}

fn value_as_string(value: Option<&Value>) -> String {
    value.map(value_to_owned_string).unwrap_or_default()
}

fn value_to_owned_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn looks_like_bolt11(invoice: &str) -> bool {
    let lower = invoice.to_ascii_lowercase();
    lower.starts_with("lnbc")
        || lower.starts_with("lntb")
        || lower.starts_with("lnbcrt")
        || lower.starts_with("lnbc1")
        || lower.starts_with("lightning:")
}

fn looks_like_fiber_invoice(invoice: &str) -> bool {
    let lower = invoice.to_ascii_lowercase();
    lower.starts_with("fibb") || lower.starts_with("fibt") || lower.starts_with("fibd")
}
