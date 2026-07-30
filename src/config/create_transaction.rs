use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use ckb_sdk::{
    constants::SIGHASH_TYPE_HASH,
    traits::{
        DefaultCellCollector, DefaultCellDepResolver, DefaultHeaderDepResolver,
        DefaultTransactionDependencyProvider, SecpCkbRawKeySigner, TransactionDependencyProvider,
    },
    tx_builder::{
        balance_tx_capacity, transfer::CapacityTransferBuilder, unlock_tx, CapacityBalancer,
        CapacityProvider, TxBuilder,
    },
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

fn placeholder_witness() -> WitnessArgs {
    WitnessArgs::new_builder()
        .lock(Some(Bytes::from(vec![0u8; 65])).pack())
        .build()
}

fn build_resolvers(
    url: &str,
) -> Result<(
    DefaultCellDepResolver,
    DefaultHeaderDepResolver,
    DefaultCellCollector,
    DefaultTransactionDependencyProvider,
)> {
    let ckb_client = client::connect_client();
    let cell_dep_resolver = {
        let genesis_block = ckb_client
            .get_block_by_number(0.into())
            .map_err(|e| anyhow!("Failed to fetch genesis block: {e}"))?
            .context("Genesis block not found")?;
        DefaultCellDepResolver::from_genesis(&BlockView::from(genesis_block))
            .map_err(|e| anyhow!("Failed to build cell dep resolver: {e}"))?
    };
    let header_dep_resolver = DefaultHeaderDepResolver::new(url);
    let cell_collector = DefaultCellCollector::new(url);
    let tx_dep_provider = DefaultTransactionDependencyProvider::new(url, 10);
    Ok((
        cell_dep_resolver,
        header_dep_resolver,
        cell_collector,
        tx_dep_provider,
    ))
}

/// Builds a fee-balanced capacity-transfer from one or more sender locks.
///
/// Multiple senders (HD receive indexes) are required when funds are split
/// across addresses — e.g. 100+100 CKB cannot pay a 100 CKB transfer from a
/// single lock (fee / 61 CKB change rules), but can when both locks fund the tx.
fn build_balanced_transfer(
    senders: &[Address],
    receiver: &Address,
    capacity: HumanCapacity,
) -> Result<TransactionView> {
    if senders.is_empty() {
        bail!("At least one sender address is required");
    }

    let placeholder = placeholder_witness();
    let provider_locks: Vec<(Script, WitnessArgs)> = senders
        .iter()
        .map(|sender| (Script::from(sender), placeholder.clone()))
        .collect();

    // Change returns to the first (typically richest) sender.
    let mut balancer = CapacityBalancer::new_with_provider(
        1000,
        CapacityProvider::new_simple(provider_locks),
    );
    balancer.change_lock_script = Some(Script::from(&senders[0]));

    let url = client::rpc_url();
    let (cell_dep_resolver, header_dep_resolver, mut cell_collector, tx_dep_provider) =
        build_resolvers(url.as_str())?;

    let output = CellOutput::new_builder()
        .lock(Script::from(receiver))
        .capacity(capacity.0)
        .build();
    let builder = CapacityTransferBuilder::new(vec![(output, Bytes::default())]);

    let base_tx = builder
        .build_base(
            &mut cell_collector,
            &cell_dep_resolver,
            &header_dep_resolver,
            &tx_dep_provider,
        )
        .map_err(|e| anyhow!("Failed to build transaction: {e}"))?;

    balance_tx_capacity(
        &base_tx,
        &balancer,
        &mut cell_collector,
        &tx_dep_provider,
        &cell_dep_resolver,
        &header_dep_resolver,
    )
    .map_err(|e| anyhow!("Failed to balance transaction capacity: {e}"))
}

/// Map each sighash script-group's first witness index → sender address string.
///
/// Clients use this to sign every input group when funds span multiple HD indexes.
pub fn signing_plan_for_tx(
    tx: &TransactionView,
    known_senders: &[Address],
) -> Result<Vec<(usize, String)>> {
    let url = client::rpc_url();
    let tx_dep_provider = DefaultTransactionDependencyProvider::new(url.as_str(), 10);

    let lock_to_address: HashMap<Script, String> = known_senders
        .iter()
        .map(|a| (Script::from(a), a.to_string()))
        .collect();

    let mut seen_locks = HashSet::new();
    let mut plan = Vec::new();

    for (idx, input) in tx.inputs().into_iter().enumerate() {
        let cell = tx_dep_provider
            .get_cell(&input.previous_output())
            .map_err(|e| anyhow!("Failed to resolve input cell for signing plan: {e}"))?;
        let lock = cell.lock();
        if !seen_locks.insert(lock.clone()) {
            continue;
        }
        let address = lock_to_address.get(&lock).cloned().ok_or_else(|| {
            anyhow!("Built transaction input lock does not match any provided sender address")
        })?;
        plan.push((idx, address));
    }

    if plan.is_empty() {
        bail!("Transaction has no inputs to sign");
    }
    Ok(plan)
}

/// Builds an unsigned, fee-balanced transfer ready for client-side signing.
pub fn build_unsigned_transaction(
    sender_address: &str,
    receiver_address: &str,
    amount: &str,
) -> Result<TransactionView> {
    build_unsigned_transaction_multi(&[sender_address.to_string()], receiver_address, amount)
        .map(|(tx, _)| tx)
}

/// Multi-sender variant. Returns `(tx, signing_plan)` where `signing_plan` is
/// `(witness_index, sender_address)` for each distinct lock group.
pub fn build_unsigned_transaction_multi(
    sender_addresses: &[String],
    receiver_address: &str,
    amount: &str,
) -> Result<(TransactionView, Vec<(usize, String)>)> {
    if sender_addresses.is_empty() {
        bail!("At least one sender address is required");
    }

    let mut senders = Vec::with_capacity(sender_addresses.len());
    let mut seen = HashSet::new();
    for addr in sender_addresses {
        let trimmed = addr.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        senders.push(
            Address::from_str(trimmed).map_err(|e| anyhow!("Invalid sender address: {e}"))?,
        );
    }
    if senders.is_empty() {
        bail!("At least one sender address is required");
    }

    let receiver =
        Address::from_str(receiver_address).map_err(|e| anyhow!("Invalid receiver address: {e}"))?;
    let capacity = HumanCapacity::from_str(amount).map_err(|e| anyhow!("Invalid amount: {e}"))?;

    let tx = build_balanced_transfer(&senders, &receiver, capacity)?;
    let plan = signing_plan_for_tx(&tx, &senders)?;
    Ok((tx, plan))
}

/// Parses a transaction a client has already signed (as CKB JSON) back into
/// a `TransactionView` so it can be broadcast.
pub fn parse_signed_transaction(json_tx: ckb_jsonrpc_types::Transaction) -> TransactionView {
    let packed_tx: ckb_types::packed::Transaction = json_tx.into();
    packed_tx.into_view()
}

/// Dev/testing convenience only: builds *and signs* a capacity-transfer
/// transaction server-side, given the sender's raw private key.
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

    let balanced_tx = build_balanced_transfer(&[sender], &receiver, capacity)?;

    let signer = SecpCkbRawKeySigner::new_with_secret_keys(vec![sender_key]);
    let sighash_unlocker = SecpSighashUnlocker::from(Box::new(signer) as Box<_>);
    let sighash_script_id = ScriptId::new_type(SIGHASH_TYPE_HASH.clone());
    let mut unlockers = HashMap::default();
    unlockers.insert(
        sighash_script_id,
        Box::new(sighash_unlocker) as Box<dyn ScriptUnlocker>,
    );

    let url = client::rpc_url();
    let tx_dep_provider = DefaultTransactionDependencyProvider::new(url.as_str(), 10);
    let (tx, still_locked_groups) = unlock_tx(balanced_tx, &tx_dep_provider, &unlockers)
        .map_err(|e| anyhow!("Failed to sign transaction: {e}"))?;

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
