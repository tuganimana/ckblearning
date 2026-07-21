use axum::http::StatusCode;

/// Dev/testing-only endpoints (server-side mnemonic generation, and
/// server-side signing given a mnemonic) are disabled unless this is
/// explicitly set. This keeps a production deployment from ever exposing a
/// route that touches a private key, while still allowing quick local
/// iteration without a client-side signer.
pub fn require_enabled() -> Result<(), (StatusCode, String)> {
    let enabled = std::env::var("ALLOW_DEV_KEY_ENDPOINTS")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false);

    if enabled {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "This endpoint handles private keys server-side and is disabled in this deployment. \
             Use the non-custodial endpoints instead (/wallet/address, /wallet/balance, \
             /transaction/build + /transaction/broadcast), or set ALLOW_DEV_KEY_ENDPOINTS=true \
             for local/dev testing only."
                .to_string(),
        ))
    }
}
