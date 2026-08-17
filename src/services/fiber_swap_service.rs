use std::time::Duration;

use axum::{
    extract::{Json, Path},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::config::fiber_client::{
    fiber_currency, fiber_rpc_call, normalize_hash, object_params, to_hex_u128,
    to_hex_u128_opt, wrapped_btc_type_script,
};

/// Fiber invoice `final_expiry_delta` for CCH receive_btc (milliseconds).
/// Fiber requires ≥ 16 hours; CCH requires Fiber TLC < half of BTC CLTV
/// (default BTC hop ~30–60h, so 16h is the safe overlap).
const RECEIVE_FIBER_FINAL_EXPIRY_DELTA_MS: u64 = 16 * 60 * 60 * 1000;

const DEFAULT_WAIT_SECS: u64 = 12;
const MAX_WAIT_SECS: u64 = 30;
const POLL_INTERVAL_MS: u64 = 800;

fn default_true() -> bool {
    true
}

fn default_wait_secs() -> u64 {
    DEFAULT_WAIT_SECS
}

#[derive(Deserialize, ToSchema)]
pub struct CkbToLightningRequest {
    /// Bitcoin Lightning BOLT11 invoice to pay with Fiber (wrapped BTC) funds.
    pub btc_pay_req: String,
    /// Fiber currency for the generated CKB-side invoice. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
}

/// One-shot: create a CCH Fiber→Lightning order and pay the Fiber invoice from
/// the connected Fiber node's channel balance.
///
/// This does **not** spend a user's on-chain `/wallet/balance` CKB. It spends
/// Fiber Network funds held by the node (CCH uses wrapped BTC UDT at ~1:1 sats).
/// Requires Fiber CCH + LND to be enabled.
#[derive(Deserialize, ToSchema)]
pub struct PayLightningRequest {
    /// Bitcoin Lightning BOLT11 invoice to settle.
    pub btc_pay_req: String,
    /// Fiber currency for the CCH Fiber invoice. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
    /// When true (default), immediately `send_payment` on the Fiber
    /// `incoming_invoice`. When false, only creates the CCH order (same as
    /// `/fiber/swap/fiber-to-lightning`).
    #[serde(default = "default_true")]
    pub auto_pay: bool,
    /// Dry-run the Fiber payment (route/fee estimate, no send).
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Max Fiber routing fee in shannons / UDT units. Decimal or `0x` hex.
    #[serde(default)]
    pub max_fee_amount: Option<String>,
    /// Required when this API node is also the CCH (same-node self payment).
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub allow_self_payment: bool,
    /// After paying, poll CCH order status until Success/Failed or timeout.
    #[serde(default = "default_true")]
    pub wait_for_settlement: bool,
    /// Settlement poll timeout in seconds (default 12, max 30).
    #[serde(default = "default_wait_secs")]
    pub timeout_seconds: u64,
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
    /// Must use `hash_algorithm: sha256` and the wrapped BTC UDT type script
    /// (create via `/fiber/swap/receive-lightning`, or `/fiber/invoice/new`
    /// with those flags).
    pub fiber_pay_req: String,
}

/// One-shot Lightning → Fiber: mint a sha256 wrapped-BTC Fiber invoice, then
/// create a CCH order. Share `lightning_invoice` with the BTC payer.
#[derive(Deserialize, ToSchema)]
pub struct ReceiveLightningRequest {
    /// Amount of wrapped BTC to receive, in satoshis (1 sat = 1 cWBTC unit).
    /// Decimal or `0x` hex.
    pub amount_sats: String,
    /// Human-readable description copied onto the Fiber invoice.
    #[serde(default)]
    pub description: Option<String>,
    /// Fiber invoice expiry in seconds. Decimal or `0x` hex.
    #[serde(default)]
    pub expiry: Option<String>,
    /// Fiber currency. Defaults from `CKB_NETWORK`.
    #[serde(default)]
    pub currency: Option<String>,
    /// Poll until Lightning is paid / order fails (default false).
    #[serde(default)]
    pub wait_for_settlement: Option<bool>,
    /// Settlement poll timeout in seconds (default 12, max 30).
    #[serde(default = "default_wait_secs")]
    pub timeout_seconds: u64,
}

#[derive(Serialize, ToSchema)]
pub struct ReceiveLightningResponse {
    pub order: CchOrderResponse,
    /// Fiber invoice this node will settle when Lightning is paid.
    pub fiber_invoice: String,
    /// BOLT11 for the Bitcoin Lightning payer (`order.incoming_invoice`).
    pub lightning_invoice: String,
    pub message: String,
}

#[derive(Deserialize, ToSchema)]
pub struct GetCchOrderRequest {
    /// Shared HTLC payment hash linking both sides of the swap.
    pub payment_hash: String,
}

#[derive(Deserialize, ToSchema)]
pub struct WaitCchOrderRequest {
    pub payment_hash: String,
    /// Poll timeout in seconds (default 12, max 30).
    #[serde(default = "default_wait_secs")]
    pub timeout_seconds: u64,
}

#[derive(Deserialize, ToSchema)]
pub struct SwapQuoteRequest {
    /// `fiber_to_lightning` or `lightning_to_fiber`. Inferred from which
    /// invoice field is set when omitted.
    #[serde(default)]
    pub direction: Option<String>,
    /// BOLT11 invoice (Fiber → Lightning quotes).
    #[serde(default)]
    pub btc_pay_req: Option<String>,
    /// Amount in satoshis for Lightning → Fiber quotes. Decimal or `0x` hex.
    #[serde(default)]
    pub amount_sats: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SwapQuoteResponse {
    /// fiber_to_lightning | lightning_to_fiber
    pub direction: String,
    /// Lightning-side amount in satoshis (invoice amount, no hub fee).
    pub lightning_amount_sats: String,
    /// Fiber-side amount in satoshis before hub fee (1:1 wrapped BTC).
    pub fiber_amount_sats: String,
    /// Hub fee is applied at order creation (typically 1 ppm + base, often 0).
    pub hub_fee_note: String,
    pub asset: String,
    pub next_step: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SwapReadyResponse {
    pub fiber_rpc_ok: bool,
    pub cch_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers_count: Option<String>,
    pub message: String,
}

#[derive(Serialize, ToSchema)]
pub struct CchOrderResponse {
    pub payment_hash: String,
    /// FiberToLightning | LightningToFiber | Unknown
    pub direction: String,
    /// Fiber or Lightning invoice the user (or counterparty) must pay.
    pub incoming_invoice: String,
    /// Whether `incoming_invoice` is Fiber or Lightning.
    pub incoming_network: String,
    /// The opposite-network pay request that the hub will settle.
    pub outgoing_pay_req: String,
    /// Amount required in satoshis (including fee), as returned by Fiber (often hex).
    pub amount_sats: String,
    /// Same amount as a decimal string for clients.
    pub amount_sats_decimal: String,
    /// Hub fee in satoshis.
    pub fee_sats: String,
    pub fee_sats_decimal: String,
    /// Pending | IncomingAccepted | OutgoingInFlight | OutgoingSuccess | Success | Failed
    pub status: String,
    pub settled: bool,
    pub failed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_delta_seconds: Option<String>,
    /// Unix seconds when the order expires (`timestamp + expiry_delta`), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub wrapped_btc_type_script: Option<Value>,
    /// What the client should do next.
    pub next_action: String,
    /// Full Fiber CCH order payload for advanced clients.
    #[schema(value_type = Object)]
    pub raw: Value,
}

/// Swap Fiber (wrapped BTC) → Bitcoin Lightning: pay a BOLT11 with Fiber funds.
///
/// Calls Fiber CCH `send_btc`. Returns a Fiber invoice in `incoming_invoice` —
/// pay it with `/fiber/payment/send` (or use `/fiber/swap/pay-lightning`).
#[utoipa::path(
    post,
    path = "/fiber/swap/fiber-to-lightning",
    request_body = CkbToLightningRequest,
    responses(
        (status = 200, description = "CCH order created; pay the Fiber incoming_invoice", body = CchOrderResponse),
        (status = 400, description = "Invalid Lightning invoice"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn fiber_to_lightning(
    Json(payload): Json<CkbToLightningRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    let order =
        create_fiber_to_lightning_order(&payload.btc_pay_req, payload.currency.as_deref()).await?;
    Ok(Json(order))
}

/// Alias of `/fiber/swap/fiber-to-lightning` (kept for existing clients).
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
    payload: Json<CkbToLightningRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    fiber_to_lightning(payload).await
}

/// Pay a Lightning invoice in one call: CCH `send_btc` + Fiber `send_payment`.
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
        create_fiber_to_lightning_order(&payload.btc_pay_req, payload.currency.as_deref()).await?;

    if !payload.auto_pay {
        return Ok(Json(PayLightningResponse {
            message: "CCH order created. Pay order.incoming_invoice via /fiber/payment/send \
                      (set allow_self_payment=true if this node is the hub) to settle Lightning."
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
                "expected Fiber incoming_invoice for Fiber→Lightning, got {}",
                order.incoming_network
            ),
        ));
    }

    let fiber_payment = send_fiber_invoice_payment(
        &order.incoming_invoice,
        payload.dry_run,
        payload.max_fee_amount.as_ref(),
        payload.allow_self_payment,
    )
    .await?;

    let dry_run = payload.dry_run.unwrap_or(false);
    if payload.wait_for_settlement && !dry_run {
        order = poll_cch_order_until(&order.payment_hash, payload.timeout_seconds)
            .await
            .unwrap_or(order);
    }

    let message = match (
        fiber_payment.status.as_str(),
        order.status.as_str(),
        dry_run,
    ) {
        (_, _, true) => {
            "Dry-run complete: CCH order created and Fiber payment route estimated (not sent)."
                .to_string()
        }
        ("Success", "Success", _) | ("Success", "OutgoingSuccess", _) => {
            "Lightning invoice settled via Fiber CCH.".to_string()
        }
        ("Failed", _, _) => format!(
            "Fiber payment failed: {}. CCH order status: {}.",
            fiber_payment
                .failed_error
                .as_deref()
                .unwrap_or("unknown error"),
            order.status
        ),
        _ => format!(
            "Fiber payment {}. Poll GET /fiber/swap/order/{{payment_hash}} or POST /fiber/swap/wait until Lightning settles (current CCH status: {}).",
            fiber_payment.status, order.status
        ),
    };

    Ok(Json(PayLightningResponse {
        order,
        fiber_payment: Some(fiber_payment),
        message,
    }))
}

/// Swap Bitcoin Lightning → Fiber: receive BTC LN payment into wrapped BTC.
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
    let order = create_lightning_to_fiber_order(&payload.fiber_pay_req).await?;
    Ok(Json(order))
}

/// One-shot Lightning → Fiber: create a CCH-compatible Fiber invoice and CCH order.
#[utoipa::path(
    post,
    path = "/fiber/swap/receive-lightning",
    request_body = ReceiveLightningRequest,
    responses(
        (status = 200, description = "CCH order created; share lightning_invoice with the BTC payer", body = ReceiveLightningResponse),
        (status = 400, description = "Invalid amount"),
        (status = 503, description = "Fiber RPC / wrapped BTC script not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn receive_lightning(
    Json(payload): Json<ReceiveLightningRequest>,
) -> Result<Json<ReceiveLightningResponse>, (StatusCode, String)> {
    let amount = to_hex_u128(&payload.amount_sats)?;
    let amount_sats = parse_u128_amount(&payload.amount_sats)?;
    if amount_sats == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "amount_sats must be greater than 0".to_string(),
        ));
    }

    let currency = payload
        .currency
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
        .unwrap_or_else(fiber_currency);

    let udt = wrapped_btc_type_script()?;
    let mut params = json!({
        "amount": amount,
        "currency": currency,
        "hash_algorithm": "sha256",
        "udt_type_script": udt,
        "final_expiry_delta": format!("0x{RECEIVE_FIBER_FINAL_EXPIRY_DELTA_MS:x}"),
        "description": payload
            .description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .unwrap_or("Fiber CCH Lightning receive"),
    });
    let obj = params.as_object_mut().expect("object");
    if let Some(expiry) = to_hex_u128_opt(&payload.expiry)? {
        obj.insert("expiry".into(), json!(expiry));
    }

    let invoice_result: Value = fiber_rpc_call("new_invoice", object_params(params)).await?;
    let fiber_invoice = invoice_result
        .get("invoice_address")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if fiber_invoice.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            "Fiber new_invoice did not return invoice_address".to_string(),
        ));
    }

    let mut order = create_lightning_to_fiber_order(&fiber_invoice).await?;
    if payload.wait_for_settlement.unwrap_or(false) {
        order = poll_cch_order_until(&order.payment_hash, payload.timeout_seconds)
            .await
            .unwrap_or(order);
    }

    let lightning_invoice = if order.incoming_network == "Lightning" {
        order.incoming_invoice.clone()
    } else {
        String::new()
    };

    let message = if order.settled {
        "Lightning payment received and Fiber invoice settled.".to_string()
    } else if lightning_invoice.is_empty() {
        "CCH order created but no Lightning invoice was returned. Inspect order.raw.".to_string()
    } else {
        format!(
            "Share lightning_invoice with the BTC payer. Poll GET /fiber/swap/order/{} until status is Success (current: {}).",
            order.payment_hash, order.status
        )
    };

    Ok(Json(ReceiveLightningResponse {
        order,
        fiber_invoice,
        lightning_invoice,
        message,
    }))
}

/// Preview a Fiber ↔ Lightning swap without creating a CCH order.
#[utoipa::path(
    post,
    path = "/fiber/swap/quote",
    request_body = SwapQuoteRequest,
    responses(
        (status = 200, description = "Swap quote", body = SwapQuoteResponse),
        (status = 400, description = "Invalid invoice or amount")
    ),
    tag = "fiber"
)]
pub async fn quote_swap(
    Json(payload): Json<SwapQuoteRequest>,
) -> Result<Json<SwapQuoteResponse>, (StatusCode, String)> {
    let btc_pay_req = payload
        .btc_pay_req
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let amount_sats = payload
        .amount_sats
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let direction = payload
        .direction
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .or_else(|| {
            if btc_pay_req.is_some() {
                Some("fiber_to_lightning".to_string())
            } else if amount_sats.is_some() {
                Some("lightning_to_fiber".to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "provide direction, or btc_pay_req (Fiber→Lightning), or amount_sats (Lightning→Fiber)"
                    .to_string(),
            )
        })?;

    let mut warnings = Vec::new();
    warnings.push(
        "CCH swaps wrapped BTC (cWBTC) 1:1 with Lightning sats — not native on-chain CKB.".into(),
    );
    warnings.push(
        "Exact hub fee is returned on the CCH order (typically 1 ppm + base fee, often 0 on testnet)."
            .into(),
    );

    match direction.as_str() {
        "fiber_to_lightning" | "ckb_to_lightning" | "fiber-to-lightning" => {
            let invoice = btc_pay_req.ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "btc_pay_req is required for fiber_to_lightning quotes".to_string(),
                )
            })?;
            let normalized = normalize_bolt11(invoice);
            if !looks_like_bolt11(&normalized) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "btc_pay_req does not look like a BOLT11 Lightning invoice".to_string(),
                ));
            }
            let sats = parse_bolt11_amount_sats(&normalized)?;
            let lightning_amount = match sats {
                Some(0) | None => {
                    warnings.push(
                        "Invoice has no amount (or zero). CCH send_btc rejects amount-less invoices."
                            .into(),
                    );
                    "0".to_string()
                }
                Some(n) => n.to_string(),
            };
            Ok(Json(SwapQuoteResponse {
                direction: "fiber_to_lightning".to_string(),
                lightning_amount_sats: lightning_amount.clone(),
                fiber_amount_sats: lightning_amount,
                hub_fee_note: "Fiber pays Lightning amount + hub fee in wrapped BTC sats."
                    .to_string(),
                asset: "cWBTC (wrapped BTC UDT, 8 decimals)".to_string(),
                next_step: "POST /fiber/swap/pay-lightning with btc_pay_req".to_string(),
                warnings,
            }))
        }
        "lightning_to_fiber" | "btc_to_ckb" | "lightning-to-fiber" => {
            let raw = amount_sats.ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "amount_sats is required for lightning_to_fiber quotes".to_string(),
                )
            })?;
            let sats = parse_u128_amount(raw)?;
            if sats == 0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "amount_sats must be greater than 0".to_string(),
                ));
            }
            Ok(Json(SwapQuoteResponse {
                direction: "lightning_to_fiber".to_string(),
                lightning_amount_sats: sats.to_string(),
                fiber_amount_sats: sats.to_string(),
                hub_fee_note: "Lightning payer sends amount + hub fee; Fiber receives amount_sats."
                    .to_string(),
                asset: "cWBTC (wrapped BTC UDT, 8 decimals)".to_string(),
                next_step: "POST /fiber/swap/receive-lightning with amount_sats".to_string(),
                warnings,
            }))
        }
        other => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "direction must be fiber_to_lightning or lightning_to_fiber, got {other}"
            ),
        )),
    }
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
    Ok(Json(fetch_cch_order(&payload.payment_hash).await?))
}

/// REST lookup of a CCH order by payment hash.
#[utoipa::path(
    get,
    path = "/fiber/swap/order/{payment_hash}",
    params(
        ("payment_hash" = String, Path, description = "HTLC payment hash (64 hex, optional 0x prefix)")
    ),
    responses(
        (status = 200, description = "CCH order status", body = CchOrderResponse),
        (status = 400, description = "Invalid payment hash"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn get_cch_order_by_path(
    Path(payment_hash): Path<String>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    Ok(Json(fetch_cch_order(&payment_hash).await?))
}

/// Poll a CCH order until Success, Failed, or timeout.
#[utoipa::path(
    post,
    path = "/fiber/swap/wait",
    request_body = WaitCchOrderRequest,
    responses(
        (status = 200, description = "Latest CCH order status", body = CchOrderResponse),
        (status = 400, description = "Invalid payment hash"),
        (status = 503, description = "Fiber RPC not configured"),
        (status = 502, description = "Fiber RPC / CCH error")
    ),
    tag = "fiber"
)]
pub async fn wait_cch_order(
    Json(payload): Json<WaitCchOrderRequest>,
) -> Result<Json<CchOrderResponse>, (StatusCode, String)> {
    Ok(Json(
        poll_cch_order_until(&payload.payment_hash, payload.timeout_seconds).await?,
    ))
}

/// Check whether Fiber RPC and CCH (`send_btc`) are available for swaps.
#[utoipa::path(
    get,
    path = "/fiber/swap/ready",
    responses(
        (status = 200, description = "Swap readiness", body = SwapReadyResponse),
        (status = 503, description = "Fiber RPC not configured")
    ),
    tag = "fiber"
)]
pub async fn swap_ready() -> Result<Json<SwapReadyResponse>, (StatusCode, String)> {
    let node: Value = fiber_rpc_call("node_info", json!([])).await?;
    let pubkey = node
        .get("pubkey")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let channel_count = node.get("channel_count").map(value_to_owned_string);
    let peers_count = node.get("peers_count").map(value_to_owned_string);

    let cch_probe = fiber_rpc_call::<Value>("send_btc", object_params(json!({}))).await;
    let cch_enabled = match &cch_probe {
        Ok(_) => true,
        Err((_, msg)) => {
            let lower = msg.to_ascii_lowercase();
            !lower.contains("method not found") && !lower.contains("not available")
        }
    };

    let channels_ok = channel_count
        .as_deref()
        .and_then(|s| parse_u128_amount(s).ok())
        .map(|n| n > 0)
        .unwrap_or(false);

    let message = match (cch_enabled, channels_ok) {
        (true, true) => {
            "Fiber CCH looks ready. Use /fiber/swap/pay-lightning or /fiber/swap/receive-lightning."
                .to_string()
        }
        (true, false) => {
            "CCH RPC is present but this node has no Fiber channels yet — swaps will fail until a channel is funded."
                .to_string()
        }
        (false, _) => {
            "Fiber RPC is up but CCH send_btc is not available. Enable the cch service and LND."
                .to_string()
        }
    };

    Ok(Json(SwapReadyResponse {
        fiber_rpc_ok: true,
        cch_enabled,
        pubkey,
        channel_count,
        peers_count,
        message,
    }))
}

async fn create_fiber_to_lightning_order(
    btc_pay_req: &str,
    currency: Option<&str>,
) -> Result<CchOrderResponse, (StatusCode, String)> {
    let btc_pay_req = normalize_bolt11(btc_pay_req);
    if btc_pay_req.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "btc_pay_req is required".to_string(),
        ));
    }
    if !looks_like_bolt11(&btc_pay_req) {
        return Err((
            StatusCode::BAD_REQUEST,
            "btc_pay_req does not look like a BOLT11 Lightning invoice".to_string(),
        ));
    }
    match parse_bolt11_amount_sats(&btc_pay_req)? {
        Some(0) | None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Lightning invoice must encode a non-zero amount (CCH send_btc rejects amount-less invoices)"
                    .to_string(),
            ));
        }
        Some(_) => {}
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

async fn create_lightning_to_fiber_order(
    fiber_pay_req: &str,
) -> Result<CchOrderResponse, (StatusCode, String)> {
    let fiber_pay_req = fiber_pay_req.trim();
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

    let parsed: Value = fiber_rpc_call(
        "parse_invoice",
        object_params(json!({ "invoice": fiber_pay_req })),
    )
    .await?;
    let invoice = parsed.get("invoice").unwrap_or(&parsed);
    validate_fiber_invoice_for_cch(invoice)?;

    let result: Value = fiber_rpc_call(
        "receive_btc",
        object_params(json!({ "fiber_pay_req": fiber_pay_req })),
    )
    .await?;

    Ok(map_cch_order(result))
}

fn validate_fiber_invoice_for_cch(invoice: &Value) -> Result<(), (StatusCode, String)> {
    let algo = invoice
        .get("hash_algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("ckb_hash")
        .to_ascii_lowercase();
    if algo != "sha256" && algo != "sha_256" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Fiber invoice hash_algorithm must be sha256 for Lightning CCH (got {algo}). \
                 Create it via /fiber/swap/receive-lightning or /fiber/invoice/new with hash_algorithm=sha256."
            ),
        ));
    }
    if invoice.get("udt_type_script").is_none()
        || invoice.get("udt_type_script").is_some_and(|v| v.is_null())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Fiber invoice must include udt_type_script for wrapped BTC (CCH does not swap native CKB)."
                .to_string(),
        ));
    }
    Ok(())
}

async fn fetch_cch_order(payment_hash: &str) -> Result<CchOrderResponse, (StatusCode, String)> {
    let payment_hash = normalize_hash(payment_hash)?;
    let result: Value = fiber_rpc_call(
        "get_cch_order",
        object_params(json!({ "payment_hash": payment_hash })),
    )
    .await?;
    Ok(map_cch_order(result))
}

async fn send_fiber_invoice_payment(
    invoice: &str,
    dry_run: Option<bool>,
    max_fee_amount: Option<&String>,
    allow_self_payment: bool,
) -> Result<FiberPaymentSummary, (StatusCode, String)> {
    let mut params = json!({
        "invoice": invoice,
        "allow_self_payment": allow_self_payment,
    });
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

async fn poll_cch_order_until(
    payment_hash: &str,
    timeout_seconds: u64,
) -> Result<CchOrderResponse, (StatusCode, String)> {
    let timeout_seconds = timeout_seconds.clamp(1, MAX_WAIT_SECS);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
    let mut last = fetch_cch_order(payment_hash).await?;
    loop {
        if last.settled || last.failed {
            return Ok(last);
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(last);
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        last = fetch_cch_order(payment_hash).await?;
    }
}

fn map_cch_order(result: Value) -> CchOrderResponse {
    let (incoming_invoice, incoming_network) = extract_incoming_invoice(&result);
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let settled = matches!(status.as_str(), "Success" | "OutgoingSuccess");
    let failed = status == "Failed";
    let direction = match incoming_network.as_str() {
        "Fiber" => "FiberToLightning",
        "Lightning" => "LightningToFiber",
        _ => "Unknown",
    }
    .to_string();
    let amount_sats = value_as_string(result.get("amount_sats"));
    let fee_sats = value_as_string(result.get("fee_sats"));
    let timestamp = result.get("timestamp").map(value_to_owned_string);
    let expiry_delta_seconds = result
        .get("expiry_delta_seconds")
        .map(value_to_owned_string);
    let expires_at = expires_at_unix(timestamp.as_deref(), expiry_delta_seconds.as_deref());
    let next_action = next_action_for(&direction, &status, &incoming_network);

    CchOrderResponse {
        payment_hash: result
            .get("payment_hash")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        direction,
        incoming_invoice,
        incoming_network,
        outgoing_pay_req: result
            .get("outgoing_pay_req")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        amount_sats_decimal: hex_or_dec_to_decimal(&amount_sats),
        fee_sats_decimal: hex_or_dec_to_decimal(&fee_sats),
        amount_sats,
        fee_sats,
        settled,
        failed,
        status,
        timestamp,
        expiry_delta_seconds,
        expires_at,
        wrapped_btc_type_script: result.get("wrapped_btc_type_script").cloned(),
        next_action,
        raw: result,
    }
}

fn next_action_for(direction: &str, status: &str, incoming_network: &str) -> String {
    match (direction, status) {
        (_, "Success") | (_, "OutgoingSuccess") => "done".to_string(),
        (_, "Failed") => "failed".to_string(),
        ("FiberToLightning", "Pending") if incoming_network == "Fiber" => {
            "pay_fiber_invoice".to_string()
        }
        ("LightningToFiber", "Pending") => "pay_lightning_invoice".to_string(),
        _ => "poll_order".to_string(),
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

fn hex_or_dec_to_decimal(value: &str) -> String {
    parse_u128_amount(value)
        .map(|n| n.to_string())
        .unwrap_or_else(|_| value.to_string())
}

fn parse_u128_amount(value: &str) -> Result<u128, (StatusCode, String)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "amount must not be empty".to_string(),
        ));
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u128::from_str_radix(hex, 16).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid hex amount: {e}"),
            )
        });
    }
    trimmed.parse().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid decimal amount: {e}"),
        )
    })
}

fn expires_at_unix(timestamp: Option<&str>, expiry_delta: Option<&str>) -> Option<String> {
    let ts = parse_u128_amount(timestamp?).ok()?;
    let delta = parse_u128_amount(expiry_delta?).ok()?;
    ts.checked_add(delta).map(|n| n.to_string())
}

fn normalize_bolt11(invoice: &str) -> String {
    let trimmed = invoice.trim();
    let without_scheme = trimmed
        .strip_prefix("lightning:")
        .or_else(|| trimmed.strip_prefix("LIGHTNING:"))
        .unwrap_or(trimmed)
        .trim();
    without_scheme.to_string()
}

fn looks_like_bolt11(invoice: &str) -> bool {
    let lower = normalize_bolt11(invoice).to_ascii_lowercase();
    lower.starts_with("lnbc") || lower.starts_with("lntb") || lower.starts_with("lnbcrt")
}

fn looks_like_fiber_invoice(invoice: &str) -> bool {
    let lower = invoice.trim().to_ascii_lowercase();
    lower.starts_with("fibb") || lower.starts_with("fibt") || lower.starts_with("fibd")
}

/// Parse the amount encoded in a BOLT11 HRP. `None` means any-amount invoice.
fn parse_bolt11_amount_sats(invoice: &str) -> Result<Option<u64>, (StatusCode, String)> {
    let lower = normalize_bolt11(invoice).to_ascii_lowercase();
    let Some((hrp, _)) = lower.rsplit_once('1') else {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid BOLT11 invoice (missing bech32 separator)".to_string(),
        ));
    };
    let rest = if let Some(r) = hrp.strip_prefix("lnbcrt") {
        r
    } else if let Some(r) = hrp.strip_prefix("lnbc") {
        r
    } else if let Some(r) = hrp.strip_prefix("lntb") {
        r
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "unsupported Lightning invoice network prefix".to_string(),
        ));
    };
    if rest.is_empty() {
        return Ok(None);
    }
    let (digits, multiplier) = match rest.chars().last() {
        Some(c @ ('m' | 'u' | 'n' | 'p')) => (&rest[..rest.len() - 1], Some(c)),
        _ => (rest, None),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid BOLT11 amount in invoice HRP".to_string(),
        ));
    }
    let amount: u128 = digits.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "BOLT11 amount is too large".to_string(),
        )
    })?;
    // Convert to millisatoshis, then to whole sats (CCH requires integer sats).
    let millisats: u128 = match multiplier {
        None => amount.saturating_mul(100_000_000_000), // BTC → msat
        Some('m') => amount.saturating_mul(100_000_000),
        Some('u') => amount.saturating_mul(100_000),
        Some('n') => amount.saturating_mul(100),
        Some('p') => amount.saturating_mul(1).saturating_div(10),
        _ => amount,
    };
    if millisats % 1000 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Lightning invoice amount is not a whole number of satoshis".to_string(),
        ));
    }
    u64::try_from(millisats / 1000)
        .map(Some)
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Lightning invoice amount exceeds u64 sats".to_string(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bolt11_detects_prefixes_and_lightning_scheme() {
        assert!(looks_like_bolt11("lnbc20m1pvjluez"));
        assert!(looks_like_bolt11("lntb100n1qq"));
        assert!(looks_like_bolt11("lightning:lnbc1pvjluez"));
        assert!(!looks_like_bolt11("fibt1qq"));
        assert!(!looks_like_bolt11(""));
    }

    #[test]
    fn bolt11_parses_hrp_amounts() {
        assert_eq!(
            parse_bolt11_amount_sats("lnbc20m1pvjluezpp5qq").unwrap(),
            Some(2_000_000)
        );
        assert_eq!(
            parse_bolt11_amount_sats("lnbc2500u1pvjluezpp5qq").unwrap(),
            Some(250_000)
        );
        assert_eq!(
            parse_bolt11_amount_sats("lnbc100n1pvjluezpp5qq").unwrap(),
            Some(10)
        );
        assert_eq!(
            parse_bolt11_amount_sats("lnbc10n1pvjluezpp5qq").unwrap(),
            Some(1)
        );
        assert_eq!(parse_bolt11_amount_sats("lnbc1pvjluezpp5qq").unwrap(), None);
        assert!(parse_bolt11_amount_sats("lnbc1n1pvjluezpp5qq").is_err());
    }

    #[test]
    fn fiber_invoice_prefix() {
        assert!(looks_like_fiber_invoice("fibt1qq"));
        assert!(looks_like_fiber_invoice("FIBB1qq"));
        assert!(!looks_like_fiber_invoice("lnbc1qq"));
    }

    #[test]
    fn extracts_tagged_incoming_invoice() {
        let (inv, net) = extract_incoming_invoice(&json!({
            "incoming_invoice": { "Fiber": "fibt1abc" }
        }));
        assert_eq!(inv, "fibt1abc");
        assert_eq!(net, "Fiber");

        let (inv, net) = extract_incoming_invoice(&json!({
            "incoming_invoice": { "Lightning": "lnbc20m1abc" }
        }));
        assert_eq!(inv, "lnbc20m1abc");
        assert_eq!(net, "Lightning");
    }

    #[test]
    fn maps_cch_order_direction_and_decimals() {
        let order = map_cch_order(json!({
            "payment_hash": "0x".to_string() + &"ab".repeat(32),
            "incoming_invoice": { "Fiber": "fibt1abc" },
            "outgoing_pay_req": "lnbc20m1xyz",
            "amount_sats": "0x2710",
            "fee_sats": "0x1",
            "status": "Pending",
            "timestamp": "0x68a1",
            "expiry_delta_seconds": "0x64"
        }));
        assert_eq!(order.direction, "FiberToLightning");
        assert_eq!(order.amount_sats_decimal, "10000");
        assert_eq!(order.fee_sats_decimal, "1");
        assert!(!order.settled);
        assert_eq!(order.next_action, "pay_fiber_invoice");
        assert_eq!(order.expires_at.as_deref(), Some("26885"));
    }

    #[test]
    fn cch_fiber_invoice_requires_sha256_and_udt() {
        let err = validate_fiber_invoice_for_cch(&json!({
            "hash_algorithm": "ckb_hash",
            "udt_type_script": { "code_hash": "0x00" }
        }))
        .unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);

        assert!(validate_fiber_invoice_for_cch(&json!({
            "hash_algorithm": "sha256",
            "udt_type_script": { "code_hash": "0x00" }
        }))
        .is_ok());
    }
}
