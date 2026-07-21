use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::create_transaction::{broadcast_transaction, parse_signed_transaction};

#[derive(Deserialize, ToSchema)]
pub struct BroadcastTransactionRequest {
    /// A fully signed transaction, in CKB's standard JSON transaction
    /// format, produced by `/transaction/build` + signing it locally on the
    /// client. The server never sees a private key on this path -- it just
    /// forwards the already-signed transaction to the node, which rejects
    /// it outright if the signature doesn't check out.
    #[schema(value_type = Object)]
    pub transaction: ckb_jsonrpc_types::Transaction,
}

#[derive(Serialize, ToSchema)]
pub struct BroadcastTransactionResponse {
    pub tx_hash: String,
}

/// Submits a client-signed transaction to the CKB node's tx pool. The
/// second half of the non-custodial send flow: build with
/// `/transaction/build`, sign locally, broadcast here.
#[utoipa::path(
    post,
    path = "/transaction/broadcast",
    request_body = BroadcastTransactionRequest,
    responses(
        (status = 200, description = "Transaction was broadcast to the CKB node", body = BroadcastTransactionResponse),
        (status = 400, description = "Invalid, unsigned, or incorrectly signed transaction"),
        (status = 500, description = "Failed to broadcast the transaction")
    )
)]
pub async fn broadcast(
    Json(payload): Json<BroadcastTransactionRequest>,
) -> Result<Json<BroadcastTransactionResponse>, (StatusCode, String)> {
    let tx_hash = tokio::task::spawn_blocking(move || {
        let tx = parse_signed_transaction(payload.transaction);
        broadcast_transaction(&tx)
    })
    .await
    .map_err(|e| {
        eprintln!("broadcast task panicked: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to broadcast the transaction. Please try again.".to_string(),
        )
    })?
    .map_err(|e| {
        eprintln!("transaction/broadcast failed: {e:#}");
        (
            StatusCode::BAD_REQUEST,
            "Failed to broadcast the transaction: it may be unsigned, invalid, or already spent."
                .to_string(),
        )
    })?;

    Ok(Json(BroadcastTransactionResponse {
        tx_hash: tx_hash.to_string(),
    }))
}
