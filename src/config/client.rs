
use::dotenvy::dotenv;
use std::env;
use ckb_sdk_rs::rpc::client::Client as CkbRpcClient;

pub fn connect_client(){
    dotenv().ok();
    let network = env::var("CKB_NETWORK")
        .unwrap_or_else(|_| "testnet".to_string());

    let rpc_url = match network.as_str() {
        "devnet" => env::var("CKB_DEVNET_URL").unwrap(),
        "mainnet" => env::var("CKB_MAINNET_URL").unwrap(),
        _ => env::var("CKB_TESTNET_URL").unwrap(),
    };

// Connect to Testnet
let mut ckb_client = CkbRpcClient::new(rpc_url);
println!("Connected to CKB {} network", network);
}