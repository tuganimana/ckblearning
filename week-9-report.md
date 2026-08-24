## Builder Track Weekly Report — Week 9

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 24-08-2026

### Courses Completed

- **RGB++ Exploration (continued)**
  - Continued exploring RGB++ beyond the initial overview from previous weeks.
  - Reviewed how RGB++ assets are bound to Bitcoin UTXOs and verified on CKB (isomorphic binding).
  - Studied issuance, transfer, and leap (Bitcoin ↔ CKB) flows, and how they relate to wallet UX.
  - Continued evaluating RGB++ use cases for Kaze, particularly stablecoins, tokenized assets, and cross-chain payments.

- **CKB Light Client (Flutter package)**
  - Started work on a Flutter CKB light-client package as the first building block for wallet infrastructure.
  - Reviewed CKB light-client architecture (SPV-style verification, header sync, and filtered cell queries).
  - Scoped the package as a reusable Dart/Flutter library rather than app-specific wallet code.
  - Mapped how a light client can later support RGB++ asset views without requiring a full node.
  - Follow the development of the package on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

### Key Learnings

- **RGB++**
  - RGB++ uses isomorphic binding: Bitcoin UTXOs as the commitment layer, CKB cells as the programmable state layer.
  - Asset transfers need coordinated Bitcoin + CKB transactions; wallets must surface both sides of a leap.
  - Stablecoins and other RGB++ assets can sit on top of existing CKB lock/type scripts once the light client can prove cell state.

- **CKB Light Client**
  - A light client verifies chain headers and only fetches the cells it needs, which is a better fit for mobile than a full node.
  - Packaging this as a Flutter/Dart package keeps CKB networking and verification reusable across apps (Kaze and others).
  - Light-client sync and proof verification are the foundation for later RGB++ and payment features.

### Practical Progress

- Continued RGB++ research: isomorphic binding, issuance/transfer/leap, and how those flows would appear in a mobile wallet.
- Started scaffolding a Flutter CKB light-client package (intended as a pub-style Dart package):
  - Package layout (`lib/`, public API, example app) for header sync and light-client RPC.
  - Initial API surface for connecting to a CKB light-client endpoint, syncing headers, and querying cells by lock.
  - Separated protocol/client code from UI so the package can be consumed by Kaze or any Flutter app.
- Defined next package milestones: header sync, cell query, then transaction proof verification.

### Environment Setup

- Same DigitalOcean Droplet + CKB infrastructure from previous weeks for chain/RPC reference.
- Local Flutter/Dart environment set up for the light-client package (package scaffolding + example app).
- Local notes and references for RGB++ binding, leap, and light-client verification.
- Next step: implement header sync and cell queries in the Flutter CKB light-client package, then continue RGB++ integration on top of it.
