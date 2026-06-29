## Builder Track Weekly Report — Week 1

**Name:** Telesphore TUGANNIMANA 
**Week Ending:** 29-o6-2026

### Courses Completed

- Completed the first **two modules** of the CKB Academy:
  - **CKB Theoretical Knowledge**:
    - I've learnt what's cell and it's structure  which contain capacity,lock script, type and data
    - Script Structure   which look like this
    ```json 
    Script: {code_hash: HexString
         args: HexString
           hash_type: Uint8, there are three allowed values: {0: "data", 1: "type", 2: "data1"
           } 
      ```
   - I have also learnt about Transaction  rules. Also I learnt about Outpoint which is the index of cell
   - I have also learn the difference between lock script and type script
   - 1CKB = 1Byte

  - **CKB Transaction:** 
  -  I am still learning  how to send transaction on academy- I have been gettinng an error 
  - I have created account and conencted with CKB academy 
  - I have learnt about live cell and blocks and the way they are structured - I also checked the CKB testnet chain
    - Deep dive into `cell_deps`, `witnesses`, `lock` and `type` scripts
    - Understood code locating and execution mechanisms in CKB

- I also dived into rust using  [Rust Book](https://doc.rust-lang.org/book/)

### Key Learnings

- Learnt some basic skills **Rust** language :
  - Variables and mutability
  - Functions
  - Data types
  - Momory management
  - Control flow
  - Enum
- I have started  building the project following the ckb-rust-sdk .
### Practical Progress

- **Set up a local CKB dev node** successfully
- Setup rust 
-  setup the. rust project , andlearnt about en variable and module in Rust
- Began using offck CLI tools:
  - to create  `smart-contract`
  - `transfer`, `balance`
  - `wallet get-live-cells`

### Environment
- Rust and Cargo installed
- CKB node and local dev environment installed and functional
- Basic CLI usage and debugging started 
