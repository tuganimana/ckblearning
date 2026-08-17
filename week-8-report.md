## Builder Track Weekly Report — Week 8

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 17-08-2026

### Courses Completed

- **Fiber Network Review**
  - Reviewed the Fiber Network architecture and its role within the CKB ecosystem.
  - Explored Fiber's payment channel architecture and how it enables fast and scalable off-chain payments.
  - Reviewed Fiber CCH (`send_btc` / `receive_btc` / `get_cch_order`) for atomic Fiber ↔ Lightning swaps.
  - Evaluated how Fiber CCH could complement Kaze wallet and payment infrastructure.

- **RGB++ Exploration**
  - Started exploring RGB++ and its approach to bringing smart-contract capabilities and asset issuance to the CKB ecosystem.
  - Reviewed the relationship between RGB++ assets, Bitcoin transactions, and CKB.
  - Explored potential use cases for stablecoins, tokenized assets, and cross-chain applications.
  - Started evaluating how RGB++ could fit into Kaze's existing wallet and payment infrastructure.

### Key Learnings

- **Fiber Network**
  - How Fiber uses payment channels to enable fast and scalable transactions.
  - CCH swaps wrapped BTC (cWBTC) 1:1 with Lightning sats — not native on-chain CKB.
  - Fiber invoices for Lightning must use `hash_algorithm: sha256` and the wrapped BTC UDT type script.
  - Paying a CCH invoice from the same Fiber node requires `allow_self_payment`.

- **RGB++**
  - The core concepts behind RGB++ and its relationship with CKB.
  - How RGB++ extends Bitcoin's functionality while leveraging CKB's programmability.
  - Potential applications for issuing and transferring digital assets across Bitcoin and CKB.

### Practical Progress

- Built Fiber ↔ Lightning swap APIs on top of Fiber CCH:
  - `POST /fiber/swap/fiber-to-lightning` — create a CCH order from a BOLT11 (alias: `/fiber/swap/ckb-to-lightning`).
  - `POST /fiber/swap/pay-lightning` — one-shot Fiber → Lightning (`send_btc` + Fiber `send_payment` + settlement poll).
  - `POST /fiber/swap/receive-lightning` — one-shot Lightning → Fiber (sha256 cWBTC invoice + `receive_btc`).
  - `POST /fiber/swap/quote` — preview amounts without creating an order.
  - `GET /fiber/swap/ready` — Fiber RPC + CCH + channel readiness check.
  - `GET /fiber/swap/order/{payment_hash}` and `POST /fiber/swap/wait` — order lookup and settlement polling.
- Strengthened CCH order responses (`direction`, `settled`, decimal amounts, `expires_at`, `next_action`).
- Rejected amount-less BOLT11 invoices and Fiber invoices that are not CCH-compatible.
- Started technical research and experimentation with RGB++.
- Began evaluating RGB++ use cases for Kaze, particularly around stablecoins and digital assets.

### Environment Setup

- Same DigitalOcean Droplet + Fiber RPC setup from previous weeks.
- Continued working with the deployed CKB infrastructure and Fiber CCH (cWBTC + LND) for swap APIs.
- Local development environment prepared for RGB++ research and experimentation.
- Next step is to test Fiber → Lightning swaps end-to-end on testnet, then go deeper into RGB++ integration.
