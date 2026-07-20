use ckb_jsonrpc_types as json_types;
use ckb_sdk::Address;
use std::str::FromStr;

mod config;
mod controller;
mod services;

use controller::app_router;

#[tokio::main]
async fn main() {
    
    let app = app_router();


    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
        .await
        .unwrap();

    println!("🚀 Production server running on http://127.0.0.1:5000");
    axum::serve(listener, app).await.unwrap()
}