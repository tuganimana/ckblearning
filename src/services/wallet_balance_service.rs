use std::sync::Arc;

use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::balance_cache;
use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::xpub_wallet::derive_address_from_xpub;

#[derive(Deserialize, ToSchema)]
pub struct WalletBalanceRequest {
    pub account_xpub: String,
    #[serde(default)]
    pub first_n: Option<u32>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct WalletAddressBalance {
    pub index: u32,
    pub address: String,
    pub balance: u64,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct WalletBalanceResponse {
    pub balances: Vec<WalletAddressBalance>,
    pub total_balance: u64,
}

fn wallet_cache_key(account_xpub: &str, first_n: Option<u32>) -> String {
    match first_n {
        Some(n) => format!("{account_xpub}|n={n}"),
        None => format!("{account_xpub}|gap"),
    }
}

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
    let cache_key = wallet_cache_key(&payload.account_xpub, payload.first_n);
    if let Some(cached) = balance_cache::get_wallet(&cache_key) {
        if let Ok(body) = serde_json::from_str::<WalletBalanceResponse>(&cached) {
            return Ok(Json(body));
        }
    }

    let account_xpub = Arc::new(payload.account_xpub);
    let fixed_limit = payload.first_n.map(|n| n.min(MAX_SCAN));

    let mut balances = Vec::new();
    let mut total_balance = 0u64;
    let mut scanned = 0u32;
    // BIP-44: stop once we have scanned past last_funded + GAP_LIMIT.
    // `None` means no funded address yet — stop after the first GAP_LIMIT empties.
    let mut last_funded: Option<u32> = None;

    while scanned < MAX_SCAN {
        let stop_after = match fixed_limit {
            Some(limit) => limit.saturating_sub(1),
            None => last_funded
                .map(|i| i.saturating_add(GAP_LIMIT))
                .unwrap_or(GAP_LIMIT.saturating_sub(1)),
        };

        if scanned > stop_after {
            break;
        }

        let batch_end = (scanned + GAP_LIMIT).min(stop_after + 1).min(MAX_SCAN);
        if batch_end <= scanned {
            break;
        }

        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let address = derive_address_from_xpub(&account_xpub, index)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            handles.push((index, address.to_string(), tokio::spawn(get_balance_async(address))));
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
            balances.push(WalletAddressBalance {
                index,
                address,
                balance,
            });
        }

        scanned = batch_end;
    }

    let response = WalletBalanceResponse {
        balances,
        total_balance,
    };

    if let Ok(payload) = serde_json::to_string(&response) {
        balance_cache::put_wallet(cache_key, payload);
    }

    Ok(Json(response))
}
