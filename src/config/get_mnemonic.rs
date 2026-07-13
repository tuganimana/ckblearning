use bip39::{Language, Mnemonic};

pub fn generate_mnemonic() -> Mnemonic {
    Mnemonic::generate_in(Language::English, 12).unwrap()
}