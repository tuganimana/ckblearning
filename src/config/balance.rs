use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use ckb_sdk::{
    rpc::{
        ckb_indexer::{CellType, Order, ScriptType, SearchKey, Tx},
        ResponseFormatGetter,
    },
    traits::{CellCollector, CellQueryOptions, DefaultCellCollector},
    Address,
};
use ckb_types::{prelude::*, H256};

use super::{balance_cache, client};

/// Cap concurrent `get_transaction` workers so we don't stampede the node.
const TX_RESOLVE_CONCURRENCY: usize = 8;

pub fn get_balance(address: &Address) -> Result<u64> {
    let key = address.to_string();
    if let Some(cached) = balance_cache::get(&key) {
        return Ok(cached);
    }

    let url = client::rpc_url();
    let mut collector = DefaultCellCollector::new(url.as_str());

    let query = CellQueryOptions::new_lock(address.payload().into());
    let (cells, _) = collector.collect_live_cells(&query, true)?;

    let balance = cells
        .iter()
        .map(|cell| {
            let cap: u64 = cell.output.capacity().unpack();
            cap
        })
        .sum();

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
/// address. A send that spends an input and creates a change output back to
/// the same address is reported as a single "sent" row (not a fake "received"
/// for the change cell).
pub fn list_transactions(address: &Address, limit: u32) -> Result<Vec<TransactionRecord>> {
    let ckb_client = client::connect_client();

    let lock_script = ckb_types::packed::Script::from(address);
    let search_key = SearchKey {
        script: lock_script.into(),
        script_type: ScriptType::Lock,
        script_search_mode: None,
        filter: None,
        with_data: None,
        // Ungrouped: one indexer row per matching cell. We net them by tx_hash
        // below so wallet UIs see transfer amounts, not raw cell movements.
        group_by_transaction: Some(false),
    };

    // Fetch extra cell rows so netting still yields ~`limit` transactions.
    // Keep the multiplier modest — each cell can cost 1–2 get_transaction RPCs.
    let fetch_limit = limit.saturating_mul(2).clamp(limit, 40);

    let page = ckb_client
        .get_transactions(search_key, Order::Desc, fetch_limit.into(), None)
        .map_err(|e| anyhow!("Failed to fetch transactions: {e}"))?;

    let entries: Vec<_> = page
        .objects
        .into_iter()
        .filter_map(|tx| match tx {
            Tx::Ungrouped(tx) => Some((
                tx.tx_hash,
                tx.block_number.value(),
                tx.io_index.value(),
                tx.io_type,
            )),
            Tx::Grouped(_) => None,
        })
        .collect();

    // Share output capacities across cell resolutions in this request.
    let capacity_cache: Arc<Mutex<HashMap<(H256, u32), u64>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let cell_records = resolve_cells_bounded(entries, capacity_cache);
    Ok(net_transactions_by_hash(cell_records, limit))
}

fn resolve_cells_bounded(
    entries: Vec<(H256, u64, u32, CellType)>,
    capacity_cache: Arc<Mutex<HashMap<(H256, u32), u64>>>,
) -> Vec<TransactionRecord> {
    let mut cell_records = Vec::with_capacity(entries.len());
    for chunk in entries.chunks(TX_RESOLVE_CONCURRENCY) {
        thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(tx_hash, block_number, io_index, io_type)| {
                    let tx_hash = tx_hash.clone();
                    let block_number = *block_number;
                    let io_index = *io_index;
                    let io_type = io_type.clone();
                    let capacity_cache = Arc::clone(&capacity_cache);
                    scope.spawn(move || -> Option<TransactionRecord> {
                        match resolve_amount_cached(&tx_hash, io_index, &io_type, &capacity_cache)
                        {
                            Ok(amount) => Some(TransactionRecord {
                                tx_hash,
                                block_number,
                                direction: match io_type {
                                    CellType::Output => "received",
                                    CellType::Input => "sent",
                                },
                                amount,
                            }),
                            Err(e) => {
                                eprintln!(
                                    "skipping tx amount lookup {tx_hash}/{io_index:?}: {e:#}"
                                );
                                None
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                if let Ok(Some(record)) = handle.join() {
                    cell_records.push(record);
                }
            }
        });
    }
    cell_records
}

/// Collapse per-cell rows into one wallet-facing entry per `tx_hash`.
fn net_transactions_by_hash(
    cell_records: Vec<TransactionRecord>,
    limit: u32,
) -> Vec<TransactionRecord> {
    let mut order: Vec<H256> = Vec::new();
    let mut received: HashMap<H256, u64> = HashMap::new();
    let mut sent: HashMap<H256, u64> = HashMap::new();
    let mut block_numbers: HashMap<H256, u64> = HashMap::new();

    for record in cell_records {
        if !block_numbers.contains_key(&record.tx_hash) {
            order.push(record.tx_hash.clone());
            block_numbers.insert(record.tx_hash.clone(), record.block_number);
        }
        match record.direction {
            "received" => {
                *received.entry(record.tx_hash).or_insert(0) += record.amount;
            }
            _ => {
                *sent.entry(record.tx_hash).or_insert(0) += record.amount;
            }
        }
    }

    let mut netted = Vec::new();
    for tx_hash in order {
        let in_amount = *received.get(&tx_hash).unwrap_or(&0);
        let out_amount = *sent.get(&tx_hash).unwrap_or(&0);
        let (direction, amount) = if in_amount > out_amount {
            ("received", in_amount - out_amount)
        } else if out_amount > in_amount {
            ("sent", out_amount - in_amount)
        } else {
            continue;
        };
        netted.push(TransactionRecord {
            block_number: *block_numbers.get(&tx_hash).unwrap_or(&0),
            tx_hash,
            direction,
            amount,
        });
        if netted.len() as u32 >= limit {
            break;
        }
    }

    netted
}

fn resolve_amount_cached(
    tx_hash: &H256,
    io_index: u32,
    io_type: &CellType,
    capacity_cache: &Mutex<HashMap<(H256, u32), u64>>,
) -> Result<u64> {
    match io_type {
        CellType::Output => get_output_capacity_cached(tx_hash, io_index, capacity_cache),
        CellType::Input => {
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

            let input = tx
                .inner
                .inputs
                .get(io_index as usize)
                .ok_or_else(|| anyhow!("Input index {io_index} out of range for tx {tx_hash}"))?;

            let previous_output = &input.previous_output;
            get_output_capacity_cached(
                &previous_output.tx_hash,
                previous_output.index.value(),
                capacity_cache,
            )
        }
    }
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
