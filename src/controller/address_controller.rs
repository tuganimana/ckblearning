use axum::{routing::post, Router};

use crate::services::{
    balance_service, dev_send_transaction_service, dev_wallet_balance_service,
    generate_address_service, generate_mnemonic_service, transaction_broadcast_service,
    transaction_build_service, transaction_history_service, wallet_address_service,
    wallet_balance_service,
};

pub fn address_router() -> Router {
    Router::new()
        // -- Non-custodial (production) API: never sees a private key --
        .route("/balance", post(balance_service::get_address_balance))
        .route(
            "/wallet/address",
            post(wallet_address_service::get_wallet_address),
        )
        .route(
            "/wallet/balance",
            post(wallet_balance_service::get_wallet_balance),
        )
        .route(
            "/transactions",
            post(transaction_history_service::get_transaction_history),
        )
        .route(
            "/transaction/build",
            post(transaction_build_service::build_transaction),
        )
        .route(
            "/transaction/broadcast",
            post(transaction_broadcast_service::broadcast),
        )
        // -- Dev/testing-only: disabled unless ALLOW_DEV_KEY_ENDPOINTS=true --
        .route(
            "/dev/generate-mnemonic",
            post(generate_mnemonic_service::generate_mnemonic),
        )
        .route(
            "/dev/generate-address",
            post(generate_address_service::generate_address),
        )
        .route(
            "/dev/balance/wallet",
            post(dev_wallet_balance_service::get_wallet_balance_dev),
        )
        .route(
            "/dev/transaction/send",
            post(dev_send_transaction_service::send_transaction_dev),
        )
}
