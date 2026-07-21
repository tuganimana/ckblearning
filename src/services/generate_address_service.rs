use axum::{extract::Json, http::StatusCode};
use bip39::{Language, Mnemonic};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::dev_guard;
use super::wallet::derive_wallet;

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
pub struct DevGeneratedAddress {
    pub mnemonic: String,
    /// Derivation index this address came from (m/44'/309'/0'/0/{index}).
    pub index: u32,
    pub address: String,
    pub public_key: String,
    pub private_key: String,
}

#[derive(Deserialize, ToSchema)]
pub struct DevGenerateAddressRequest {
    #[serde(default)]
    pub mnemonic: Option<String>,

    #[serde(default)]
    pub index: Option<u32>,
}

/// **Dev/testing only** -- disabled unless `ALLOW_DEV_KEY_ENDPOINTS=true`.
/// Generates/derives an address *and returns its private key*. Use
/// `/wallet/address` (account xpub, no private key) for the real,
/// non-custodial flow.
#[utoipa::path(
    post,
    path = "/dev/generate-address",
    request_body = DevGenerateAddressRequest,
    responses(
        (status = 200, description = "A new wallet, or the address for the given mnemonic/index", body = DevGeneratedAddress),
        (status = 400, description = "Invalid mnemonic or derivation path"),
        (status = 403, description = "Disabled: set ALLOW_DEV_KEY_ENDPOINTS=true to enable for local testing"),
        (status = 500, description = "Failed to fetch balances while scanning for the next unused index")
    )
)]
pub async fn generate_address(
    Json(payload): Json<DevGenerateAddressRequest>,
) -> Result<Json<DevGeneratedAddress>, (StatusCode, String)> {
    dev_guard::require_enabled()?;

    let (mnemonic, index) = match (payload.mnemonic, payload.index) {
        (Some(mnemonic), Some(index)) => (mnemonic, index),
        (Some(mnemonic), None) => {
            let index = find_next_index(&mnemonic).await?;
            (mnemonic, index)
        }
        (None, _) => {
            let mnemonic = Mnemonic::generate_in(Language::English, 12).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to generate mnemonic: {e}"),
                )
            })?;
            (mnemonic.to_string(), 0)
        }
    };

    let wallet = derive_wallet(&mnemonic, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(DevGeneratedAddress {
        mnemonic,
        index,
        address: wallet.address.to_string(),
        public_key: wallet.public_key,
        private_key: wallet.private_key,
    }))
}
