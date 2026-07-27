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
