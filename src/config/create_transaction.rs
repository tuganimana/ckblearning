use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use ckb_sdk::{
    constants::SIGHASH_TYPE_HASH,
    traits::{
        DefaultCellCollector, DefaultCellDepResolver, DefaultHeaderDepResolver,
        DefaultTransactionDependencyProvider, SecpCkbRawKeySigner,
    },
    tx_builder::{transfer::CapacityTransferBuilder, CapacityBalancer, TxBuilder},
    unlock::{ScriptUnlocker, SecpSighashUnlocker},
    Address, HumanCapacity, ScriptId,
};
use ckb_types::{
    bytes::Bytes,
    core::{BlockView, TransactionView},
    packed::{CellOutput, Script, WitnessArgs},
    prelude::*,
    H256,
};

use super::client;

/// Builds and signs a capacity-transfer transaction from `sender_address`
/// (unlocked with `sender_private_key`) to `receiver_address`. Does not
/// broadcast it -- see `broadcast_transaction` for that.
pub fn create_transaction(
    sender_address: &str,
    sender_private_key: &str,
    receiver_address: &str,
    amount: &str,
) -> Result<TransactionView> {
    let sender = Address::from_str(sender_address).map_err(|e| anyhow!("Invalid sender address: {e}"))?;
    let sender_key = secp256k1::SecretKey::from_slice(
        &hex::decode(sender_private_key.trim_start_matches("0x")).context("Invalid private key hex")?,
    )
    .context("Invalid private key")?;

    let receiver =
        Address::from_str(receiver_address).map_err(|e| anyhow!("Invalid receiver address: {e}"))?;
    let capacity = HumanCapacity::from_str(amount).map_err(|e| anyhow!("Invalid amount: {e}"))?;

    let signer = SecpCkbRawKeySigner::new_with_secret_keys(vec![sender_key]);
    let sighash_unlocker = SecpSighashUnlocker::from(Box::new(signer) as Box<_>);
    let sighash_script_id = ScriptId::new_type(SIGHASH_TYPE_HASH.clone());
    let mut unlockers = HashMap::default();
    unlockers.insert(
        sighash_script_id,
        Box::new(sighash_unlocker) as Box<dyn ScriptUnlocker>,
    );

    let placeholder_witness = WitnessArgs::new_builder()
        .lock(Some(Bytes::from(vec![0u8; 65])).pack())
        .build();
    let balancer =
        CapacityBalancer::new_simple(sender.payload().into(), placeholder_witness, 1000);

    let url = client::rpc_url();
    let ckb_client = client::connect_client();
    let cell_dep_resolver = {
        let genesis_block = ckb_client
            .get_block_by_number(0.into())
            .map_err(|e| anyhow!("Failed to fetch genesis block: {e}"))?
            .context("Genesis block not found")?;
        DefaultCellDepResolver::from_genesis(&BlockView::from(genesis_block))
            .map_err(|e| anyhow!("Failed to build cell dep resolver: {e}"))?
    };
    let header_dep_resolver = DefaultHeaderDepResolver::new(url.as_str());
    let mut cell_collector = DefaultCellCollector::new(url.as_str());
    let tx_dep_provider = DefaultTransactionDependencyProvider::new(url.as_str(), 10);

    let output = CellOutput::new_builder()
        .lock(Script::from(&receiver))
        .capacity(capacity.0)
        .build();
    let builder = CapacityTransferBuilder::new(vec![(output, Bytes::default())]);
    let (tx, still_locked_groups) = builder
        .build_unlocked(
            &mut cell_collector,
            &cell_dep_resolver,
            &header_dep_resolver,
            &tx_dep_provider,
            &balancer,
            &unlockers,
        )
        .map_err(|e| anyhow!("Failed to build transaction: {e}"))?;

    if !still_locked_groups.is_empty() {
        bail!("Failed to unlock all script groups on the transaction");
    }

    Ok(tx)
}

/// Submits an already-built, signed transaction to the CKB node's tx pool
/// and returns the resulting transaction hash.
pub fn broadcast_transaction(tx: &TransactionView) -> Result<H256> {
    let ckb_client = client::connect_client();
    let json_tx = ckb_jsonrpc_types::Transaction::from(tx.data());

    ckb_client
        .send_transaction(json_tx, None)
        .map_err(|e| anyhow!("Failed to broadcast transaction: {e}"))
}
