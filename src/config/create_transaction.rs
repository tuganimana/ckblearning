use std::collections::HashMap;
use std::str::FromStr;

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
};

use super::client;

pub fn create_transaction(
    sender_address: &str,
    sender_private_key: &str,
    receiver_address: &str,
    amount: &str,
) -> TransactionView {
    let sender = Address::from_str(sender_address).expect("invalid sender address");
    let sender_key = secp256k1::SecretKey::from_slice(
        &hex::decode(sender_private_key.trim_start_matches("0x"))
            .expect("invalid private key hex"),
    )
    .expect("invalid private key");

    let receiver = Address::from_str(receiver_address).expect("invalid receiver address");
    let capacity = HumanCapacity::from_str(amount).expect("invalid amount");

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
        let genesis_block = ckb_client.get_block_by_number(0.into()).unwrap().unwrap();
        DefaultCellDepResolver::from_genesis(&BlockView::from(genesis_block)).unwrap()
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
        .unwrap();
    assert!(still_locked_groups.is_empty());

    tx
}
