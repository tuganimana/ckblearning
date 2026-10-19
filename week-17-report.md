## Builder Track Weekly Final Report — Week 17

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 19-10-2026

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

### Key Learnings

- **USDC and USDT are two pairs on one swap path**
  - CKB–USDC and CKB–USDT share quote, fund, sign, and confirm. The asset changes; the lock and the signing step do not.
  - A stablecoin swap is only finished when the CKB side and the USDC or USDT side both show in the refreshed balance.

- **Optimization is freshness after the swap**
  - Create, unlock, receive, send, and swap still use the same lock.
  - After a CKB–USDC or CKB–USDT confirm, capacity and history must come from the same live-cell query, or the app shows a stale balance.

### Practical Progress

- Kaze swaps:
  - Shipped CKB–USDC and CKB–USDT in the Kaze app: quote, fund, sign on device, and confirm.
  - Both pairs stay on screen until settlement, then balance and history update from live cells.
- Kaze app optimization:
  - Receive, send, and the two stablecoin swaps now refresh from the same cells.
  - Kept onboarding and local signing (mnemonic and passkey, same fundable lock).

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Kaze on-chain CKB↔BTC → Kaze Chrome extension → CKB–USDC and CKB–USDT swaps and Kaze app optimization.
- **Shipped:** address, mnemonic, transfer, balance/history; Fiber CCH swaps; Kaze self-custodial wallet with on-chain CKB → Bitcoin; browser wallet; CKB–USDC and CKB–USDT swaps.
- **This week:** added CKB–USDC and CKB–USDT swaps in Kaze, and optimized the app so balances stay fresh after send and after a swap.

### Environment Setup

- Kaze app on CKB mainnet for CKB–USDC and CKB–USDT quote, sign, and confirm.
- Same self-custodial lock for receive, send, and both stablecoin swaps, with live-cell refresh after each confirm.

### Conclusion

This week I added CKB–USDC and CKB–USDT swaps to Kaze and optimized the app around them. A user can quote either pair, sign on device, confirm, and see the updated balance from live cells. Thank you.
