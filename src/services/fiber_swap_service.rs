use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::config::fiber_client::{
    fiber_currency, fiber_rpc_call, object_params, to_hex_u128_opt,
};

#[derive(Deserialize, ToSchema)]
pub struct CkbToLightningRequest {
    /// Bitcoin Lightning BOLT11 invoice to pay with Fiber (CKB / wrapped BTC) funds.
    pub btc_pay_req: String,
    /// Fiber currency for the generated CKB-side invoice. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
}

/// One-shot: create a CCH CKB→Lightning order and pay the Fiber invoice from
/// the connected Fiber node's channel balance.
///
/// This does **not** spend a user's on-chain `/wallet/balance` CKB. It spends
/// Fiber Network funds held by the node (CCH typically uses wrapped BTC UDT
/// at ~1:1 sats). Requires Fiber CCH + LND to be enabled.
#[derive(Deserialize, ToSchema)]
pub struct PayLightningRequest {
    /// Bitcoin Lightning BOLT11 invoice to settle.
    pub btc_pay_req: String,
    /// Fiber currency for the CCH Fiber invoice. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
    /// When true (default), immediately `send_payment` on the Fiber
    /// `incoming_invoice`. When false, only creates the CCH order (same as
    /// `/fiber/swap/ckb-to-lightning`).
    #[serde(default = "default_true")]
    pub auto_pay: bool,
    /// Dry-run the Fiber payment (route/fee estimate, no send).
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Max Fiber routing fee in shannons. Decimal or `0x` hex.
    #[serde(default)]
    pub max_fee_amount: Option<String>,
    /// After paying, poll CCH order status a few times for settlement.
    #[serde(default)]
    pub wait_for_settlement: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, ToSchema)]
pub struct FiberPaymentSummary {
    pub payment_hash: String,
    /// Created | Inflight | Success | Failed
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PayLightningResponse {
    /// CCH order (amounts, Fiber invoice, Lightning status).
    pub order: CchOrderResponse,
    /// Fiber-side payment result when `auto_pay` ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fiber_payment: Option<FiberPaymentSummary>,
    /// Human-readable next step / outcome for clients (Kaze UI).
    pub message: String,
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
    let order = create_ckb_to_lightning_order(&payload.btc_pay_req, payload.currency.as_deref()).await?;
    Ok(Json(order))
}

/// Pay a Lightning invoice in one call: CCH `send_btc` + Fiber `send_payment`.
///
/// Existing step-by-step endpoints (`/fiber/swap/ckb-to-lightning` then
/// `/fiber/payment/send`) remain unchanged for clients that prefer them.
#[utoipa::path(
    post,
    path = "/fiber/swap/pay-lightning",
    request_body = PayLightningRequest,
    responses(
        (status = 200, description = "CCH order created; Fiber payment started when auto_pay", body = PayLightningResponse),
        (status = 400, description = "Invalid Lightning invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH / payment error")
    ),
    tag = "fiber"
)]
pub async fn pay_lightning(
    Json(payload): Json<PayLightningRequest>,
) -> Result<Json<PayLightningResponse>, (StatusCode, String)> {
    let mut order =
        create_ckb_to_lightning_order(&payload.btc_pay_req, payload.currency.as_deref()).await?;

    if !payload.auto_pay {
        return Ok(Json(PayLightningResponse {
            message: "CCH order created. Pay order.incoming_invoice via /fiber/payment/send to settle Lightning."
                .to_string(),
            order,
            fiber_payment: None,
        }));
    }

    if order.incoming_invoice.trim().is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            "CCH order missing incoming_invoice; cannot auto-pay".to_string(),
        ));
    }
    if !order.incoming_network.is_empty() && order.incoming_network != "Fiber" {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!(
                "expected Fiber incoming_invoice for CKB→Lightning, got {}",
                order.incoming_network
            ),
        ));
    }

    let fiber_payment = send_fiber_invoice_payment(
        &order.incoming_invoice,
        payload.dry_run,
        payload.max_fee_amount.as_ref(),
    )
    .await?;

    if payload.wait_for_settlement.unwrap_or(false) && !payload.dry_run.unwrap_or(false) {
        order = poll_cch_order_briefly(&order.payment_hash).await.unwrap_or(order);
    }

    let message = match (
        fiber_payment.status.as_str(),
        order.status.as_str(),
        payload.dry_run.unwrap_or(false),
    ) {
        (_, _, true) => {
            "Dry-run complete: CCH order created and Fiber payment route estimated (not sent)."
                .to_string()
        }
        ("Success", "Success", _) | ("Success", "OutgoingSuccess", _) => {
            "Lightning invoice settled via Fiber CCH.".to_string()
        }
        ("Success", _, _) | ("Inflight", _, _) | ("Created", _, _) => format!(
            "Fiber payment {}. Poll /fiber/swap/order with payment_hash until Lightning settles (current CCH status: {}).",
            fiber_payment.status, order.status
        ),
        ("Failed", _, _) => format!(
            "Fiber payment failed: {}. CCH order status: {}.",
            fiber_payment
                .failed_error
                .as_deref()
                .unwrap_or("unknown error"),
            order.status
        ),
        _ => format!(
            "Fiber payment {}; CCH status {}. Poll /fiber/swap/order with payment_hash.",
            fiber_payment.status, order.status
        ),
    };

    Ok(Json(PayLightningResponse {
        order,
        fiber_payment: Some(fiber_payment),
        message,
    }))
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

async fn create_ckb_to_lightning_order(
    btc_pay_req: &str,
    currency: Option<&str>,
) -> Result<CchOrderResponse, (StatusCode, String)> {
    let btc_pay_req = btc_pay_req.trim();
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

    let currency = currency
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
        .unwrap_or_else(fiber_currency);

    let result: Value = fiber_rpc_call(
        "send_btc",
        object_params(json!({
            "btc_pay_req": btc_pay_req,
            "currency": currency,
        })),
    )
    .await?;

    Ok(map_cch_order(result))
}

async fn send_fiber_invoice_payment(
    invoice: &str,
    dry_run: Option<bool>,
    max_fee_amount: Option<&String>,
) -> Result<FiberPaymentSummary, (StatusCode, String)> {
    let mut params = json!({ "invoice": invoice });
    let obj = params.as_object_mut().expect("object");

    if let Some(dry_run) = dry_run {
        obj.insert("dry_run".into(), json!(dry_run));
    }
    let max_fee = max_fee_amount.cloned();
    if let Some(max_fee_amount) = to_hex_u128_opt(&max_fee)? {
        obj.insert("max_fee_amount".into(), json!(max_fee_amount));
    }

    let result: Value = fiber_rpc_call("send_payment", object_params(params)).await?;
    Ok(FiberPaymentSummary {
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
            .map(str::to_string),
        fee: result.get("fee").map(value_to_owned_string),
    })
}

/// Short poll so one-shot callers can often return Success without a second request.
async fn poll_cch_order_briefly(payment_hash: &str) -> Result<CchOrderResponse, (StatusCode, String)> {
    const ATTEMPTS: usize = 5;
    const DELAY_MS: u64 = 800;

    let mut last = None;
    for _ in 0..ATTEMPTS {
        let result: Value = fiber_rpc_call(
            "get_cch_order",
            object_params(json!({ "payment_hash": payment_hash })),
        )
        .await?;
        let order = map_cch_order(result);
        let done = matches!(
            order.status.as_str(),
            "Success" | "OutgoingSuccess" | "Failed"
        );
        last = Some(order);
        if done {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(DELAY_MS)).await;
    }

    last.ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            "failed to refresh CCH order status".to_string(),
        )
    })
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
