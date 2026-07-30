use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::create_transaction::build_unsigned_transaction_multi;

#[derive(Deserialize, ToSchema)]
pub struct BuildTransactionRequest {
    /// Sender's CKB address (public -- no key material).
    /// Ignored when `sender_addresses` is non-empty.
    #[serde(default)]
    pub sender_address: Option<String>,
    /// One or more sender addresses (HD indexes). Use this when balance is
    /// split across receive addresses so inputs can be aggregated.
    #[serde(default)]
    pub sender_addresses: Option<Vec<String>>,
    /// Recipient CKB address.
    pub receiver_address: String,
    /// Amount to send, in CKB, e.g. "100.0".
    pub amount: String,
}

#[derive(Serialize, ToSchema)]
pub struct BuildTransactionSigner {
    /// Witness index that must carry this lock's secp256k1 signature.
    pub witness_index: u32,
    /// Address whose private key signs this witness.
    pub address: String,
}

#[derive(Serialize, ToSchema)]
pub struct BuildTransactionResponse {
    /// Unsigned, fee-balanced transaction in CKB's standard JSON transaction
    /// format. Sign every entry in `signers` on the client, then submit to
    /// `/transaction/broadcast`.
    #[schema(value_type = Object)]
    pub transaction: serde_json::Value,
    pub sender_address: String,
    pub receiver_address: String,
    pub amount: String,
    /// One entry per distinct input lock group (multi-HD sends have >1).
    pub signers: Vec<BuildTransactionSigner>,
}

fn resolve_senders(payload: &BuildTransactionRequest) -> Result<Vec<String>, (StatusCode, String)> {
    let mut senders = Vec::new();
    if let Some(list) = &payload.sender_addresses {
        for addr in list {
            let trimmed = addr.trim();
            if !trimmed.is_empty() {
                senders.push(trimmed.to_string());
            }
        }
    }
    if senders.is_empty() {
        if let Some(single) = payload
            .sender_address
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            senders.push(single);
        }
    }
    if senders.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "sender_address or sender_addresses is required".to_string(),
        ));
    }
    Ok(senders)
}

/// Builds a fee-balanced transfer transaction *without signing it*.
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
    let senders = resolve_senders(&payload)?;
    let receiver_address = payload.receiver_address.clone();
    let amount = payload.amount.clone();
    let primary_sender = senders[0].clone();

    let (tx, plan) = tokio::task::spawn_blocking(move || {
        build_unsigned_transaction_multi(&senders, &receiver_address, &amount)
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

    let signers = plan
        .into_iter()
        .map(|(witness_index, address)| BuildTransactionSigner {
            witness_index: witness_index as u32,
            address,
        })
        .collect();

    Ok(Json(BuildTransactionResponse {
        transaction,
        sender_address: primary_sender,
        receiver_address: payload.receiver_address,
        amount: payload.amount,
        signers,
    }))
}
