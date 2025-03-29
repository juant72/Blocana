//! Comprehensive test program for the Blocana blockchain
//! 
//! This example demonstrates a complete workflow of the Blocana blockchain:
//! - Setting up storage
//! - Creating key pairs for validators and users
//! - Initializing genesis state and block
//! - Creating, signing, and validating transactions
//! - Creating and validating blocks
//! - Applying transactions to update state
//! - Verifying database integrity
//!
//! Run with: cargo run --example blockchain_test

use std::collections::HashMap;
use std::path::Path;
use blocana::{
    block::Block,
    crypto::KeyPair,
    state::BlockchainState,
    storage::{BlockchainStorage, StorageConfig},
    transaction::Transaction,
};

/// Main function that runs the complete blockchain test
///
/// This demonstrates all core components of Blocana working together.
///
/// # Returns
/// `Ok(())` if the test completes successfully, otherwise an error
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    println!("Starting Blocana integration test...");
    
    // 1. Set up temporary storage for the test
    let test_db_path = "test_blockchain_db";
    if Path::new(test_db_path).exists() {
        std::fs::remove_dir_all(test_db_path)?;
    }
    
    let config = StorageConfig {
        db_path: test_db_path.into(),
        ..Default::default()
    };
    
    // 2. Initialize storage
    let storage = BlockchainStorage::open(&config)?;
    println!("✅ Storage successfully initialized");
    
    // 3. Generate key pairs for testing
    println!("Generating test accounts...");
    let validator_keypair = KeyPair::generate()?;
    let user1_keypair = KeyPair::generate()?;
    let user2_keypair = KeyPair::generate()?;
    
    // 4. Create initial state with account balances
    let mut initial_balances = HashMap::new();
    initial_balances.insert(user1_keypair.public_key, 1000);
    initial_balances.insert(user2_keypair.public_key, 500);
    
    let mut state = BlockchainState::genesis_state(initial_balances);
    println!("✅ Initial state created with account balances");
    
    // 5. Create genesis block
    let genesis_block = Block::genesis(validator_keypair.public_key, Vec::new())?;
    let genesis_hash = genesis_block.header.hash();
    
    // 6. Store genesis block
    storage.store_block(&genesis_block)?;
    println!("✅ Genesis block created and stored with hash: {}", hex::encode(genesis_hash));
    
    // 7. Create a transaction from user1 to user2
    println!("Creating test transaction...");
    let mut tx = Transaction::new(
        user1_keypair.public_key, // sender
        user2_keypair.public_key, // recipient
        100,                      // amount
        10,                       // fee
        0,                        // initial nonce
        vec![],                   // data
    );
    
    // 8. Sign the transaction
    tx.sign(&user1_keypair.private_key)?;
    println!("✅ Transaction created and signed");
    
    // 9. Verify the transaction
    tx.verify()?;
    println!("✅ Transaction signature verified");
    
    // 10. Create a block with the transaction
    let block1 = Block::new(
        genesis_hash,
        1,
        vec![tx],
        validator_keypair.public_key,
    )?;
    
    // 11. Sign the block
    let mut header = block1.header.clone();
    header.sign(&validator_keypair.private_key)?;
    
    let block1 = Block {
        header,
        transactions: block1.transactions,
    };
    
    // 12. Verify the block
    block1.validate()?;
    println!("✅ Block 1 created and validated");
    
    // 13. Store the block
    storage.store_block(&block1)?;
    println!("✅ Block 1 successfully stored");
    
    // 14. Apply the block to update the state
    state.apply_block(&block1)?;
    println!("✅ State updated with block 1 transactions");
    
    // 15. Verify the resulting state
    {
        // Process account states one at a time
        {
            let user1_state = state.get_account_state(&user1_keypair.public_key);
            println!("User1 final state: {} (nonce: {})", user1_state.balance, user1_state.nonce);
            assert_eq!(user1_state.balance, 890); // 1000 - 100 - 10 fee
            assert_eq!(user1_state.nonce, 1);     // incremented to 1
        } // First mutable borrow ends here
    
        {
            let user2_state = state.get_account_state(&user2_keypair.public_key);
            println!("User2 final state: {}", user2_state.balance);
            assert_eq!(user2_state.balance, 600); // 500 + 100
        } // Second mutable borrow ends here
    
        println!("✅ Final state verified correctly");
    }
    
    // 16. Retrieve blocks from the database
    let block_from_db = storage.get_block(&block1.header.hash())?.expect("Block should exist");
    assert_eq!(block_from_db.header.hash(), block1.header.hash());
    println!("✅ Block successfully retrieved from database");
    
    // 17. Verify chain continuity
    let height_1_hash = storage.get_block_hash_by_height(1)?;
    assert_eq!(height_1_hash, block1.header.hash());
    println!("✅ Chain continuity verified");
    
    // 18. Verify database integrity
    assert!(storage.verify_integrity()?);
    println!("✅ Database integrity verified");
    
    // Clean up resources
    std::fs::remove_dir_all(test_db_path)?;
    println!("\n✅ Integration test completed successfully! The blockchain works correctly.");

    Ok(())
}
