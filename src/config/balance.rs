use anyhow::Result;
use ckb_sdk::{
    traits::{CellCollector, CellQueryOptions, DefaultCellCollector},
    Address,
};
use ckb_types::prelude::*;

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