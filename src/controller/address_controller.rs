use axum::{routing::post, Router};

use crate::services::{
    balance_service, generate_address_service, generate_mnemonic_service, send_transaction_service,
    transaction_history_service,
};

pub fn address_router() -> Router {
    Router::new()
        .route(
            "/generate-mnemonic",
            post(generate_mnemonic_service::generate_mnemonic),
        )
        .route(
            "/generate-address",
            post(generate_address_service::generate_address),
        )
        .route("/balance", post(balance_service::get_address_balance))
        .route("/balance/wallet", post(balance_service::get_wallet_balance))
        .route(
            "/transaction/send",
            post(send_transaction_service::send_transaction),
        )
        .route(
            "/transactions",
            post(transaction_history_service::get_transaction_history),
        )
}
