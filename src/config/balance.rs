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

use super::client;

pub fn get_balance(address: &Address) -> Result<u64> {
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
    let fetch_limit = limit.saturating_mul(3).clamp(limit, 100);

    let page = ckb_client
        .get_transactions(search_key, Order::Desc, fetch_limit.into(), None)
        .map_err(|e| anyhow!("Failed to fetch transactions: {e}"))?;

    let entries: Vec<_> = page
        .objects
        .into_iter()
        .filter_map(|tx| match tx {
            Tx::Ungrouped(tx) => Some((tx.tx_hash, tx.block_number.value(), tx.io_index.value(), tx.io_type)),
            // Shouldn't happen since we asked for group_by_transaction: Some(false).
            Tx::Grouped(_) => None,
        })
        .collect();

    // Working out the amount takes 1-2 extra RPC round trips per entry (see
    // `resolve_amount`), so resolve them concurrently on plain OS threads
    // rather than one at a time.
    let cell_records = thread::scope(|scope| {
        entries
            .into_iter()
            .map(|(tx_hash, block_number, io_index, io_type)| {
                scope.spawn(move || {
                    let amount = resolve_amount(&tx_hash, io_index, &io_type)?;
                    Ok(TransactionRecord {
                        tx_hash,
                        block_number,
                        direction: match io_type {
                            CellType::Output => "received",
                            CellType::Input => "sent",
                        },
                        amount,
                    })
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| anyhow!("Transaction amount lookup thread panicked"))?
            })
            .collect::<Result<Vec<_>>>()
    })?;

    Ok(net_transactions_by_hash(cell_records, limit))
}

/// Collapse per-cell rows into one wallet-facing entry per `tx_hash`.
fn net_transactions_by_hash(cell_records: Vec<TransactionRecord>, limit: u32) -> Vec<TransactionRecord> {
    use std::collections::HashMap;

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

/// Resolves the capacity (in Shannons) of the cell referenced by
/// `tx_hash`/`io_index`. For an output cell that's simply the created cell's
/// own capacity; for an input cell it's the capacity of the earlier output
/// it spends, since a spent cell is no longer "live" and can't be looked up
/// directly.
fn resolve_amount(tx_hash: &H256, io_index: u32, io_type: &CellType) -> Result<u64> {
    match io_type {
        CellType::Output => get_output_capacity(tx_hash, io_index),
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
            get_output_capacity(&previous_output.tx_hash, previous_output.index.value())
        }
    }
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