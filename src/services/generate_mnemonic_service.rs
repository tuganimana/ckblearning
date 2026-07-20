use axum::Json;
use bip39::{Language, Mnemonic};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct GeneratedMnemonic {
    pub mnemonic: String,
}


#[utoipa::path(
    post,
    path = "/generate-mnemonic",
    responses(
        (status = 200, description = "Generated a fresh BIP-39 recovery phrase", body = GeneratedMnemonic)
    )
)]
pub async fn generate_mnemonic() -> Json<GeneratedMnemonic> {
    let mnemonic = Mnemonic::generate_in(Language::English, 12).unwrap();

    Json(GeneratedMnemonic {
        mnemonic: mnemonic.to_string(),
    })
}
