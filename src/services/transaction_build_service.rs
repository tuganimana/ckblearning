use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::create_transaction::build_unsigned_transaction;

#[derive(Deserialize, ToSchema)]
pub struct BuildTransactionRequest {
    /// Sender's CKB address (public -- no key material).
    pub sender_address: String,
    /// Recipient CKB address.
    pub receiver_address: String,
    /// Amount to send, in CKB, e.g. "100.0".
    pub amount: String,
}

#[derive(Serialize, ToSchema)]
pub struct BuildTransactionResponse {
    /// Unsigned, fee-balanced transaction in CKB's standard JSON transaction
    /// format (same shape the CKB RPC/indexer use). Sign it on the client
    /// with the sender's private key -- which never leaves the client --
    /// using an official CKB SDK (ckb-sdk-js, lumos, ckb-sdk-python, the
    /// Rust `ckb-sdk` crate, etc: they all consume/produce this exact JSON
    /// shape), then submit the signed result to `/transaction/broadcast`.
    #[schema(value_type = Object)]
    pub transaction: serde_json::Value,
    pub sender_address: String,
    pub receiver_address: String,
    pub amount: String,
}

/// Builds a fee-balanced transfer transaction *without signing it*. This
/// endpoint only ever touches public chain data (live cells, cell deps) --
/// it never needs, sees, or asks for a private key. Pair with client-side
/// signing and `/transaction/broadcast` for the full non-custodial send
/// flow.
#[utoipa::path(
    post,
    path = "/transaction/build",
    request_body = BuildTransactionRequest,
    responses(
        (status = 200, description = "Unsigned, fee-balanced transaction ready for the client to sign", body = BuildTransactionResponse),
        (status = 400, description = "Invalid address/amount, or not enough balance to cover it"),
        (status = 500, description = "Failed to build the transaction")
    )
)]
pub async fn build_transaction(
    Json(payload): Json<BuildTransactionRequest>,
) -> Result<Json<BuildTransactionResponse>, (StatusCode, String)> {
    let sender_address = payload.sender_address.clone();
    let receiver_address = payload.receiver_address.clone();
    let amount = payload.amount.clone();

    // Building does blocking network RPC calls (cell collection, genesis
    // block lookup), so run it on the blocking thread pool.
    let tx = tokio::task::spawn_blocking(move || {
        build_unsigned_transaction(&sender_address, &receiver_address, &amount)
    })
    .await
    .map_err(|e| {
        eprintln!("transaction build task panicked: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build the transaction. Please try again.".to_string(),
        )
    })?
    .map_err(|e| {
        let message = e.to_string();
        eprintln!("transaction/build failed: {e:#}");
        if message.starts_with("Invalid") {
            (StatusCode::BAD_REQUEST, message)
        } else {
            (
                StatusCode::BAD_REQUEST,
                "Failed to build the transaction: check the sender address has enough balance."
                    .to_string(),
            )
        }
    })?;

    let json_tx = ckb_jsonrpc_types::Transaction::from(tx.data());
    let transaction = serde_json::to_value(&json_tx).map_err(|e| {
        eprintln!("failed to serialize built transaction: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build the transaction. Please try again.".to_string(),
        )
    })?;

    Ok(Json(BuildTransactionResponse {
        transaction,
        sender_address: payload.sender_address,
        receiver_address: payload.receiver_address,
        amount: payload.amount,
    }))
}
