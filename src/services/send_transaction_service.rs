use std::str::FromStr;

use axum::{extract::Json, http::StatusCode};
use ckb_sdk::{Address, HumanCapacity};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::balance::get_balance;
use crate::config::create_transaction::{broadcast_transaction, create_transaction};

use super::wallet::{derive_wallet, DerivedWallet};


const GAP_LIMIT: u32 = 20;

/// Hard safety cap on how many addresses we'll ever derive/check in one request.
const MAX_SCAN: u32 = 10_000;
const FEE_BUFFER_SHANNONS: u64 = 100_000_000;

async fn get_balance_async(address: Address) -> Result<u64, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || get_balance(&address))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Balance task panicked: {e}"),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch balance: {e}"),
            )
        })
}


async fn find_funded_wallet(
    mnemonic: &str,
    min_balance: u64,
) -> Result<(u32, DerivedWallet), (StatusCode, String)> {
    let mut scanned = 0u32;

    while scanned < MAX_SCAN {
        let batch_end = (scanned + GAP_LIMIT).min(MAX_SCAN);

        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let wallet = derive_wallet(mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            let address = wallet.address.clone();
            handles.push((index, wallet, tokio::spawn(get_balance_async(address))));
        }

        let mut batch_had_funds = false;
        for (index, wallet, handle) in handles {
            let balance = handle.await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Balance task panicked: {e}"),
                )
            })??;

            if balance > 0 {
                batch_had_funds = true;
            }

            if balance >= min_balance {
                return Ok((index, wallet));
            }
        }

        scanned = batch_end;

        if !batch_had_funds {
            break;
        }
    }

    Err((
        StatusCode::BAD_REQUEST,
        "No address derived from this mnemonic has enough balance to cover that amount".to_string(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct SendTransactionRequest {
    /// Sender's mnemonic phrase, e.g. produced by /generate-mnemonic.
    /// Self-custodial: used in-memory only to derive the signing key for
    /// this single request, never stored server-side.
    pub mnemonic: String,
    /// Recipient CKB address.
    pub receiver_address: String,
    /// Amount to send, in CKB, e.g. "100.0".
    pub amount: String,
}

#[derive(Serialize, ToSchema)]
pub struct SendTransactionResponse {
    pub tx_hash: String,
    /// Derivation index the funds were actually sent from, found
    /// automatically rather than requiring the caller to specify it.
    pub sender_index: u32,
    pub sender_address: String,
    pub receiver_address: String,
    pub amount: String,
}

/// Builds, signs, and broadcasts a capacity-transfer transaction. The sender
/// doesn't need to specify which derivation index to spend from: this scans
/// the mnemonic's addresses (like /balance/wallet) and automatically uses
/// the first one with enough balance to cover the amount.
#[utoipa::path(
    post,
    path = "/transaction/send",
    request_body = SendTransactionRequest,
    responses(
        (status = 200, description = "Transaction was built, signed, and broadcast to the CKB node", body = SendTransactionResponse),
        (status = 400, description = "Invalid mnemonic/address/amount, or no address has enough balance"),
        (status = 500, description = "Failed to build or broadcast the transaction")
    )
)]
pub async fn send_transaction(
    Json(payload): Json<SendTransactionRequest>,
) -> Result<Json<SendTransactionResponse>, (StatusCode, String)> {
    let amount_shannons = HumanCapacity::from_str(&payload.amount)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid amount: {e}")))?
        .0;

    let (index, wallet) =
        find_funded_wallet(&payload.mnemonic, amount_shannons + FEE_BUFFER_SHANNONS).await?;

    let sender_address = wallet.address.to_string();
    let sender_private_key = wallet.private_key;
    let receiver_address = payload.receiver_address.clone();
    let amount = payload.amount.clone();

    // create_transaction/broadcast_transaction do blocking network RPC calls,
    // so run them on the blocking thread pool instead of stalling the async
    // runtime's worker thread.
    let tx_hash = tokio::task::spawn_blocking(move || {
        let tx = create_transaction(&sender_address, &sender_private_key, &receiver_address, &amount)?;
        broadcast_transaction(&tx)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Transaction task panicked: {e}"),
        )
    })?
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(SendTransactionResponse {
        tx_hash: tx_hash.to_string(),
        sender_index: index,
        sender_address: wallet.address.to_string(),
        receiver_address: payload.receiver_address,
        amount: payload.amount,
    }))
}
