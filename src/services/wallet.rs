use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use ckb_sdk::types::{Address, AddressPayload};
use secp256k1::PublicKey;

use crate::config::client::network_type;

pub struct DerivedWallet {
    pub address: Address,
    pub public_key: String,
    pub private_key: String,
}

/// Derives a CKB wallet (address + keypair) from a BIP-39 mnemonic phrase at
/// the given account `index`, so the same mnemonic can deterministically
pub fn derive_wallet(mnemonic_phrase: &str, index: u32) -> Result<DerivedWallet, String> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic_phrase)
        .map_err(|e| format!("Invalid mnemonic: {e}"))?;
    let seed = mnemonic.to_seed("");

    let path: DerivationPath = format!("m/44'/309'/0'/0/{index}")
        .parse()
        .map_err(|_| "Invalid derivation path".to_string())?;
    let child_key =
        XPrv::derive_from_path(&seed, &path).map_err(|e| format!("Failed to derive child key: {e}"))?;

    let private_key_bytes = child_key.private_key().to_bytes();
    let secp_pubkey = child_key.public_key().to_bytes();
    let pubkey =
        PublicKey::from_slice(&secp_pubkey).map_err(|e| format!("Invalid public key format: {e}"))?;
    let payload = AddressPayload::from_pubkey(&pubkey);
    let address = Address::new(network_type(), payload, true);

    Ok(DerivedWallet {
        address,
        public_key: hex::encode(pubkey.serialize()),
        private_key: hex::encode(private_key_bytes),
    })
}
