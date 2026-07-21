use std::str::FromStr;

use axum::{extract::Json, http::StatusCode};
use ckb_sdk::Address;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::wallet_scan::get_balance_async;

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
/// read -- never needs a mnemonic or private key.
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
