//! Example demonstrating transaction replacement functionality
//!
//! This example shows how a user can replace a pending transaction
//! with a higher fee version to accelerate confirmation.
//!
//! Run with: cargo run --example transaction_replacement_example

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    println!("Transaction Replacement Example");
    
    // Create a pool with 10% required fee bump
    let config = TransactionPoolConfig {
        replacement_fee_bump: 10, // 10% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this example
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Generate keypairs
    let sender = KeyPair::generate()?;
    let recipient = KeyPair::generate()?;
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    println!("\nStep 1: Create and submit the initial transaction");
    
    // Create initial transaction with low fee
    let mut tx1 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount
        10,  // low fee
        0,   // nonce
        vec![],
    );
    tx1.sign(&sender.private_key)?;
    
    let tx1_hash = pool.add_transaction(tx1.clone(), &mut state)?;
    println!("Added transaction with hash: {}", hex::encode(&tx1_hash[0..8]));
    println!("Transaction fee: {}", tx1.fee);
    
    // Create a replacement with higher fee
    let mut tx2 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // same amount
        12,  // higher fee (+20%)
        0,   // same nonce
        vec![],
    );
    tx2.sign(&sender.private_key)?;
    
    // Try to add without replacement - should fail
    println!("\nStep 2: Try to add a transaction with the same nonce without replacement");
    let result = pool.add_transaction(tx2.clone(), &mut state);
    match result {
        Ok(_) => println!("Transaction added successfully (this shouldn't happen)"),
        Err(e) => println!("Failed as expected: {}", e),
    }
    
    // Try with replacement - should succeed
    println!("\nStep 3: Add the replacement transaction with higher fee");
    let tx2_hash = pool.add_transaction_with_replacement(tx2.clone(), &mut state, true)?;
    println!("Successfully replaced transaction with hash: {}", hex::encode(&tx2_hash[0..8]));
    println!("New transaction fee: {}", tx2.fee);
    
    // Verify original transaction was removed
    println!("\nStep 4: Verify the original transaction was replaced");
    let tx1_exists = pool.get_transaction(&tx1_hash).is_some();
    let tx2_exists = pool.get_transaction(&tx2_hash).is_some();
    
    println!("Original transaction still in pool: {}", tx1_exists);
    println!("Replacement transaction in pool: {}", tx2_exists);
    
    // Create a replacement with insufficient fee
    println!("\nStep 5: Try to replace with insufficient fee bump");
    let mut tx3 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // same amount
        13,  // only 8% higher than tx2, below 10% requirement
        0,   // same nonce
        vec![],
    );
    tx3.sign(&sender.private_key)?;
    
    match pool.add_transaction_with_replacement(tx3.clone(), &mut state, true) {
        Ok(_) => println!("Replacement succeeded (this shouldn't happen)"),
        Err(e) => println!("Replacement failed as expected: {}", e),
    }
    
    // Get pool statistics
    println!("\nFinal pool statistics:");
    let transaction_count = pool.len();
    let memory_usage = pool.memory_usage();
    println!("Transaction count: {}", transaction_count);
    println!("Memory usage: {} bytes", memory_usage);
    
    Ok(())
}
