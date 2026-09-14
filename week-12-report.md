## Builder Track Weekly Final Report — Week 12

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 14-09-2026

### Courses Completed

- **Kaze CKB → Bitcoin on-chain swap**
  - Contributed to completing the final Kaze CKB to Bitcoin on-chain swap (native cells + Bitcoin UTXOs, not Fiber/Lightning CCH).
  - Wired the swap path so CKB lock/capacity and the Bitcoin commitment settle together (RGB++-style leap / isomorphic bind).
  - Helped close remaining quote, fund, sign, and confirm steps so a swap can finish end-to-end in Kaze.

- **Open Flutter CKB wallet client**
  - Continued contributing to the open `ckb_flutter_client` wallet: light-client RPC, header sync, lock queries, receive, and send.
  - Kept package APIs in `lib/` so Kaze and other Flutter apps can reuse the same client.
  - Follow the development of the package on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

- **Example app design**
  - Continued designing the package example as a real wallet demo, not a starter template.
  - Refined onboarding (generate / import mnemonic, passkey) and Receive / Send / Cells screens.
  - ![CKB wallet setup](screenshots/app1.png)
  - ![CKB wallet receive](screenshots/app.png)

### Key Learnings

- **On-chain CKB ↔ Bitcoin**
  - On-chain swap is cell + UTXO: CKB spend and Bitcoin commitment must match, unlike Fiber CCH (cWBTC ↔ Lightning).
  - The wallet must show both sides (CKB address/lock and Bitcoin destination) until both txs confirm.

- **Open Flutter client**
  - A reusable Dart package is the right surface for Kaze: connect, sync, query cells, then sign and send.
  - Mnemonic and passkey both end at a fundable CKB lock; the light client indexes that lock.

- **Example as design lab**
  - The example is where wallet UX is tested: network switch, onboarding, receive address, send, live cells.
  - Clear screens in `example/` keep protocol code out of the UI and make the package easier to adopt.

### Practical Progress

- Kaze CKB → Bitcoin on-chain swap:
  - Contributed to the final swap flow (quote, lock/fund, sign, submit, confirm).
  - Aligned Kaze wallet UX with on-chain settlement, not only off-chain Fiber swaps.
- Open Flutter CKB client:
  - Continued package work (RPC, sync, cells, capacity, receive/send).
  - Same client remains the building block for Kaze and the public example.
- Example design:
  - Setup: generate mnemonic, import mnemonic, create with passkey; Testnet / Mainnet.
  - Wallet: Receive address, Send, Cells, capacity refresh after funding.
- Package work continues on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

### Review of Completed

- **Track:** CKB cells and txs → Rust wallet API → Fiber + RGB++ → Flutter light client → Kaze on-chain CKB↔BTC swap.
- **Shipped:** address, mnemonic, transfer, balance/history API; Fiber CCH swaps; open Flutter CKB client + example wallet.
- **This week:** final Kaze on-chain swap contribution; more Flutter client work; example design (onboarding + receive).

### Environment Setup

- Same DigitalOcean Droplet + CKB / Fiber infrastructure from previous weeks for chain and swap reference.
- Local Flutter/Dart environment for `ckb_flutter_client` (package + example wallet design).
- Local `ckb-light-client` RPC (`http://127.0.0.1:9000`) for header sync, cell queries, and send.

### Conclusion

I am happy to have been part of this program and to complete it with real contributions to two products: the Kaze CKB slef custodial wallet , and the open Flutter CKB wallet client (plus its example design). This track took me from CKB fundamentals to shipping wallet and swap work . Thank you.
