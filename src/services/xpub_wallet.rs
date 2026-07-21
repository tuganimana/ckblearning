use bip32::{ChildNumber, XPub};
use ckb_sdk::types::{Address, AddressPayload};
use secp256k1::PublicKey;
use std::str::FromStr;

use crate::config::client::network_type;

/// Derives the CKB address at `m/<account>/0/{index}` given only the
/// **account-level extended public key** (`account_xpub`, e.g.
/// `xpub6...`) for `m/44'/309'/0'`.
///
/// This never needs, sees, or could even reconstruct a private key: BIP-32
/// public-key derivation only works for non-hardened path segments (`0` and
/// `{index}` here), which is exactly why the wallet's hardened account key
/// (`m/44'/309'/0'`) has to be derived once on the client and its *public*
/// half (the xpub) handed to the server for every address/balance lookup
/// after that.
pub fn derive_address_from_xpub(account_xpub: &str, index: u32) -> Result<Address, String> {
    let xpub = XPub::from_str(account_xpub).map_err(|e| format!("Invalid extended public key: {e}"))?;

    let external_chain = ChildNumber::new(0, false).map_err(|e| format!("Invalid derivation path: {e}"))?;
    let address_index = ChildNumber::new(index, false).map_err(|e| format!("Invalid derivation path: {e}"))?;

    let child_xpub = xpub
        .derive_child(external_chain)
        .and_then(|chain_xpub| chain_xpub.derive_child(address_index))
        .map_err(|e| format!("Failed to derive child public key: {e}"))?;

    let pubkey = PublicKey::from_slice(&child_xpub.to_bytes())
        .map_err(|e| format!("Invalid public key format: {e}"))?;
    let payload = AddressPayload::from_pubkey(&pubkey);
    let address = Address::new(network_type(), payload, true);

    Ok(address)
}
