use std::sync::Arc;

use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::dev_guard;
use super::wallet::derive_wallet;

#[derive(Deserialize, ToSchema)]
pub struct DevWalletBalanceRequest {
    pub mnemonic: String,
}

#[derive(Serialize, ToSchema)]
pub struct DevAddressBalance {
    pub index: u32,
    pub address: String,
    pub balance: u64,
}

#[derive(Serialize, ToSchema)]
pub struct DevWalletBalanceResponse {
    pub balances: Vec<DevAddressBalance>,
    pub total_balance: u64,
}

/// **Dev/testing only** -- disabled unless `ALLOW_DEV_KEY_ENDPOINTS=true`.
/// Takes a mnemonic to scan every derived address. Use `/wallet/balance`
/// (account xpub, no private key) for the real, non-custodial flow.
#[utoipa::path(
    post,
    path = "/dev/balance/wallet",
    request_body = DevWalletBalanceRequest,
    responses(
        (status = 200, description = "Total balance across every address derived from the mnemonic", body = DevWalletBalanceResponse),
        (status = 400, description = "Invalid mnemonic or derivation path"),
        (status = 403, description = "Disabled: set ALLOW_DEV_KEY_ENDPOINTS=true to enable for local testing"),
        (status = 500, description = "Failed to fetch balance from the CKB node")
    )
)]
pub async fn get_wallet_balance_dev(
    Json(payload): Json<DevWalletBalanceRequest>,
) -> Result<Json<DevWalletBalanceResponse>, (StatusCode, String)> {
    dev_guard::require_enabled()?;

    let mnemonic = Arc::new(payload.mnemonic);

    let mut balances = Vec::new();
    let mut total_balance = 0u64;
    let mut scanned = 0u32;
    let mut last_funded: Option<u32> = None;

    while scanned < MAX_SCAN {
        let stop_after = last_funded
            .map(|i| i.saturating_add(GAP_LIMIT))
            .unwrap_or(GAP_LIMIT.saturating_sub(1));
        if scanned > stop_after {
            break;
        }

        let batch_end = (scanned + GAP_LIMIT).min(stop_after + 1).min(MAX_SCAN);
        if batch_end <= scanned {
            break;
        }

        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let wallet =
                derive_wallet(&mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            let address = wallet.address.clone();
            handles.push((index, wallet.address.to_string(), tokio::spawn(get_balance_async(address))));
        }

        for (index, address, handle) in handles {
            let balance = handle.await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Balance task panicked: {e}"),
                )
            })??;

            if balance > 0 {
                last_funded = Some(index);
            }

            total_balance += balance;
            balances.push(DevAddressBalance {
                index,
                address,
                balance,
            });
        }

        scanned = batch_end;
    }

    Ok(Json(DevWalletBalanceResponse {
        balances,
        total_balance,
    }))
}
