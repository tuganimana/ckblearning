use std::str::FromStr;
use std::sync::Arc;

use axum::{extract::Json, http::StatusCode};
use ckb_sdk::Address;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::balance::get_balance;

use super::wallet::derive_wallet;

const GAP_LIMIT: u32 = 10;
/// Hard safety cap on how many addresses we'll ever derive/check in one request.
const MAX_SCAN: u32 = 10_000;

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

#[derive(Deserialize, ToSchema)]
pub struct BalanceRequest {
    /// The CKB address to check. Checking a single balance only ever needs
    /// the (public) address, never the mnemonic/private key.
    pub address: String,
}

#[derive(Serialize, ToSchema)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: u64,
}

/// Checks the on-chain balance of a single CKB address. Purely a public
#[utoipa::path(
    post,
    path = "/balance",
    request_body = BalanceRequest,
    responses(
        (status = 200, description = "Balance of the given address", body = BalanceResponse),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Failed to fetch balance from the CKB node")
    )
)]
pub async fn get_address_balance(
    Json(payload): Json<BalanceRequest>,
) -> Result<Json<BalanceResponse>, (StatusCode, String)> {
    let address = Address::from_str(&payload.address)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid address: {e}")))?;

    let balance = get_balance_async(address.clone()).await?;

    Ok(Json(BalanceResponse {
        address: address.to_string(),
        balance,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct WalletBalanceRequest {
    pub mnemonic: String,
}

#[derive(Serialize, ToSchema)]
pub struct AddressBalance {
    pub index: u32,
    pub address: String,
    pub balance: u64,
}

#[derive(Serialize, ToSchema)]
pub struct WalletBalanceResponse {
    pub balances: Vec<AddressBalance>,
    pub total_balance: u64,
}

#[utoipa::path(
    post,
    path = "/balance/wallet",
    request_body = WalletBalanceRequest,
    responses(
        (status = 200, description = "Total balance across every address derived from the mnemonic", body = WalletBalanceResponse),
        (status = 400, description = "Invalid mnemonic or derivation path"),
        (status = 500, description = "Failed to fetch balance from the CKB node")
    )
)]
pub async fn get_wallet_balance(
    Json(payload): Json<WalletBalanceRequest>,
) -> Result<Json<WalletBalanceResponse>, (StatusCode, String)> {
    let mnemonic = Arc::new(payload.mnemonic);

    let mut balances = Vec::new();
    let mut total_balance = 0u64;
    let mut scanned = 0u32;

    while scanned < MAX_SCAN {
        let batch_end = (scanned + GAP_LIMIT).min(MAX_SCAN);

  
        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let wallet =
                derive_wallet(&mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            let address = wallet.address.clone();
            handles.push((index, wallet.address.to_string(), tokio::spawn(get_balance_async(address))));
        }

        let mut batch_had_funds = false;
        for (index, address, handle) in handles {
            let balance = handle.await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Balance task panicked: {e}"),
                )
            })??;

            if balance > 0 {
                batch_had_funds = true;
            }

            total_balance += balance;
            balances.push(AddressBalance {
                index,
                address,
                balance,
            });
        }

        scanned = batch_end;

        if !batch_had_funds {
            break;
        }
    }

    Ok(Json(WalletBalanceResponse {
        balances,
        total_balance,
    }))
}
