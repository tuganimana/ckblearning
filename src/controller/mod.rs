use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod address_controller;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::services::generate_mnemonic_service::generate_mnemonic,
        crate::services::generate_address_service::generate_address,
        crate::services::balance_service::get_address_balance,
        crate::services::balance_service::get_wallet_balance,
        crate::services::send_transaction_service::send_transaction,
        crate::services::transaction_history_service::get_transaction_history
    ),
    components(schemas(
        crate::services::generate_mnemonic_service::GeneratedMnemonic,
        crate::services::generate_address_service::GeneratedAddress,
        crate::services::generate_address_service::GenerateAddressRequest,
        crate::services::balance_service::BalanceRequest,
        crate::services::balance_service::BalanceResponse,
        crate::services::balance_service::WalletBalanceRequest,
        crate::services::balance_service::AddressBalance,
        crate::services::balance_service::WalletBalanceResponse,
        crate::services::send_transaction_service::SendTransactionRequest,
        crate::services::send_transaction_service::SendTransactionResponse,
        crate::services::transaction_history_service::TransactionHistoryRequest,
        crate::services::transaction_history_service::TransactionRecord,
        crate::services::transaction_history_service::TransactionHistoryResponse
    ))
)]
struct ApiDoc;

pub fn app_router() -> Router {
    Router::new()
        .merge(address_controller::address_router())
        // Nests wallet routes under a dedicated prefix (/api/v1/wallets/generate)
        // .nest("/api/v1/wallets", wallet::router())
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
}