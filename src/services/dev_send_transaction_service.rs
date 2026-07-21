use std::str::FromStr;

use axum::{extract::Json, http::StatusCode};
use ckb_sdk::HumanCapacity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::create_transaction::{broadcast_transaction, create_transaction};
use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::dev_guard;
use super::wallet::{derive_wallet, DerivedWallet};

const FEE_BUFFER_SHANNONS: u64 = 100_000_000;

/// `create_transaction`/`broadcast_transaction` report both client-caused
/// problems (bad address/amount -- prefixed "Invalid" by convention in that
/// module) and internal/infra failures (RPC or node issues). We log the full
/// detail server-side either way, but only echo the message back to the
/// caller when it's actually about their input; infra failures get a
/// generic message so we don't leak node/RPC internals in the response.
fn classify_tx_error(e: anyhow::Error) -> (StatusCode, String) {
    let message = e.to_string();
    eprintln!("dev/transaction/send failed: {e:#}");

    if message.starts_with("Invalid") {
        (StatusCode::BAD_REQUEST, message)
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send the transaction. Please verify your inputs and try again.".to_string(),
        )
    }
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
pub struct DevSendTransactionRequest {
    /// Sender's mnemonic phrase. Used in-memory only to derive the signing
    /// key for this single request, never stored server-side -- but it
    /// still crosses the network to reach this endpoint, which is exactly
    /// what the non-custodial flow (/transaction/build + client-side
    /// signing + /transaction/broadcast) avoids.
    pub mnemonic: String,
    /// Recipient CKB address.
    pub receiver_address: String,
    /// Amount to send, in CKB, e.g. "100.0".
    pub amount: String,
}

#[derive(Serialize, ToSchema)]
pub struct DevSendTransactionResponse {
    pub tx_hash: String,
    /// Derivation index the funds were actually sent from, found
    /// automatically rather than requiring the caller to specify it.
    pub sender_index: u32,
    pub sender_address: String,
    pub receiver_address: String,
    pub amount: String,
}

/// **Dev/testing only** -- disabled unless `ALLOW_DEV_KEY_ENDPOINTS=true`.
/// Builds, signs, and broadcasts a transaction *server-side* given a raw
/// mnemonic. Use `/transaction/build` + client-side signing +
/// `/transaction/broadcast` for the real, non-custodial flow: this endpoint
/// exists only for quick local/devnet iteration when you don't yet have a
/// client-side signer wired up.
#[utoipa::path(
    post,
    path = "/dev/transaction/send",
    request_body = DevSendTransactionRequest,
    responses(
        (status = 200, description = "Transaction was built, signed, and broadcast to the CKB node", body = DevSendTransactionResponse),
        (status = 400, description = "Invalid mnemonic/address/amount, or no address has enough balance"),
        (status = 403, description = "Disabled: set ALLOW_DEV_KEY_ENDPOINTS=true to enable for local testing"),
        (status = 500, description = "Failed to build or broadcast the transaction")
    )
)]
pub async fn send_transaction_dev(
    Json(payload): Json<DevSendTransactionRequest>,
) -> Result<Json<DevSendTransactionResponse>, (StatusCode, String)> {
    dev_guard::require_enabled()?;

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
        eprintln!("dev transaction task panicked: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send the transaction. Please try again.".to_string(),
        )
    })?
    .map_err(classify_tx_error)?;

    Ok(Json(DevSendTransactionResponse {
        tx_hash: tx_hash.to_string(),
        sender_index: index,
        sender_address: wallet.address.to_string(),
        receiver_address: payload.receiver_address,
        amount: payload.amount,
    }))
}
