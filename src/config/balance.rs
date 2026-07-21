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

/// One cell that was created at ("received") or spent from ("sent") this
/// address, as reported by the CKB node's indexer.
pub struct TransactionRecord {
    pub tx_hash: H256,
    pub block_number: u64,
    /// "received" when a cell locked to this address was created by this
    /// transaction, "sent" when one was spent as an input.
    pub direction: &'static str,
    /// The cell's capacity in Shannons: the amount received (for an output
    /// cell) or spent (for an input cell).
    pub amount: u64,
}

/// Lists the most recent transactions that touched `address`, newest first.
/// This is how you find out a payment actually arrived (and its tx hash),
/// rather than only ever seeing the current lump balance: poll this (or
/// `get_balance`) periodically and look for new "received" entries you
/// haven't seen before.
pub fn list_transactions(address: &Address, limit: u32) -> Result<Vec<TransactionRecord>> {
    let ckb_client = client::connect_client();

    let lock_script = ckb_types::packed::Script::from(address);
    let search_key = SearchKey {
        script: lock_script.into(),
        script_type: ScriptType::Lock,
        script_search_mode: None,
        filter: None,
        with_data: None,
        // Explicitly ungrouped: one row per matching cell, each with its own
        // io_type, instead of one row per transaction with a list of cells.
        group_by_transaction: Some(false),
    };

    let page = ckb_client
        .get_transactions(search_key, Order::Desc, limit.into(), None)
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
    let records = thread::scope(|scope| {
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

    Ok(records)
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