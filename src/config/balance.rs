use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use ckb_sdk::{
    rpc::{
        ckb_indexer::{CellType, Order, ScriptType, SearchKey, Tx},
        ResponseFormatGetter,
    },
    Address,
};
use ckb_types::H256;

use super::{balance_cache, client};

/// Cap concurrent `get_transaction` workers so we don't stampede the node.
const TX_RESOLVE_CONCURRENCY: usize = 12;

fn lock_search_key(address: &Address) -> SearchKey {
    let lock_script = ckb_types::packed::Script::from(address);
    SearchKey {
        script: lock_script.into(),
        script_type: ScriptType::Lock,
        script_search_mode: None,
        filter: None,
        // Capacity-only lookups never need cell data.
        with_data: Some(false),
        group_by_transaction: None,
    }
}

/// Live capacity for `address` in Shannons.
///
/// Uses the indexer `get_cells_capacity` RPC (one round-trip) instead of
/// paging every live cell via `collect_live_cells`.
pub fn get_balance(address: &Address) -> Result<u64> {
    let key = address.to_string();
    if let Some(cached) = balance_cache::get(&key) {
        return Ok(cached);
    }

    let ckb_client = client::connect_client();
    let balance = match ckb_client
        .get_cells_capacity(lock_search_key(address))
        .map_err(|e| anyhow!("Failed to fetch capacity: {e}"))?
    {
        Some(cells) => cells.capacity.value(),
        None => 0,
    };

    balance_cache::put(key, balance);
    Ok(balance)
}

/// One netted wallet activity row for an address.
///
/// Built from indexer cell movements: inputs ("sent") and outputs
/// ("received") for the same `tx_hash` are summed, then reported as a
/// single direction + amount (so change-back cells don't appear as
/// separate receives).
pub struct TransactionRecord {
    pub tx_hash: H256,
    pub block_number: u64,
    /// "received" when net capacity for this address increased,
    /// "sent" when it decreased.
    pub direction: &'static str,
    /// Net capacity change in Shannons for this address in the tx.
    pub amount: u64,
}

/// Lists the most recent transactions that touched `address`, newest first.
///
/// One entry **per transaction**, with the **net** capacity change for this
/// address. Uses `group_by_transaction` so each wallet tx is one indexer row
/// and typically one `get_transaction` round-trip (plus previous-output
/// lookups for spent inputs).
pub fn list_transactions(address: &Address, limit: u32) -> Result<Vec<TransactionRecord>> {
    let ckb_client = client::connect_client();

    let mut search_key = lock_search_key(address);
    // One indexer row per wallet transaction (all matching cells nested).
    search_key.group_by_transaction = Some(true);

    let page = ckb_client
        .get_transactions(search_key, Order::Desc, limit.into(), None)
        .map_err(|e| anyhow!("Failed to fetch transactions: {e}"))?;

    let groups: Vec<_> = page
        .objects
        .into_iter()
        .filter_map(|tx| match tx {
            Tx::Grouped(tx) => Some((
                tx.tx_hash,
                tx.block_number.value(),
                tx.cells
                    .into_iter()
                    .map(|(io_type, io_index)| (io_type, io_index.value()))
                    .collect::<Vec<_>>(),
            )),
            // Defensive: some nodes may still return ungrouped rows.
            Tx::Ungrouped(tx) => Some((
                tx.tx_hash,
                tx.block_number.value(),
                vec![(tx.io_type, tx.io_index.value())],
            )),
        })
        .collect();

    let capacity_cache: Arc<Mutex<HashMap<(H256, u32), u64>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut records = Vec::with_capacity(groups.len());
    for chunk in groups.chunks(TX_RESOLVE_CONCURRENCY) {
        thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(tx_hash, block_number, cells)| {
                    let tx_hash = tx_hash.clone();
                    let block_number = *block_number;
                    let cells = cells.clone();
                    let capacity_cache = Arc::clone(&capacity_cache);
                    scope.spawn(move || -> Option<TransactionRecord> {
                        match net_grouped_tx(&tx_hash, block_number, &cells, &capacity_cache) {
                            Ok(record) => record,
                            Err(e) => {
                                eprintln!("skipping tx {tx_hash}: {e:#}");
                                None
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                if let Ok(Some(record)) = handle.join() {
                    records.push(record);
                }
            }
        });
    }

    Ok(records)
}

/// Resolve one grouped indexer row into a single netted wallet record.
fn net_grouped_tx(
    tx_hash: &H256,
    block_number: u64,
    cells: &[(CellType, u32)],
    capacity_cache: &Mutex<HashMap<(H256, u32), u64>>,
) -> Result<Option<TransactionRecord>> {
    if cells.is_empty() {
        return Ok(None);
    }

    // Fetch the wallet-touching tx once; all Output capacities live here,
    // and Input previous_outputs are listed here too.
    let ckb_client = client::connect_client();
    let resp = ckb_client
        .get_transaction(tx_hash.clone())
        .map_err(|e| anyhow!("Failed to fetch transaction {tx_hash}: {e}"))?
        .ok_or_else(|| anyhow!("Transaction {tx_hash} not found"))?;

    let tx = resp
        .transaction
        .ok_or_else(|| anyhow!("Transaction {tx_hash} has no data"))?
        .get_value()
        .map_err(|e| anyhow!("Failed to decode transaction {tx_hash}: {e}"))?;

    let mut received = 0u64;
    let mut sent = 0u64;

    for (io_type, io_index) in cells {
        match io_type {
            CellType::Output => {
                let output = tx
                    .inner
                    .outputs
                    .get(*io_index as usize)
                    .ok_or_else(|| {
                        anyhow!("Output index {io_index} out of range for tx {tx_hash}")
                    })?;
                let capacity = output.capacity.value();
                capacity_cache
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert((tx_hash.clone(), *io_index), capacity);
                received = received.saturating_add(capacity);
            }
            CellType::Input => {
                let input = tx
                    .inner
                    .inputs
                    .get(*io_index as usize)
                    .ok_or_else(|| {
                        anyhow!("Input index {io_index} out of range for tx {tx_hash}")
                    })?;
                let previous_output = &input.previous_output;
                let capacity = get_output_capacity_cached(
                    &previous_output.tx_hash,
                    previous_output.index.value(),
                    capacity_cache,
                )?;
                sent = sent.saturating_add(capacity);
            }
        }
    }

    let (direction, amount) = if received > sent {
        ("received", received - sent)
    } else if sent > received {
        ("sent", sent - received)
    } else {
        return Ok(None);
    };

    Ok(Some(TransactionRecord {
        tx_hash: tx_hash.clone(),
        block_number,
        direction,
        amount,
    }))
}

fn get_output_capacity_cached(
    tx_hash: &H256,
    index: u32,
    capacity_cache: &Mutex<HashMap<(H256, u32), u64>>,
) -> Result<u64> {
    let key = (tx_hash.clone(), index);
    if let Some(hit) = capacity_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .copied()
    {
        return Ok(hit);
    }

    let capacity = get_output_capacity(tx_hash, index)?;
    capacity_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, capacity);
    Ok(capacity)
}

/// Fetches `tx_hash` and returns the capacity of its output at `index`.
fn get_output_capacity(tx_hash: &H256, index: u32) -> Result<u64> {
    let ckb_client = client::connect_client();
    let resp = ckb_client
        .get_transaction(tx_hash.clone())
        .map_err(|e| anyhow!("Failed to fetch transaction {tx_hash}: {e}"))?
        .ok_or_else(|| anyhow!("Transaction {tx_hash} not found"))?;

    let tx = resp
        .transaction
        .ok_or_else(|| anyhow!("Transaction {tx_hash} has no data"))?
        .get_value()
        .map_err(|e| anyhow!("Failed to decode transaction {tx_hash}: {e}"))?;

    let output = tx
        .inner
        .outputs
        .get(index as usize)
        .ok_or_else(|| anyhow!("Output index {index} out of range for tx {tx_hash}"))?;

    Ok(output.capacity.value())
}
