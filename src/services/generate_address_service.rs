use axum::{extract::Json, http::StatusCode};
use bip39::{Language, Mnemonic};
use ckb_sdk::Address;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::balance::get_balance;

use super::wallet::derive_wallet;
const GAP_LIMIT: u32 = 20;

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

async fn find_next_index(mnemonic: &str) -> Result<u32, (StatusCode, String)> {
    let mut last_used_index: Option<u32> = None;
    let mut scanned = 0u32;

    while scanned < MAX_SCAN {
        let batch_end = (scanned + GAP_LIMIT).min(MAX_SCAN);

        let mut handles = Vec::with_capacity((batch_end - scanned) as usize);
        for index in scanned..batch_end {
            let wallet = derive_wallet(mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            handles.push((index, tokio::spawn(get_balance_async(wallet.address))));
        }

        let mut batch_had_funds = false;
        for (index, handle) in handles {
            let balance = handle.await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Balance task panicked: {e}"),
                )
            })??;

            if balance > 0 {
                batch_had_funds = true;
                last_used_index = Some(index);
            }
        }

        scanned = batch_end;

        if !batch_had_funds {
            break;
        }
    }

    Ok(last_used_index.map(|i| i + 1).unwrap_or(0))
}

#[derive(Serialize, ToSchema)]
pub struct GeneratedAddress {
    pub mnemonic: String,
    /// Derivation index this address came from (m/44'/309'/0'/0/{index}).
    pub index: u32,
    pub address: String,
    pub public_key: String,
    pub private_key: String, // i will remove it on prod
}

#[derive(Deserialize, ToSchema)]
pub struct GenerateAddressRequest {
    #[serde(default)]
    pub mnemonic: Option<String>,
  
    #[serde(default)]
    pub index: Option<u32>,
}

#[utoipa::path(
    post,
    path = "/generate-address",
    request_body = GenerateAddressRequest,
    responses(
        (status = 200, description = "A new wallet, or the address for the given mnemonic/index", body = GeneratedAddress),
        (status = 400, description = "Invalid mnemonic or derivation path"),
        (status = 500, description = "Failed to fetch balances while scanning for the next unused index")
    )
)]
pub async fn generate_address(
    Json(payload): Json<GenerateAddressRequest>,
) -> Result<Json<GeneratedAddress>, (StatusCode, String)> {
    let (mnemonic, index) = match (payload.mnemonic, payload.index) {
        (Some(mnemonic), Some(index)) => (mnemonic, index),
        (Some(mnemonic), None) => {
            let index = find_next_index(&mnemonic).await?;
            (mnemonic, index)
        }
        (None, _) => (Mnemonic::generate_in(Language::English, 12).unwrap().to_string(), 0),
    };

    let wallet = derive_wallet(&mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(GeneratedAddress {
        mnemonic,
        index,
        address: wallet.address.to_string(),
        public_key: wallet.public_key,
        private_key: wallet.private_key,
    }))
}
