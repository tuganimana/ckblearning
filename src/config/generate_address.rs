use ckb_sdk::types::{Address, AddressPayload, NetworkType};
use rand::Rng;
use secp256k1::{PublicKey, Secp256k1, SecretKey};


/// Full wallet result (clean structure)
pub struct GeneratedAddress {
    pub address: String,
    pub public_key: String,
    pub private_key: String,// i will remove it on prod
}

pub fn generate_address() -> GeneratedAddress {
    let mut privkey_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut privkey_bytes);

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&privkey_bytes).expect("Invalid private key");
    let pubkey = PublicKey::from_secret_key(&secp, &secret_key);

    // 4. Build CKB address payload
    let payload = AddressPayload::from_pubkey(&pubkey);

    // 5. Create address
    let address = Address::new(
        NetworkType::Testnet, 
        payload,
        true,
    );

    // 6. Return structured data
    GeneratedAddress {
        address: address.to_string(),
        public_key: hex::encode(pubkey.serialize()),
        private_key: hex::encode(privkey_bytes),
    }
}