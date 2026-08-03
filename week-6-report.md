## Builder Track Weekly Report — Week 6

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 03-08-2026

### Courses Completed

- **Wallet balance & history performance**
  - Added short-lived in-process balance cache (`balance_cache.rs`) for address + xpub wallet scans.
  - Switched balance reads to indexer `get_cells_capacity` (one RPC) instead of paging live cells.
  - Reworked transaction history with `group_by_transaction` so each wallet tx reports a single net amount.
  - Cached full `/wallet/balance` responses; invalidate cache after broadcast.

- **Multi-address (HD) transfers**
  - Extended transaction builder to fund a send from multiple sender locks (`build_unsigned_transaction_multi`).
  - Updated `/transaction/build` to accept `sender_addresses` and return a multi-signer plan.
  - Fixed the case where funds are split across receive indexes (fee / 61 CKB change rules) so the transfer can consolidate inputs.

- **API tuning for clients**
  - Raised per-IP rate limits so Flutter + Python (balance/history in parallel) are not queued.
  - Cleared up broadcast error messages when a signed tx fails to submit.

### Key Learnings

- **CKB indexing & capacity**
  - `get_cells_capacity` vs collecting every live cell for balance.
  - Netting sends/receives per tx hash (change cells should not look like a receive).
  - When one HD address cannot cover amount + fee, multiple locks must fund the same transfer.

- **API performance**
  - Short TTL caches cut repeated indexer RPCs from wallet polling.
  - Rate limits that are too low dominate latency even when the node is warm.

### Practical Progress

- Faster `/wallet/balance` and address balance via cache + capacity RPC.
- Transaction history returns correct consolidated (net) amounts.
- Multi-sender unsigned builds work for split HD balances; client signs each lock group.
- Deployed app + Fiber setup from week 5 still in place; this week focused on wallet/tx correctness and speed.

### Environment Setup

- Same DigitalOcean Droplet + Fiber RPC as week 5.
- Local/API work against the deployed CKB indexer for balance and history.
- Clients: Flutter wallet UI + Kaze Python API calling balance/history in parallel.
