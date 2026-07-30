//! Short-lived in-process balance caches.
//!
//! Wallet UIs poll every few seconds; without this, every refresh re-runs
//! indexer RPCs for the same addresses / xpubs.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(20);
const MAX_ENTRIES: usize = 4_096;
const MAX_WALLET_ENTRIES: usize = 512;

struct Entry {
    balance: u64,
    at: Instant,
}

struct WalletEntry {
    payload: String,
    at: Instant,
}

static CACHE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);
static WALLET_CACHE: Mutex<Option<HashMap<String, WalletEntry>>> = Mutex::new(None);

fn with_cache<R>(f: impl FnOnce(&mut HashMap<String, Entry>) -> R) -> R {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().expect("balance cache initialized"))
}

fn with_wallet_cache<R>(f: impl FnOnce(&mut HashMap<String, WalletEntry>) -> R) -> R {
    let mut guard = WALLET_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().expect("wallet cache initialized"))
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

/// Full `/wallet/balance` JSON body, keyed by `account_xpub` (+ optional `first_n`).
pub fn get_wallet(key: &str) -> Option<String> {
    with_wallet_cache(|map| {
        let Some(entry) = map.get(key) else {
            return None;
        };
        if entry.at.elapsed() > TTL {
            map.remove(key);
            return None;
        }
        Some(entry.payload.clone())
    })
}

pub fn put_wallet(key: String, payload: String) {
    with_wallet_cache(|map| {
        if map.len() >= MAX_WALLET_ENTRIES {
            map.retain(|_, e| e.at.elapsed() <= TTL);
            if map.len() >= MAX_WALLET_ENTRIES {
                map.clear();
            }
        }
        map.insert(
            key,
            WalletEntry {
                payload,
                at: Instant::now(),
            },
        );
    });
}

/// Drop cached balances (e.g. after broadcast so the next read is fresh).
pub fn invalidate_all() {
    with_cache(|map| map.clear());
    with_wallet_cache(|map| map.clear());
}
