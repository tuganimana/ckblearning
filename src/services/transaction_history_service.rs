use std::str::FromStr;

use axum::{extract::Json, http::StatusCode};
use ckb_sdk::Address;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::balance::list_transactions;

const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize, ToSchema)]
pub struct TransactionHistoryRequest {
    /// The CKB address to look up. Purely a public read, same as /balance.
    pub address: String,
    pub limit: Option<u32>,
}

#[derive(Serialize, ToSchema)]
pub struct TransactionRecord {
    pub tx_hash: String,
    pub block_number: u64,
    /// "received" when this transaction paid a cell to the address,
    /// "sent" when it spent one from it.
    pub direction: String,
}

#[derive(Serialize, ToSchema)]
pub struct TransactionHistoryResponse {
    pub address: String,
    pub transactions: Vec<TransactionRecord>,
}

#[utoipa::path(
    post,
    path = "/transactions",
    request_body = TransactionHistoryRequest,
    responses(
        (status = 200, description = "Recent transactions for the given address, newest first", body = TransactionHistoryResponse),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Failed to fetch transaction history from the CKB node")
    )
)]
pub async fn get_transaction_history(
    Json(payload): Json<TransactionHistoryRequest>,
) -> Result<Json<TransactionHistoryResponse>, (StatusCode, String)> {
    let address = Address::from_str(&payload.address)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid address: {e}")))?;

    let limit = payload.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let records = tokio::task::spawn_blocking({
        let address = address.clone();
        move || list_transactions(&address, limit)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Transaction history task panicked: {e}"),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch transaction history: {e}"),
        )
    })?;

    let transactions = records
        .into_iter()
        .map(|record| TransactionRecord {
            tx_hash: record.tx_hash.to_string(),
            block_number: record.block_number,
            direction: record.direction.to_string(),
        })
        .collect();

    Ok(Json(TransactionHistoryResponse {
        address: address.to_string(),
        transactions,
    }))
}
