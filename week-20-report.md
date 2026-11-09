## Builder Track Weekly Final Report — Week 20

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 09-11-2026

### Courses Completed

- **Optimizing the Kaze app**
  - Cut repeated work on the hot paths: balance, history, and Fiber status are fetched once per screen and refreshed on demand, not on every render.
  - Split the on-chain view (live cells) from the Fiber view (channel balance), so a cell refresh no longer waits on the Fiber node and the other way round.
  - Kept onboarding and local signing for the self-custodial lock unchanged.

- **Fixing Fiber payout in Kaze**
  - Payout (Kaze pays out over Fiber) was mixing networks: one node, one data dir, and one config served both testnet cWBTC and mainnet CKB.
  - Split the Fiber deploy by network: `config.mainnet.yml` and `config.testnet.yml`, each with its own store and key dir (`~/fiber-node-mainnet` and `~/fiber-node`), so switching never mixes channels or keys.
  - Mainnet pays native CKB over Fiber. Lock scripts, bootnodes, and the UDT whitelist come from the official Fiber v0.9.1 mainnet config, and the image tag moved from `0.9.0-rc7` to `0.9.1` to match.
  - A payout is done only when the Fiber payment shows Success. Kaze now polls the payment by hash and shows Failed with the reason instead of leaving it pending.

- **Fixing Fiber collection in Kaze**
  - Collection (Kaze receives over Fiber) mints an invoice on the node for the network the user is on: native CKB on mainnet, cWBTC on testnet.
  - The invoice amount and currency stay on screen until the invoice is Paid or Expired.
  - Testnet keeps LND and Fiber CCH for Lightning ↔ Fiber swaps. Mainnet runs Fiber only, with no LND or CCH.

### Key Learnings

- **One network per Fiber node**
  - A Fiber node's store, key, and channels belong to one chain. Reusing a testnet data dir for mainnet (or the reverse) breaks payouts in ways that look like liquidity problems.
  - Separate configs, data dirs, and an optional separate key (`FIBER_MAINNET_CKB_KEY`, falling back to `FIBER_CKB_KEY`) keep each network clean.

- **Config must match the image**
  - Lock script hashes and cell deps in `config.<network>.yml` must match the deployed `nervos/fiber` tag. Bumping the image without updating the config is a silent failure.

- **Payout and collection need their own status**
  - Payout is finished when the payment reaches Success. Collection is finished when the invoice is Paid.
  - Refreshing live cells will not show either one. Kaze refreshes the Fiber payment or invoice by hash.

- **Mainnet is native CKB; Lightning stays on testnet for now**
  - Mainnet Fiber moves CKB off-chain. Fiber CCH with LND for BOLT11 stays on testnet with cWBTC.
  - RGB++ is still the on-chain CKB ↔ Bitcoin leap and is not part of payout or collection.

### Practical Progress

- App optimization:
  - Reduced duplicate RPC calls and separated the on-chain and Fiber refreshes.
- Fiber deploy:
  - `deploy-fiber.yml` now takes a `network` input (default `mainnet`), copies both configs to `~/fiber-deploy/`, and starts the node with the matching config and data dir.
  - Testnet deploys add LND (CCH payer). Mainnet deploys run Fiber only.
- Fiber payout:
  - Fixed payouts sent through the wrong network config. Payment status is polled to Success or Failed.
- Fiber collection:
  - Invoices are minted for the active network and tracked to Paid or Expired.

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Kaze on-chain CKB↔BTC → Kaze Chrome extension → CKB–USDC and CKB–USDT swaps and Kaze app optimization → exchange optimization and CKB → KES rails → Fiber in Kaze and Fiber CCH to Bitcoin Lightning invoices → Kaze optimization and Fiber payout and collection fixes.
- **Shipped:** address, mnemonic, transfer, balance/history; Fiber CCH swaps; Kaze self-custodial wallet with on-chain CKB → Bitcoin; browser wallet; CKB–USDC and CKB–USDT swaps; CKB → KES rail; Fiber ↔ Lightning invoices in Kaze; mainnet Fiber payout and collection in Kaze.
- **This week:** optimized the Kaze app and fixed Fiber payout and collection by splitting the Fiber node into mainnet and testnet deployments.

### Environment Setup

- Kaze app with Fiber RPC (`FIBER_RPC_URL=http://172.17.0.1:8227`) for invoices and payments.
- Fiber `0.9.1` on the Droplet: `config.mainnet.yml` (native CKB, `~/fiber-node-mainnet`) and `config.testnet.yml` (cWBTC, LND + CCH, `~/fiber-node`).
- Secrets: `FIBER_SECRET_KEY_PASSWORD`, `FIBER_CKB_KEY`, optional `FIBER_MAINNET_CKB_KEY` and `FIBER_ANNOUNCED_ADDR`.
- Same self-custodial lock for on-chain receive, send, and RGB++ CKB ↔ Bitcoin. Fiber channel balance is separate.

### Conclusion

This week I optimized the Kaze app and fixed Fiber payout and collection. The fix was to run one Fiber node per network, with its own config, data dir, and key, and to track each payout and each invoice to a final status. Mainnet now pays and collects native CKB over Fiber, and testnet keeps Fiber CCH with LND for Lightning. Thank you.
