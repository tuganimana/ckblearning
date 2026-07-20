use anyhow::{anyhow, Result};
use ckb_sdk::{
    rpc::ckb_indexer::{CellType, Order, ScriptType, SearchKey, Tx},
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

    let records = page
        .objects
        .into_iter()
        .filter_map(|tx| match tx {
            Tx::Ungrouped(tx) => Some(TransactionRecord {
                tx_hash: tx.tx_hash,
                block_number: tx.block_number.value(),
                direction: match tx.io_type {
                    CellType::Output => "received",
                    CellType::Input => "sent",
                },
            }),
            // Shouldn't happen since we asked for group_by_transaction: Some(false).
            Tx::Grouped(_) => None,
        })
        .collect();

    Ok(records)
}