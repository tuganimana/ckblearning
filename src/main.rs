mod config;
fn main() {
    println!("Learn Rust!");


   let config= config::client::connect_client();
    // get block data
    let block = config.get_block_by_number(0.into()).unwrap();
    println!("block: {}", serde_json::to_string_pretty(&block).unwrap());

    // generate an address 
    let generated = config::generate_address::generate_address();
    println!("address: {}", generated.address);
    println!("public_key: {}", generated.public_key);
    println!("private_key: {}", generated.private_key);
}
