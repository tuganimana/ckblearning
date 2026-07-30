//! Short-lived in-process balance cache.
//!
//! Wallet UIs poll every few seconds; without this, every refresh re-runs
//! indexer `collect_live_cells` for the same addresses.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(15);
const MAX_ENTRIES: usize = 4_096;

struct Entry {
    balance: u64,
    at: Instant,
}

static CACHE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

fn with_cache<R>(f: impl FnOnce(&mut HashMap<String, Entry>) -> R) -> R {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().expect("balance cache initialized"))
}

pub fn get(address: &str) -> Option<u64> {
    with_cache(|map| {
        let Some(entry) = map.get(address) else {
            return None;
        };
        if entry.at.elapsed() > TTL {
            map.remove(address);
            return None;
        }
        Some(entry.balance)
    })
}

pub fn put(address: String, balance: u64) {
    with_cache(|map| {
        if map.len() >= MAX_ENTRIES {
            // Drop expired first; if still full, clear (simple + safe).
            map.retain(|_, e| e.at.elapsed() <= TTL);
            if map.len() >= MAX_ENTRIES {
                map.clear();
            }
        }
        map.insert(
            address,
            Entry {
                balance,
                at: Instant::now(),
            },
        );
    });
}

/// Drop cached balances (e.g. after broadcast so the next read is fresh).
pub fn invalidate_all() {
    with_cache(|map| map.clear());
}
