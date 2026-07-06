
use::dotenvy::dotenv;
use std::env;
use ckb_sdk::rpc::CkbRpcClient;
pub fn connect_client() -> CkbRpcClient {
    dotenv().ok();
    let network = env::var("CKB_NETWORK")
        .unwrap_or_else(|_| "testnet".to_string());

    let rpc_url: String = match network.as_str() {
        "devnet" => env::var("CKB_DEVNET_URL").unwrap(),
        "mainnet" => env::var("CKB_MAINNET_URL").unwrap(),
        _ => env::var("CKB_TESTNET_URL").unwrap(),
    };

// Connect to Testnet
let  ckb_client = CkbRpcClient::new(rpc_url.as_str());
println!("Connected to CKB {} network", network);
 ckb_client
}