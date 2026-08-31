## Builder Track Weekly Report — Week 10

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 31-08-2026

### Courses Completed

- **CKB Flutter Client (example app)**
  - Cleaned up the package example app so it is a usable wallet demo instead of a generic Flutter starter.
  - Tightened the example around the `ckb_flutter_client` APIs: light-client RPC URL, header sync, and lock-based cell queries.
  - Separated demo UI from package code so the example only consumes the public Dart API.
  - Follow the development of the package on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

- **Mnemonic wallet setup**
  - Set up BIP-39 mnemonic generation and restore in the Flutter CKB package / example flow.
  - Derived a secp256k1-blake160 lock from the mnemonic and encoded a fundable CKB address (`ckt1…` / `ckb1…`).
  - Used that address as the receive target so the wallet can be funded on testnet.

- **CKB wallet with passkey**
  - Started passkey (WebAuthn) wallet setup as an alternative to a seed phrase.
  - Mapped passkey registration to a CKB lock / address so the same example app can create a wallet without storing a mnemonic.
  - Compared mnemonic (secp256k1) vs passkey (device-backed credential) onboarding for mobile UX.

### Key Learnings

- **Example app as package surface**
  - A clean example app is how the Flutter package is actually tested: connect to a light-client RPC, sync headers, then query cells for a lock.
  - Keeping protocol/client code in `lib/` and UI in `example/` keeps the package reusable for Kaze and other Flutter apps.

- **Mnemonic → CKB address**
  - A 12/24-word mnemonic derives a private key, then a blake160 lock args, then a CKB address that can be funded.
  - The light client indexes by lock script, so the generated address/lock is what you register with `set_scripts` and query with `get_cells`.

- **Passkey wallet**
  - Passkeys let the device hold the credential (Face ID / fingerprint / platform authenticator) instead of showing a seed phrase.
  - A passkey wallet still needs a CKB address derived from the credential’s public key so it can be funded and later unlocked on-chain.

### Practical Progress

- Cleaned the `ckb_flutter_client` example app:
  - Replaced starter boilerplate with a focused demo (RPC URL, header sync, lock query, capacity / live cells).
  - Kept UI out of the package so `example/` is the only place that renders wallet screens.
- Set up mnemonic onboarding in the Flutter CKB package flow:
  - Generate or restore a mnemonic.
  - Derive lock args and a CKB address.
  - Show the address so the wallet can be funded (faucet / transfer).
- Set up passkey wallet creation in the same example:
  - Register a passkey and derive a CKB address from the credential.
  - Same fund-the-wallet path as mnemonic: display address, then query cells via the light client.
- Package work continues on [Ckb Flutter client](https://github.com/tuganimana/ckb-flutter-client)

### Environment Setup

- Same DigitalOcean Droplet + CKB infrastructure from previous weeks for chain/RPC reference.
- Local Flutter/Dart environment for `ckb_flutter_client` (package + cleaned example app).
- Local `ckb-light-client` RPC (`http://127.0.0.1:9000`) for header sync and cell queries against the generated address.
- Next step: fund the generated address, confirm cells via the light client, then sign/send from both mnemonic and passkey wallets.
