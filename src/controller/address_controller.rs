use axum::{
    routing::{get, post},
    Router,
};

use crate::services::{
    balance_service, dev_send_transaction_service, dev_wallet_balance_service,
    fiber_invoice_service, fiber_payment_service, fiber_swap_service, generate_address_service,
    generate_mnemonic_service, transaction_broadcast_service, transaction_build_service,
    transaction_history_service, wallet_address_service, wallet_balance_service,
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
        // -- Fiber Network (invoices, payments, CCH swaps). Requires FIBER_RPC_URL. --
        .route("/fiber/invoice/new", post(fiber_invoice_service::new_invoice))
        .route(
            "/fiber/invoice/parse",
            post(fiber_invoice_service::parse_invoice),
        )
        .route("/fiber/invoice/get", post(fiber_invoice_service::get_invoice))
        .route(
            "/fiber/invoice/cancel",
            post(fiber_invoice_service::cancel_invoice),
        )
        .route(
            "/fiber/invoice/settle",
            post(fiber_invoice_service::settle_invoice),
        )
        .route(
            "/fiber/payment/send",
            post(fiber_payment_service::send_payment),
        )
        .route(
            "/fiber/payment/get",
            post(fiber_payment_service::get_payment),
        )
        .route(
            "/fiber/swap/fiber-to-lightning",
            post(fiber_swap_service::fiber_to_lightning),
        )
        .route(
            "/fiber/swap/ckb-to-lightning",
            post(fiber_swap_service::ckb_to_lightning),
        )
        .route(
            "/fiber/swap/pay-lightning",
            post(fiber_swap_service::pay_lightning),
        )
        .route(
            "/fiber/swap/receive-lightning",
            post(fiber_swap_service::receive_lightning),
        )
        .route(
            "/fiber/swap/btc-to-ckb",
            post(fiber_swap_service::btc_to_ckb),
        )
        .route("/fiber/swap/quote", post(fiber_swap_service::quote_swap))
        .route("/fiber/swap/wait", post(fiber_swap_service::wait_cch_order))
        .route("/fiber/swap/ready", get(fiber_swap_service::swap_ready))
        .route(
            "/fiber/swap/order/{payment_hash}",
            get(fiber_swap_service::get_cch_order_by_path),
        )
        .route(
            "/fiber/swap/order",
            post(fiber_swap_service::get_cch_order),
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
