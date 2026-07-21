use std::sync::Arc;

use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::xpub_wallet::derive_address_from_xpub;

#[derive(Deserialize, ToSchema)]
pub struct WalletBalanceRequest {
    /// Account-level extended public key (BIP-32 xpub for `m/44'/309'/0'`).
    /// Never send the mnemonic or an extended private key here.
    pub account_xpub: String,
}

#[derive(Serialize, ToSchema)]
pub struct WalletAddressBalance {
    pub index: u32,
    pub address: String,
    pub balance: u64,
}

#[derive(Serialize, ToSchema)]
pub struct WalletBalanceResponse {
    pub balances: Vec<WalletAddressBalance>,
    pub total_balance: u64,
}

/// Totals the balance across every address derived from an **account
/// xpub**, using the same gap-limit scan as the old mnemonic-based
/// endpoint -- but never needs a private key, since address derivation
/// from an xpub is public-key math only.
#[utoipa::path(
    post,
    path = "/wallet/balance",
    request_body = WalletBalanceRequest,
    responses(
        (status = 200, description = "Total balance across every address derived from the account xpub", body = WalletBalanceResponse),
        (status = 400, description = "Invalid extended public key or derivation path"),
        (status = 500, description = "Failed to fetch balance from the CKB node")
    )
)]
pub async fn get_wallet_balance(
    Json(payload): Json<WalletBalanceRequest>,
) -> Result<Json<WalletBalanceResponse>, (StatusCode, String)> {
    let account_xpub = Arc::new(payload.account_xpub);

    let mut balances = Vec::new();
    let mut total_balance = 0u64;
    let mut scanned = 0u32;

    while scanned < MAX_SCAN {
        let batch_end = (scanned + GAP_LIMIT).min(MAX_SCAN);

        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let address = derive_address_from_xpub(&account_xpub, index)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            handles.push((index, address.to_string(), tokio::spawn(get_balance_async(address))));
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
            balances.push(WalletAddressBalance {
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
