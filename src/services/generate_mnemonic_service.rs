use axum::{http::StatusCode, Json};
use bip39::{Language, Mnemonic};
use serde::Serialize;
use utoipa::ToSchema;

use super::dev_guard;

#[derive(Serialize, ToSchema)]
pub struct DevGeneratedMnemonic {
    pub mnemonic: String,
}

/// **Dev/testing only** -- disabled unless `ALLOW_DEV_KEY_ENDPOINTS=true`.
/// A real wallet must generate its mnemonic on the user's own device; a
/// server that can generate (and thus see) your seed phrase is not
/// self-custodial. Kept only for quick local iteration without a
/// client-side signer.
#[utoipa::path(
    post,
    path = "/dev/generate-mnemonic",
    responses(
        (status = 200, description = "Generated a fresh BIP-39 recovery phrase", body = DevGeneratedMnemonic),
        (status = 403, description = "Disabled: set ALLOW_DEV_KEY_ENDPOINTS=true to enable for local testing"),
        (status = 500, description = "Failed to generate a mnemonic")
    )
)]
pub async fn generate_mnemonic() -> Result<Json<DevGeneratedMnemonic>, (StatusCode, String)> {
    dev_guard::require_enabled()?;

    let mnemonic = Mnemonic::generate_in(Language::English, 12).map_err(|e| {
        eprintln!("failed to generate mnemonic: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate a mnemonic. Please try again.".to_string(),
        )
    })?;

    Ok(Json(DevGeneratedMnemonic {
        mnemonic: mnemonic.to_string(),
    }))
}
