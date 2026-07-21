use axum::http::StatusCode;
use ckb_sdk::Address;

use super::balance::get_balance;

/// How many consecutive unused addresses we tolerate before assuming the
pub const GAP_LIMIT: u32 = 20;

pub const MAX_SCAN: u32 = 10_000;
pub async fn get_balance_async(address: Address) -> Result<u64, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || get_balance(&address))
        .await
        .map_err(|e| {
            eprintln!("balance task panicked: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch balance. Please try again.".to_string(),
            )
        })?
        .map_err(|e| {
            eprintln!("failed to fetch balance for an address: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch balance. Please try again.".to_string(),
            )
        })
}
