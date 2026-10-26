## Builder Track Weekly Final Report — Week 18

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 26-10-2026

### Courses Completed

- **CKB–USDC and CKB–USDT swaps in Kaze**
  - Added two stablecoin pairs on the same self-custodial lock: CKB ↔ USDC and CKB ↔ USDT.
  - Each swap follows the same path: quote the pair, fund from the CKB lock, sign on device, then confirm once both sides settle.
  - The quote stays on screen with the CKB amount and the USDC or USDT amount until the swap confirms.
  - After confirm, capacity and history refresh from live cells, so the new stablecoin balance and the spent CKB agree.

- **Optimizing the Kaze app**
  - Tightened the wallet so receive, send, and swap all read the same live cells after a transfer or a swap.
  - Kept unlock local: mnemonic or passkey still opens the same secp256k1-blake160 lock, and Kaze signs on device.
  - Shortened the swap screen so the pair, the amounts, and the confirm step stay visible together.
  - Left one path through the app: one lock, one sync, then sign and submit for send and for both stablecoin swaps.

- **Optimizing the exchange**
  - Tightened the exchange so CKB–USDC, CKB–USDT, and CKB → KES share one path: quote, fund from the CKB lock, sign on device, then confirm.
  - Kept the rate on screen with the CKB amount and the destination amount until the exchange confirms.
  - Refreshed the quote from the live rate before sign, so the amount the user confirms is the amount that settles.
  - After confirm, capacity and history still come from the same live-cell query, for a stablecoin pair and for the KES rail.

- **Kaze CKB → KES rails**
  - Integrated a CKB → KES rail on the same self-custodial lock: quote CKB in Kenyan shillings, fund from the lock, sign on device, then confirm once KES settles.
  - The quote stays on screen with the CKB amount and the KES amount until the rail confirms.
  - KES is a fiat rail next to the stablecoin pairs. The asset and the payout change; the lock and the signing step do not.
  - After confirm, the spent CKB and the history refresh from live cells, and the KES side shows as settled.

### Key Learnings

- **USDC and USDT are two pairs on one swap path**
  - CKB–USDC and CKB–USDT share quote, fund, sign, and confirm. The asset changes; the lock and the signing step do not.
  - A stablecoin swap is only finished when the CKB side and the USDC or USDT side both show in the refreshed balance.

- **Optimization is freshness after the swap**
  - Create, unlock, receive, send, and swap still use the same lock.
  - After a CKB–USDC or CKB–USDT confirm, capacity and history must come from the same live-cell query, or the app shows a stale balance.

- **An optimized exchange is one quote the user can still see**
  - Stablecoin pairs and the KES rail should not grow a second sign or a second sync.
  - The rate has to be fresh at confirm. A stale quote settles a different amount than the one on screen.

- **KES is another rail on the same lock**
  - CKB → KES pays out Kenyan shillings. It does not replace USDC, USDT, or the Bitcoin path.
  - The rail is finished only when the spent CKB is in the refreshed cells and the KES side has settled.

### Practical Progress

- Kaze swaps:
  - Shipped CKB–USDC and CKB–USDT in the Kaze app: quote, fund, sign on device, and confirm.
  - Both pairs stay on screen until settlement, then balance and history update from live cells.
- Kaze app optimization:
  - Receive, send, and the two stablecoin swaps now refresh from the same cells.
  - Kept onboarding and local signing (mnemonic and passkey, same fundable lock).
- Exchange optimization:
  - One exchange path for CKB–USDC, CKB–USDT, and CKB → KES, with the live rate held on screen through confirm.
- Kaze CKB → KES rails:
  - Integrated CKB → KES: quote in shillings, fund from the same lock, sign on device, and confirm.
  - After confirm, CKB capacity and history refresh from live cells and the KES payout shows as settled.

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Kaze on-chain CKB↔BTC → Kaze Chrome extension → CKB–USDC and CKB–USDT swaps and Kaze app optimization → exchange optimization and CKB → KES rails.
- **Shipped:** address, mnemonic, transfer, balance/history; Fiber CCH swaps; Kaze self-custodial wallet with on-chain CKB → Bitcoin; browser wallet; CKB–USDC and CKB–USDT swaps; CKB → KES rail.
- **This week:** added CKB–USDC and CKB–USDT swaps, optimized the app and the exchange, and integrated a CKB → KES rail on the same lock.

### Environment Setup

- Kaze app on CKB mainnet for CKB–USDC, CKB–USDT, and CKB → KES quote, sign, and confirm.
- Same self-custodial lock for receive, send, both stablecoin swaps, and the KES rail, with live-cell refresh after each confirm.

### Conclusion

This week I added CKB–USDC and CKB–USDT swaps to Kaze, optimized the exchange, and integrated a CKB → KES rail. A user can quote a stablecoin pair or shillings, sign on device, confirm, and see the updated CKB balance from live cells. Thank you.
