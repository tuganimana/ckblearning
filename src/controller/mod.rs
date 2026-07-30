use std::time::Duration;

use axum::{
    http::{header, HeaderValue, Method, StatusCode},
    Router,
};
use tower_governor::{governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod address_controller;

/// JSON bodies here are only ever a mnemonic/address/amount -- a few hundred
/// bytes at most. Cap well above that to block abusive oversized payloads.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Some endpoints (`/wallet/balance`) can scan many derived addresses;
/// give them room, but don't let a stuck connection hold a worker forever.
const REQUEST_TIMEOUT_SECS: u64 = 45;

/// Per-client-IP rate limit. Kept high enough that the Flutter app + Kaze
/// Python API (balance + history in parallel, plus deposit poller) are not
/// queued behind a 2/s ceiling — that queueing was the dominant multi-second
/// latency on warm endpoints.
const RATE_LIMIT_PER_SECOND: u64 = 20;
const RATE_LIMIT_BURST_SIZE: u32 = 40;

/// Restricts cross-origin browser access. Set `CORS_ALLOWED_ORIGINS` (a
/// comma-separated list, e.g. `https://app.example.com,https://example.com`)
/// before exposing this to browser-based frontends. Left permissive by
/// default since this API is equally meant for direct server-to-server or
/// native-app callers, which don't send an `Origin` header at all.
fn cors_layer() -> CorsLayer {
    match std::env::var("CORS_ALLOWED_ORIGINS") {
        Ok(origins) if !origins.trim().is_empty() => {
            let allowed_origins: Vec<HeaderValue> = origins
                .split(',')
                .filter_map(|origin| origin.trim().parse().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(allowed_origins)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
        }
        _ => {
            eprintln!(
                "CORS_ALLOWED_ORIGINS is not set: allowing requests from any origin. \
                 Set it to a comma-separated allowlist before exposing this to browsers."
            );
            CorsLayer::permissive()
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        // Non-custodial (production) API -- never touches a private key.
        crate::services::balance_service::get_address_balance,
        crate::services::wallet_address_service::get_wallet_address,
        crate::services::wallet_balance_service::get_wallet_balance,
        crate::services::transaction_history_service::get_transaction_history,
        crate::services::transaction_build_service::build_transaction,
        crate::services::transaction_broadcast_service::broadcast,
        // Fiber Network -- invoices, payments, CCH cross-chain swaps.
        crate::services::fiber_invoice_service::new_invoice,
        crate::services::fiber_invoice_service::parse_invoice,
        crate::services::fiber_invoice_service::get_invoice,
        crate::services::fiber_invoice_service::cancel_invoice,
        crate::services::fiber_invoice_service::settle_invoice,
        crate::services::fiber_payment_service::send_payment,
        crate::services::fiber_payment_service::get_payment,
        crate::services::fiber_swap_service::ckb_to_lightning,
        crate::services::fiber_swap_service::btc_to_ckb,
        crate::services::fiber_swap_service::get_cch_order,
        // Dev/testing-only -- disabled unless ALLOW_DEV_KEY_ENDPOINTS=true.
        crate::services::generate_mnemonic_service::generate_mnemonic,
        crate::services::generate_address_service::generate_address,
        crate::services::dev_wallet_balance_service::get_wallet_balance_dev,
        crate::services::dev_send_transaction_service::send_transaction_dev,
    ),
    components(schemas(
        crate::services::balance_service::BalanceRequest,
        crate::services::balance_service::BalanceResponse,
        crate::services::wallet_address_service::WalletAddressRequest,
        crate::services::wallet_address_service::WalletAddressResponse,
        crate::services::wallet_balance_service::WalletBalanceRequest,
        crate::services::wallet_balance_service::WalletAddressBalance,
        crate::services::wallet_balance_service::WalletBalanceResponse,
        crate::services::transaction_history_service::TransactionHistoryRequest,
        crate::services::transaction_history_service::TransactionRecord,
        crate::services::transaction_history_service::TransactionHistoryResponse,
        crate::services::transaction_build_service::BuildTransactionRequest,
        crate::services::transaction_build_service::BuildTransactionResponse,
        crate::services::transaction_broadcast_service::BroadcastTransactionRequest,
        crate::services::transaction_broadcast_service::BroadcastTransactionResponse,
        crate::services::fiber_invoice_service::NewFiberInvoiceRequest,
        crate::services::fiber_invoice_service::NewFiberInvoiceResponse,
        crate::services::fiber_invoice_service::ParseFiberInvoiceRequest,
        crate::services::fiber_invoice_service::ParseFiberInvoiceResponse,
        crate::services::fiber_invoice_service::GetFiberInvoiceRequest,
        crate::services::fiber_invoice_service::GetFiberInvoiceResponse,
        crate::services::fiber_invoice_service::CancelFiberInvoiceRequest,
        crate::services::fiber_invoice_service::SettleFiberInvoiceRequest,
        crate::services::fiber_invoice_service::SettleFiberInvoiceResponse,
        crate::services::fiber_payment_service::SendFiberPaymentRequest,
        crate::services::fiber_payment_service::SendFiberPaymentResponse,
        crate::services::fiber_payment_service::GetFiberPaymentRequest,
        crate::services::fiber_swap_service::CkbToLightningRequest,
        crate::services::fiber_swap_service::BtcToCkbRequest,
        crate::services::fiber_swap_service::GetCchOrderRequest,
        crate::services::fiber_swap_service::CchOrderResponse,
        crate::services::generate_mnemonic_service::DevGeneratedMnemonic,
        crate::services::generate_address_service::DevGeneratedAddress,
        crate::services::generate_address_service::DevGenerateAddressRequest,
        crate::services::dev_wallet_balance_service::DevWalletBalanceRequest,
        crate::services::dev_wallet_balance_service::DevAddressBalance,
        crate::services::dev_wallet_balance_service::DevWalletBalanceResponse,
        crate::services::dev_send_transaction_service::DevSendTransactionRequest,
        crate::services::dev_send_transaction_service::DevSendTransactionResponse,
    )),
    tags(
        (name = "fiber", description = "Fiber Network invoices, payments, and CKB↔Lightning CCH swaps")
    )
)]
struct ApiDoc;

pub fn app_router() -> Router {
    let mut governor_builder = GovernorConfigBuilder::default();
    governor_builder
        .per_second(RATE_LIMIT_PER_SECOND)
        .burst_size(RATE_LIMIT_BURST_SIZE);
    let governor_conf = governor_builder
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .expect("failed to build rate limiter config");

    // Rate limiting only applies to the wallet API itself, not the docs UI,
    // since loading Swagger UI legitimately fires off several requests at
    // once for its static assets.
    let api_routes = address_controller::address_router().layer(GovernorLayer::new(governor_conf));

    Router::new()
        .merge(api_routes)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors_layer())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
        ))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}