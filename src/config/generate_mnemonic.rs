use bip39::{Language, Mnemonic};
use bip32::{DerivationPath, XPrv};
use ckb_sdk::types::{Address, AddressPayload, NetworkType};
use secp256k1::PublicKey;

pub struct GeneratedAddress {
    pub address: String,
    pub public_key: String,
    pub private_key: String,
    pub mnemonic: String, // i will remove it on prod
}
pub fn generate_mnemonic() -> GeneratedAddress {
    // 1. Generate a fresh, secure 12-word recovery phrase
    let mnemonic = Mnemonic::generate_in(Language::English, 12).unwrap();
    let phrase = mnemonic.to_string();


    let seed = mnemonic.to_seed("");

    let path: DerivationPath = "m/44'/309'/0'/0/0"
        .parse()
        .expect("Invalid derivation path");
    let child_key = XPrv::derive_from_path(&seed, &path).expect("Failed to derive child key");

    // 4. Extract the raw private key and build the public key
    let private_key_bytes = child_key.private_key().to_bytes();

    // We can convert the bip32 public key to bytes and pass it to ckb_sdk
    let secp_pubkey = child_key.public_key().to_bytes();
    let pubkey = PublicKey::from_slice(&secp_pubkey).expect("Invalid public key format");
    let payload = AddressPayload::from_pubkey(&pubkey);
    let address = Address::new(
        NetworkType::Testnet, // Change to NetworkType::Mainnet for production!
        payload,
        true,
    );
    GeneratedAddress {
        mnemonic: phrase,
        address: address.to_string(),
        public_key: hex::encode(pubkey.serialize()),
        private_key: hex::encode(private_key_bytes),}
}