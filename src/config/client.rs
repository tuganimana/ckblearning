use std::env;

use ckb_sdk::{rpc::CkbRpcClient, types::NetworkType};

/// Active chain from `CKB_NETWORK` in `.env`: `testnet` | `mainnet` | `devnet`.
pub fn network() -> String {
    env::var("CKB_NETWORK").unwrap_or_else(|_| "testnet".to_string())
}

pub fn rpc_url() -> String {
    match network().as_str() {
        "devnet" => env::var("CKB_DEVNET_URL")
            .expect("CKB_DEVNET_URL must be set when CKB_NETWORK=devnet"),
        "mainnet" => env::var("CKB_MAINNET_URL")
            .expect("CKB_MAINNET_URL must be set when CKB_NETWORK=mainnet"),
        "testnet" => env::var("CKB_TESTNET_URL")
            .expect("CKB_TESTNET_URL must be set when CKB_NETWORK=testnet"),
        other => panic!("Unknown CKB_NETWORK={other:?}; use testnet, mainnet, or devnet"),
    }
}

/// The `NetworkType` that must be baked into every address we generate, so
/// addresses always match whichever chain `CKB_NETWORK` points the RPC/
/// indexer calls at. This must never be hardcoded elsewhere -- a mismatch
/// here means addresses are encoded for the wrong chain (e.g. `ckt1...`
/// testnet-formatted addresses while actually transacting on mainnet).
pub fn network_type() -> NetworkType {
    match network().as_str() {
        "devnet" => NetworkType::Dev,
        "mainnet" => NetworkType::Mainnet,
        "testnet" => NetworkType::Testnet,
        other => panic!("Unknown CKB_NETWORK={other:?}; use testnet, mainnet, or devnet"),
    }
}

pub fn connect_client() -> CkbRpcClient {
    CkbRpcClient::new(rpc_url().as_str())
}
