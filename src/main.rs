use ckb_jsonrpc_types as json_types;
use ckb_sdk::Address;
use std::str::FromStr;

mod config;
fn main() {
    println!("Learn Rust!");

    let config = config::client::connect_client();
    // get block data
    let block = config.get_block_by_number(0.into()).unwrap();
    println!("block: {}", serde_json::to_string_pretty(&block).unwrap());

    // generate an address
    // let generated = config::generate_address::generate_address();
    // println!("address: {}", generated.address);
    // println!("public_key: {}", generated.public_key);
    // println!("private_key: {}", generated.private_key);

    let tx = config::create_transaction::create_transaction(
        "ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwxy50ykktq65vrd7pteuffl8jlkwkth2gvr4x5p",
        "46c40a86a79ac34ae34ef4d002f76477d5ab2025d639e0951793309265ea0952",
        "ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqdnjn4m9al2zhsf6kk2xsv4qw25rvdv76s6y03z2",
        "1000.0",
    );
    
    let json_tx = json_types::TransactionView::from(tx);
    println!("============Transaction============");
    println!("tx: {}", serde_json::to_string_pretty(&json_tx).unwrap());
    println!("============Transaction============");

    // let generated = config::generate_address::generate_address();
    let address = Address::from_str("ckt1qzda0cr08m85hc8jlnfp3zer7xulejywt49kt2rr0vthywaa50xwsqwxy50ykktq65vrd7pteuffl8jlkwkth2gvr4x5p").expect("invalid address");
    let balance = config::balance::get_balance(&address);
    println!("balance: {}", balance.unwrap());
    let generated = config::generate_mnemonic::generate_mnemonic();
    println!("mnemonic: {}", generated.mnemonic);
    println!("address: {}", generated.address);
    println!("public_key: {}", generated.public_key);
}