use dotenvy::dotenv;
use std::env;

use ckb_sdk::rpc::CkbRpcClient;

pub fn network() -> String {
    dotenv().ok();
    env::var("CKB_NETWORK").unwrap_or_else(|_| "testnet".to_string())
}

pub fn rpc_url() -> String {
    match network().as_str() {
        "devnet" => env::var("CKB_DEVNET_URL").unwrap(),
        "mainnet" => env::var("CKB_MAINNET_URL").unwrap(),
        _ => env::var("CKB_TESTNET_URL").unwrap(),
    }
}

pub fn connect_client() -> CkbRpcClient {
    let network = network();
    let url = rpc_url();
    let ckb_client = CkbRpcClient::new(url.as_str());
    println!("Connected to CKB {} network", network);
    ckb_client
}
