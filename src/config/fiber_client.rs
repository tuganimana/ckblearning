use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// Optional Fiber Network Node (FNN) JSON-RPC URL.
/// When unset, `/fiber/*` endpoints return 503 without affecting the rest of the API.
pub fn fiber_rpc_url() -> Option<String> {
    env::var("FIBER_RPC_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Optional Bearer token for Fiber RPC Biscuit auth (`Authorization: Bearer …`).
pub fn fiber_rpc_token() -> Option<String> {
    env::var("FIBER_RPC_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Maps `CKB_NETWORK` to Fiber invoice currency (`Fibb` / `Fibt` / `Fibd`).
pub fn fiber_currency() -> String {
    match crate::config::client::network().as_str() {
        "mainnet" => "Fibb".to_string(),
        "devnet" => "Fibd".to_string(),
        _ => "Fibt".to_string(),
    }
}

/// Wrapped BTC UDT type script used by Fiber CCH (1 sat = 1 UDT unit, 8 decimals).
///
/// Override with `FIBER_WRAPPED_BTC_TYPE_SCRIPT` (full CKB Script JSON).
/// Testnet defaults to the cWBTC script in `deploy/fiber/config.yml`.
pub fn wrapped_btc_type_script() -> Result<Value, (StatusCode, String)> {
    if let Ok(raw) = env::var("FIBER_WRAPPED_BTC_TYPE_SCRIPT") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return serde_json::from_str(trimmed).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("FIBER_WRAPPED_BTC_TYPE_SCRIPT is not valid JSON: {e}"),
                )
            });
        }
    }

    match crate::config::client::network().as_str() {
        "testnet" => Ok(serde_json::json!({
            "code_hash": "0x25c29dc317811a6f6f3985a7a9ebc4838bd388d19d0feeecf0bcd60f6c0975bb",
            "hash_type": "type",
            "args": "0x9a1086531ed6dc69e0bd44cef5278e03faf3015b31aff60b08fb87663ce8507100000000"
        })),
        other => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Set FIBER_WRAPPED_BTC_TYPE_SCRIPT to a CKB Script JSON for CKB_NETWORK={other} \
                 (required for Lightning→Fiber invoice creation)."
            ),
        )),
    }
}

/// Normalize a 32-byte hash to `0x` + 64 lowercase hex (Fiber `Hash256`).
pub fn normalize_hash(value: &str) -> Result<String, (StatusCode, String)> {
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

/// Encode a decimal (or already-hex) integer as Fiber's `0x…` hex string.
pub fn to_hex_u128(value: &str) -> Result<String, (StatusCode, String)> {
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
        let n = u128::from_str_radix(hex, 16).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid hex amount: {e}"),
            )
        })?;
        return Ok(format!("0x{n:x}"));
    }
    let n: u128 = trimmed.parse().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid decimal amount: {e}"),
        )
    })?;
    Ok(format!("0x{n:x}"))
}

/// Encode an optional decimal/hex integer as Fiber hex, or omit when None/empty.
pub fn to_hex_u128_opt(
    value: &Option<String>,
) -> Result<Option<String>, (StatusCode, String)> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => Ok(Some(to_hex_u128(v)?)),
    }
}

static RPC_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    #[allow(dead_code)]
    id: Option<Value>,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

/// Call a Fiber Network Node JSON-RPC method.
///
/// Fiber wraps object params as a one-element array: `params: [{ … }]`.
/// Pass `params` already in that array form (or as a bare value for methods
/// that take a single positional string — prefer objects when the RPC type
/// is a struct).
pub async fn fiber_rpc_call<T: DeserializeOwned>(
    method: &str,
    params: Value,
) -> Result<T, (StatusCode, String)> {
    let url = fiber_rpc_url().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Fiber RPC is not configured. Set FIBER_RPC_URL to your Fiber Network Node RPC endpoint."
                .to_string(),
        )
    })?;

    let id = RPC_ID.fetch_add(1, Ordering::Relaxed);
    let body = JsonRpcRequest {
        jsonrpc: "2.0",
        id,
        method,
        params,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| {
            eprintln!("fiber rpc client build failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create Fiber RPC client.".to_string(),
            )
        })?;

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body);

    if let Some(token) = fiber_rpc_token() {
        request = request.bearer_auth(token);
    }

    let response = request.send().await.map_err(|e| {
        eprintln!("fiber rpc HTTP error calling {method}: {e}");
        (
            StatusCode::BAD_GATEWAY,
            format!("Fiber RPC unreachable: {e}"),
        )
    })?;

    let status = response.status();
    let text = response.text().await.map_err(|e| {
        eprintln!("fiber rpc failed to read body for {method}: {e}");
        (
            StatusCode::BAD_GATEWAY,
            "Fiber RPC returned an unreadable response.".to_string(),
        )
    })?;

    if !status.is_success() {
        eprintln!("fiber rpc HTTP {status} for {method}: {text}");
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Fiber RPC HTTP error ({status})"),
        ));
    }

    let parsed: JsonRpcResponse<T> = serde_json::from_str(&text).map_err(|e| {
        eprintln!("fiber rpc invalid JSON for {method}: {e}; body={text}");
        (
            StatusCode::BAD_GATEWAY,
            "Fiber RPC returned invalid JSON.".to_string(),
        )
    })?;

    if let Some(err) = parsed.error {
        eprintln!(
            "fiber rpc method {method} error [{}]: {}",
            err.code, err.message
        );
        let client_error =
            (err.code >= -32099 && err.code <= -32000) || err.code == -32602;
        let status = if client_error {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::BAD_GATEWAY
        };
        return Err((status, format!("Fiber RPC error: {}", err.message)));
    }

    parsed.result.ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            "Fiber RPC returned no result.".to_string(),
        )
    })
}

/// Wrap a single object param the way Fiber/jsonrpsee expects: `[{ … }]`.
pub fn object_params(obj: Value) -> Value {
    Value::Array(vec![obj])
}
