use axum::{extract::Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::wallet_scan::{get_balance_async, GAP_LIMIT, MAX_SCAN};

use super::xpub_wallet::derive_address_from_xpub;

async fn find_next_index(account_xpub: &str) -> Result<u32, (StatusCode, String)> {
    let mut last_used_index: Option<u32> = None;
    let mut scanned = 0u32;

    while scanned < MAX_SCAN {
        // BIP-44: stop after last_funded + GAP_LIMIT (or first GAP empties).
        let stop_after = last_used_index
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
            let address =
                derive_address_from_xpub(account_xpub, index).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            handles.push((index, tokio::spawn(get_balance_async(address))));
        }

        for (index, handle) in handles {
            let balance = handle.await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Balance task panicked: {e}"),
                )
            })??;

            if balance > 0 {
                last_used_index = Some(index);
            }
        }

        scanned = batch_end;
    }

    Ok(last_used_index.map(|i| i + 1).unwrap_or(0))
}

#[derive(Deserialize, ToSchema)]
pub struct WalletAddressRequest {
    /// Account-level extended public key (BIP-32 xpub for `m/44'/309'/0'`),
    /// derived once on the client from the mnemonic and kept alongside it.
    /// Only ever send the *public* xpub here -- never the mnemonic or an
    /// extended private key.
    pub account_xpub: String,
    /// Address index to derive. Omit to auto-detect the next unused index
    /// by scanning the account's on-chain history.
    #[serde(default)]
    pub index: Option<u32>,
}

#[derive(Serialize, ToSchema)]
pub struct WalletAddressResponse {
    /// Derivation index this address came from (m/44'/309'/0'/0/{index}).
    pub index: u32,
    pub address: String,
}

/// Derives a CKB address from an **account xpub** -- no mnemonic or private
/// key ever needed. This is the non-custodial replacement for the old
/// "generate address from mnemonic" flow: the mnemonic is generated and
/// kept entirely on the client, which derives the account key locally and
/// shares only its public half with this API.
#[utoipa::path(
    post,
    path = "/wallet/address",
    request_body = WalletAddressRequest,
    responses(
        (status = 200, description = "Address for the given account xpub/index (next unused index if omitted)", body = WalletAddressResponse),
        (status = 400, description = "Invalid extended public key or derivation path"),
        (status = 500, description = "Failed to fetch balances while scanning for the next unused index")
    )
)]
pub async fn get_wallet_address(
    Json(payload): Json<WalletAddressRequest>,
) -> Result<Json<WalletAddressResponse>, (StatusCode, String)> {
    let index = match payload.index {
        Some(index) => index,
        None => find_next_index(&payload.account_xpub).await?,
    };

    let address = derive_address_from_xpub(&payload.account_xpub, index)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(WalletAddressResponse {
        index,
        address: address.to_string(),
    }))
}
