# Client integration guide

This API is **non-custodial**: it never generates, stores, or receives your
users' mnemonics or private keys on the production endpoints. Key
generation and transaction signing happen entirely on the client (browser,
mobile app, desktop app, wherever your users' devices are); the server only
ever sees public addresses, public keys (as an "account xpub"), and
already-signed transactions.

This also means anything derived here is fully portable: the derivation
path (`m/44'/309'/0'/0/{index}`) and lock script (secp256k1-blake160
sighash-all) are the standard ones used by Nervos's own **Neuron** wallet
and the wider CKB ecosystem, not something custom to this API. A mnemonic
your app generates can be imported into Neuron (or any other
standards-compliant CKB wallet) and will show the exact same addresses and
balance, and vice versa.

## Architecture at a glance

```
Client (owns the mnemonic/private keys)          Server (this API)
──────────────────────────────────────           ─────────────────
generate mnemonic (BIP-39)
  │
derive account key m/44'/309'/0' (BIP-32)
  │
export account xpub (public only) ─────────────▶ POST /wallet/address
                                                  POST /wallet/balance
                                    ◀───────────  { index, address, balances... }

build a transfer ───────────────────────────────▶ POST /transaction/build
                                    ◀───────────  { transaction }  (unsigned)
  │
sign `transaction` locally with the
sender's private key (never leaves
the client)
  │
submit signed transaction ─────────────────────▶ POST /transaction/broadcast
                                    ◀───────────  { tx_hash }
```

The server never needs a signature-capable key at any point in this flow.

## Endpoint reference (production / non-custodial)

All requests/responses are JSON. Base path is whatever host you deploy to
(see `/docs` for interactive Swagger UI).

### `POST /wallet/address`

Derive a receive address from an account xpub. Omit `index` to
auto-detect the next unused index (scans on-chain history with a 20-address
gap limit).

Request:

```json
{ "account_xpub": "xpub6...", "index": 0 }
```

Response:

```json
{ "index": 0, "address": "ckt1qzda..." }
```

### `POST /wallet/balance`

Totals the balance across every address derived from an account xpub.

Request:

```json
{ "account_xpub": "xpub6..." }
```

Response:

```json
{
  "balances": [{ "index": 0, "address": "ckt1qzda...", "balance": 6000000000 }],
  "total_balance": 6000000000
}
```

### `POST /balance`

Balance of a single, already-known address (no xpub needed).

Request: `{ "address": "ckt1qzda..." }`
Response: `{ "address": "ckt1qzda...", "balance": 6000000000 }`

### `POST /transactions`

Recent transaction history for an address.

Request: `{ "address": "ckt1qzda...", "limit": 20 }`
Response:

```json
{
  "address": "ckt1qzda...",
  "transactions": [
    { "tx_hash": "0x...", "block_number": 123, "direction": "received", "amount": 6000000000 }
  ]
}
```

### `POST /transaction/build`

Builds a fee-balanced, **unsigned** transfer transaction. Only touches
public chain data (live cells, cell deps) -- no key material involved.

Request:

```json
{
  "sender_address": "ckt1qzda...",
  "receiver_address": "ckt1qother...",
  "amount": "100.0"
}
```

Response:

```json
{
  "transaction": { "version": "0x0", "cell_deps": [...], "inputs": [...], "outputs": [...], "outputs_data": [...], "witnesses": [...] },
  "sender_address": "ckt1qzda...",
  "receiver_address": "ckt1qother...",
  "amount": "100.0"
}
```

`transaction` is a standard CKB JSON transaction (the same shape the CKB
RPC/indexer use), so it can be handed straight to any official CKB SDK to
sign.

### `POST /transaction/broadcast`

Submits a client-signed transaction. Balances give-or-take, this just calls
`send_transaction` on the CKB node -- it doesn't need to (and doesn't) trust
the caller, since the node itself rejects a bad signature.

Request:

```json
{ "transaction": { "...": "the same shape, but with real signatures in `witnesses`" } }
```

Response: `{ "tx_hash": "0x..." }`

### Dev-only endpoints

`/dev/generate-mnemonic`, `/dev/generate-address`, `/dev/balance/wallet`,
`/dev/transaction/send` exist only for local iteration without a
client-side signer wired up yet. They return `403 Forbidden` unless the
server has `ALLOW_DEV_KEY_ENDPOINTS=true` set -- which should never be true
in production, since these endpoints touch a mnemonic/private key
server-side by design.

## Client-side implementation (JavaScript example)

Any BIP-39/BIP-32 library works for key generation; any CKB SDK works for
signing. This example uses `bip39` + `@ckb-lumos/hd` for key
derivation/export (lumos ships a CKB-specific `AccountExtendedPublicKey`
class that already implements the `m/44'/309'/0'` path) and
`@nervosnetwork/ckb-sdk-core` for signing, since its `signTransaction`
helper operates directly on the same raw JSON transaction shape
`/transaction/build` returns.

```bash
npm install bip39 @ckb-lumos/hd @nervosnetwork/ckb-sdk-core
```

### 1. Create a wallet (once per user, entirely on-device)

```javascript
import { generateMnemonic, mnemonicToSeedSync } from "bip39";
import { ExtendedPrivateKey } from "@ckb-lumos/hd";

// Generate and show this to the user to back up. Never send it anywhere.
const mnemonic = generateMnemonic();

const seed = mnemonicToSeedSync(mnemonic);
const masterKey = ExtendedPrivateKey.fromSeed(seed); // m
const accountKey = masterKey.privateKeyInfo(0);      // m/44'/309'/0'

// Only this ever leaves the device:
const accountXpub = accountKey.publicKey.serialize(); // "xpub..."

// Persist `mnemonic` (encrypted, on-device only) and `accountXpub`
// (fine to send to your backend / store server-side, it's public data).
```

### 2. Look up addresses and balances

```javascript
const { index, address } = await fetch("/wallet/address", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ account_xpub: accountXpub }),
}).then((r) => r.json());

const { balances, total_balance } = await fetch("/wallet/balance", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ account_xpub: accountXpub }),
}).then((r) => r.json());
```

### 3. Send funds (build → sign locally → broadcast)

```javascript
import { ExtendedPrivateKey, AddressType } from "@ckb-lumos/hd";
import CKB from "@nervosnetwork/ckb-sdk-core";

// Re-derive the seed and the specific child key that owns `sender_address`.
// The private key for index N never leaves this function/device.
const seed = mnemonicToSeedSync(mnemonic);
const masterKey = ExtendedPrivateKey.fromSeed(seed);
const childKey = masterKey.privateKeyInfo(0).privateKeyInfo(AddressType.Receiving, senderIndex);
const senderPrivateKey = childKey.privateKey; // hex string, "0x..."

// 1. Ask the server to build the unsigned transaction (public data only).
const { transaction } = await fetch("/transaction/build", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ sender_address, receiver_address, amount: "100.0" }),
}).then((r) => r.json());

// 2. Sign it locally. `signTransaction` fills in the witnesses with real
//    signatures given the raw transaction + private key.
const ckb = new CKB();
const signedTransaction = ckb.signTransaction(senderPrivateKey)(transaction);

// 3. Hand the signed transaction back -- the server only ever forwards it.
const { tx_hash } = await fetch("/transaction/broadcast", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ transaction: signedTransaction }),
}).then((r) => r.json());
```

Swap in whichever CKB SDK matches your platform (Python, Go, Swift/Kotlin
native, etc.) -- the contract is the same: `/transaction/build` returns a
standard CKB transaction, any CKB SDK can sign it, `/transaction/broadcast`
takes the signed result.

### Interoperability with other CKB wallets

Because the derivation path and lock script above are the CKB ecosystem
standard (not custom to this API):

- A mnemonic generated in your app can be imported into Neuron (or another
  standards-compliant wallet) and will show the same addresses/balance.
- A mnemonic a user already has in Neuron can be used with this API just by
  deriving its `m/44'/309'/0'` xpub and calling `/wallet/address` /
  `/wallet/balance` with it.
