mod config;
mod controller;
mod services;

use std::net::SocketAddr;

use controller::app_router;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Fail fast at startup rather than mid-request: this both validates
    // CKB_NETWORK and confirms the matching RPC URL env var is set.
    let network = config::client::network();
    let rpc_url = config::client::rpc_url();
    println!("CKB_NETWORK={network} (rpc: {rpc_url})");

    let dev_endpoints_enabled = std::env::var("ALLOW_DEV_KEY_ENDPOINTS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    if dev_endpoints_enabled {
        println!(
            "WARNING: ALLOW_DEV_KEY_ENDPOINTS=true -- /dev/* endpoints that generate mnemonics \
             and sign transactions server-side are ENABLED. Do not enable this in production."
        );
    } else {
        println!("/dev/* key-handling endpoints are disabled (set ALLOW_DEV_KEY_ENDPOINTS=true to enable for local testing)");
    }

    match config::fiber_client::fiber_rpc_url() {
        Some(url) => {
            println!(
                "Fiber RPC enabled (FIBER_RPC_URL={url}, currency={})",
                config::fiber_client::fiber_currency()
            );
        }
        None => {
            println!(
                "/fiber/* endpoints require FIBER_RPC_URL (unset -- Fiber calls will return 503)"
            );
        }
    }

    let app = app_router();

    let port = std::env::var("PORT").unwrap_or_else(|_| "5000".to_string());
    let addr = format!("0.0.0.0:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {addr}: {e}"));

    println!("Server listening on http://{addr}");

    // `with_connect_info` lets the rate limiter fall back to the peer's
    // socket address when no X-Forwarded-For/X-Real-Ip header is present
    // (e.g. local development, or a direct connection with no proxy).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap_or_else(|e| panic!("Server error: {e}"));
}
