## Builder Track Weekly Final Report — Week 14

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 28-09-2026

### Courses Completed

- **Finalizing and optimizing the Kaze CKB self-custodial wallet**
  - Closed the wallet so keys stay on device: mnemonic or passkey unlocks the same secp256k1-blake160 lock, and Kaze signs locally.
  - Tightened receive, send, capacity, and history so the UI refreshes from live cells after funding and after a transfer.
  - Optimized the on-chain CKB → Bitcoin flow (quote, fund, sign, confirm) so the CKB lock and the Bitcoin destination stay visible until both sides settle.
  - Kept the self-custodial path small: one lock, one light-client sync, then sign and submit.

- **Research: an alternative for CKB–BTC**
  - Compared the shipped RGB++ path (CKB cell + Bitcoin UTXO, isomorphic bind / leap) with Fiber CCH multi-asset as an off-chain alternative.
  - Older CCH demos swapped wrapped BTC (`cWBTC`) 1:1 with Lightning sats. Generalized CCH drops that mapping: the Fiber leg can be native CKB (shannons) or an allowlisted UDT, Lightning stays BTC-denominated, and the rate comes from the invoice pair plus hub fees. The HTLC flow itself does not change.
  - Also reviewed classic HTLC atomic swaps (a CKB HTLC cell against a Bitcoin HTLC) as a peer-to-peer on-chain option that needs neither an RGB++ leap nor a hub.
  - For Kaze: keep RGB++ when the user wants on-chain settlement of native cells and Bitcoin UTXOs; use Fiber CCH multi-asset when they want CKB capacity to pay a Lightning invoice without a leap.

- **Flutter CKB client package**
  - Continued `ckb_flutter_client` so other Flutter apps can integrate CKB without reading Kaze or the package source.
  - Public API stays short: connect to a light-client RPC, sync headers, query cells by lock, then receive and send.
  - Example app remains the reference: generate / import mnemonic, passkey on create, Testnet / Mainnet, receive and send on the same lock.
  - ![CKB wallet setup](screenshots/app1.png)
  - ![CKB wallet receive](screenshots/app.png)
  - Follow the development of the package on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

### Key Learnings

- **A finished self-custodial wallet is one lock**
  - Create, unlock, receive, send, and swap all use the same lock. Passkey and mnemonic only change how the seed is unlocked on device.
  - Optimization is mostly freshness: after send or after a swap confirm, capacity and history must come from the same live-cell query.

- **Two rails from CKB toward Bitcoin**
  - RGB++ leap binds a CKB cell to a Bitcoin UTXO. Both transactions must confirm, and the CKB address does not change.
  - Fiber CCH multi-asset is the alternative: native CKB (or a UDT) moves on a Fiber channel, and a hub settles sats on Lightning. It is faster and off-chain, and it depends on channel liquidity and a hub quote.
  - An HTLC atomic swap is the trust-minimized on-chain fallback when neither a leap nor a hub fits.

- **The package is how other apps get CKB**
  - Integrators should not fork Kaze. They add `ckb_flutter_client`, point it at a light client, and call connect, sync, query, sign, and send.
  - Protocol code stays in `lib/`; the example in `example/` is the adoption guide.

### Practical Progress

- Kaze CKB self-custodial wallet:
  - Finalized onboarding and local signing (mnemonic and passkey, same fundable lock).
  - Optimized capacity / history refresh and the CKB → Bitcoin confirm step so both sides of a swap stay on screen.
- CKB–BTC alternative:
  - Researched Fiber CCH multi-asset (native CKB or UDT ↔ Lightning BTC, quoted rate and hub fee, no hard 1:1 `cWBTC` mapping) next to the RGB++ path already in Kaze.
  - Noted HTLC atomic swap as a later peer-to-peer on-chain option.
- Flutter package:
  - Continued `ckb_flutter_client`: install, connect to `ckb-light-client`, sync, cells, capacity, receive, and send.
  - Example wallet remains the path for new integrators.
  - Package work continues on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Flutter light client → Kaze on-chain CKB↔BTC → package docs and passkey → finalize Kaze, CKB–BTC alternative, Flutter client.
- **Shipped:** address, mnemonic, transfer, balance/history; Fiber CCH swaps; open Flutter CKB client + example wallet; Kaze self-custodial wallet with on-chain CKB → Bitcoin.
- **This week:** finalized and optimized the Kaze wallet; researched Fiber CCH multi-asset (and HTLC atomic swap) as alternatives to RGB++ for CKB–BTC; continued the Flutter client so other apps can integrate CKB.

### Environment Setup

- Local Flutter/Dart environment for the Kaze wallet and for `ckb_flutter_client` (package + example).
- Local `ckb-light-client` RPC (`http://127.0.0.1:9000`) for header sync, cell queries, and send.
- Research notes on Fiber CCH multi-asset (native CKB / UDT ↔ Lightning BTC) as the alternative to the RGB++ on-chain swap.

### Conclusion

This closing week finished the wallet and left a clear next path. I finalized and optimized the Kaze self-custodial CKB wallet, researched Fiber CCH multi-asset as an alternative to the on-chain RGB++ CKB–BTC swap, and kept building `ckb_flutter_client` so other Flutter apps can integrate CKB through a small public API. Thank you.
