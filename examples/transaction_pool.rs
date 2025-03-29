//! Example demonstrating the Transaction Pool functionality
//!
//! This example shows how to:
//! - Create a transaction pool
//! - Add transactions to the pool
//! - Select transactions for inclusion in blocks
//! - Handle transaction prioritization based on fees
//!
//! Run with: cargo run --example transaction_pool

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up a transaction pool
    let config = TransactionPoolConfig {
        max_size: 1000,
        expiry_time: 300, // 5 minutes
        max_memory: 10 * 1024 * 1024, // 10 MB
        min_fee_per_byte: 1,
    };
    
    let  pool = TransactionPool::with_config(config);
    println!("Transaction pool created");
    
    // Create a blockchain state for testing
    let mut state = BlockchainState::new();
    
    // Generate test accounts
    println!("Generating test accounts...");
    let alice = KeyPair::generate()?;
    let bob = KeyPair::generate()?;
    
    // Add balance to accounts
    state.get_account_state(&alice.public_key).balance = 1000;
    state.get_account_state(&bob.public_key).balance = 500;
    
    println!("Initial balances:");
    println!("- Alice: {}", state.get_account_state(&alice.public_key).balance);
    println!("- Bob: {}", state.get_account_state(&bob.public_key).balance);
    
    // Create and sign transactions
    println!("\nCreating transactions:");
    
    // High fee transaction
    let mut tx1 = Transaction::new(
        alice.public_key,
        bob.public_key,
        100, // amount
        20,  // high fee
        0,   // nonce
        vec![],
    );
    tx1.sign(&alice.private_key)?;
    println!("- Created transaction 1: 100 coins with 20 fee (high)");
    
    // Low fee transaction
    let mut tx2 = Transaction::new(
        bob.public_key,
        alice.public_key,
        50, // amount
        5,  // low fee
        0,  // nonce
        vec![],
    );
    tx2.sign(&bob.private_key)?;
    println!("- Created transaction 2: 50 coins with 5 fee (low)");
    
    // Medium fee transaction
    let mut tx3 = Transaction::new(
        alice.public_key,
        bob.public_key,
        75, // amount
        10, // medium fee
        1,  // next nonce
        vec![],
    );
    tx3.sign(&alice.private_key)?;
    println!("- Created transaction 3: 75 coins with 10 fee (medium)");
    
 
    
    // Select transactions for a block (should select in fee order)
    println!("\nSelecting transactions for block:");
    let selected = pool.select_transactions(2, &mut state);
    
    println!("Selected {} transactions:", selected.len());
    for (i, tx) in selected.iter().enumerate() {
        println!("- Transaction {}: {} coins with {} fee", 
            i+1, tx.amount, tx.fee);
    }
    
    // Simulate block processing
    println!("\nSimulating block processing...");
    // Update Alice's account state (transaction 1 included in block)
    state.get_account_state(&alice.public_key).balance -= 120; // 100 + 20 fee
    state.get_account_state(&alice.public_key).nonce = 1;
    state.get_account_state(&bob.public_key).balance += 100;
    
    println!("Updated balances after first transaction:");
    println!("- Alice: {}", state.get_account_state(&alice.public_key).balance);
    println!("- Bob: {}", state.get_account_state(&bob.public_key).balance);
    
    // Select transactions again (should now select different ones)
    let selected_after = pool.select_transactions(2, &mut state);
    println!("\nSelecting transactions again:");
    println!("Selected {} transactions:", selected_after.len());
    for (i, tx) in selected_after.iter().enumerate() {
        println!("- Transaction {}: {} coins with {} fee", 
            i+1, tx.amount, tx.fee);
    }
    
    println!("\nTransaction pool example completed successfully!");
    Ok(())
}
