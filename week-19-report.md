## Builder Track Weekly Final Report — Week 19

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 02-11-2026

### Courses Completed

- **Integrating Fiber in the Kaze app**
  - Wired Fiber into Kaze as the off-chain payment rail next to the self-custodial CKB lock.
  - The lock still holds on-chain CKB. A Lightning payment spends Fiber channel balance (wrapped BTC), not live-cell capacity.
  - Kaze talks to the Fiber node for invoices, payments, and CCH orders, and keeps signing on device for the on-chain lock.
  - One app, two balances: cells for receive, send, and on-chain swaps; Fiber channels for Lightning.

- **Fiber to Bitcoin Lightning invoices**
  - Implemented both directions through Fiber CCH and LND.
  - Fiber → Lightning: paste a BOLT11, CCH `send_btc` returns a Fiber invoice (`fibt…`), Kaze pays that invoice from the Fiber channel, and the hub pays the Lightning invoice.
  - Lightning → Fiber: mint a Fiber invoice with `hash_algorithm: sha256` and the wrapped BTC type script, CCH `receive_btc` returns a BOLT11, and the Fiber invoice settles when that BOLT11 is paid.
  - Wrapped BTC stays 1:1 with sats (1 sat = 1 cWBTC unit). The Fiber invoice for a Lightning swap must be sha256. A `ckb_hash` invoice will not match the hub.
  - The quote stays on screen with the Lightning amount and the Fiber amount until the CCH order reaches Success.

- **RGB++ stays the on-chain Bitcoin rail**
  - RGB++ is the leap already in Kaze: a CKB cell bound to a Bitcoin UTXO. Both transactions confirm on chain.
  - That path does not create or pay a Lightning invoice. Lightning is off-chain, and the hub that joins it to Fiber is CCH, not an RGB++ leap.
  - Kaze keeps both: RGB++ when the user wants native CKB and a Bitcoin UTXO to settle on chain; Fiber CCH when the user wants to pay or receive a BOLT11.

### Key Learnings

- **A Lightning invoice is a Fiber CCH order**
  - Paying or receiving Bitcoin Lightning from Kaze is `send_btc` or `receive_btc` on Fiber CCH, then a Fiber payment. It is not an RGB++ leap and it is not a CKB cell transfer.
  - The order is finished only when the Fiber side and the Lightning side both show settled on the same payment hash.

- **Channel balance is not the CKB lock**
  - `/wallet/balance` is live cells. Fiber → Lightning spends wrapped BTC already in a Fiber channel.
  - After a Lightning confirm, refresh the Fiber payment and the CCH order. Refreshing cells alone will not show the Lightning spend.

- **The Fiber invoice has to be Lightning-shaped**
  - Lightning → Fiber needs `hash_algorithm: sha256` and the wrapped BTC UDT type script.
  - CCH rejects a BOLT11 with no amount. The amount on the invoice is the amount that settles.

- **RGB++ and Fiber answer different questions**
  - RGB++ leap: on-chain CKB cell ↔ Bitcoin UTXO. The CKB address does not change, and both chains must confirm.
  - Fiber CCH: off-chain wrapped BTC ↔ Lightning sats, through a hub with LND. Faster, and it depends on channel liquidity.
  - Putting RGB++ in front of a Lightning invoice adds a leap the invoice does not need.

### Practical Progress

- Fiber in Kaze:
  - Integrated Fiber invoices, Fiber payments, and CCH orders into the Kaze app beside the existing lock.
  - Kept onboarding and local signing for the on-chain lock. Lightning uses the Fiber node and CCH.
- Fiber ↔ Lightning:
  - Shipped Fiber → Lightning (`send_btc`, pay the Fiber invoice, poll until the BOLT11 settles).
  - Shipped Lightning → Fiber (sha256 wrapped-BTC invoice, `receive_btc`, share the BOLT11).
  - Amount, fee, and status stay on screen until the CCH order is Success or Failed.
- RGB++:
  - Left the on-chain CKB ↔ Bitcoin leap in place. Did not route Lightning invoices through it.

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Kaze on-chain CKB↔BTC → Kaze Chrome extension → CKB–USDC and CKB–USDT swaps and Kaze app optimization → exchange optimization and CKB → KES rails → Fiber in Kaze and Fiber CCH to Bitcoin Lightning invoices.
- **Shipped:** address, mnemonic, transfer, balance/history; Fiber CCH swaps; Kaze self-custodial wallet with on-chain CKB → Bitcoin; browser wallet; CKB–USDC and CKB–USDT swaps; CKB → KES rail; Fiber ↔ Lightning invoices in Kaze.
- **This week:** integrated Fiber into the Kaze app and implemented Fiber CCH to Bitcoin Lightning invoices, while keeping RGB++ as the on-chain CKB ↔ Bitcoin path.

### Environment Setup

- Kaze app with Fiber RPC (`FIBER_RPC_URL`) for invoices, payments, and CCH orders.
- Fiber CCH with LND for `send_btc` and `receive_btc`. Wrapped BTC type script set for Lightning → Fiber.
- Same self-custodial lock for on-chain receive, send, and RGB++ CKB ↔ Bitcoin. Fiber channel balance is separate.

### Conclusion

This week I integrated Fiber into the Kaze app and implemented Fiber to Bitcoin Lightning invoices. A user can pay a BOLT11 from Fiber channel balance, or receive Lightning into a sha256 wrapped-BTC Fiber invoice, and see the CCH order settle. RGB++ stays the on-chain leap to a Bitcoin UTXO. It is not the path that pays a Lightning invoice. Thank you.
