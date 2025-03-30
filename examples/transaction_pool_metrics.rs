//! Example demonstrating transaction pool metrics functionality
//!
//! This example shows how to monitor and analyze transaction pool performance.
//!
//! Run with: cargo run --example transaction_pool_metrics

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::TransactionPool},
    transaction::metrics::OperationType,
};

fn main() {
    // Initialize logging
    env_logger::init();
    println!("Transaction Pool Metrics Example");
    
    // Create a transaction pool
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Generate keypairs
    let sender_keypair = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender_keypair.public_key).balance = 10000;
    
    // Simulate various operations to generate metrics
    println!("Performing various pool operations...");
    
    // 1. Add transactions of different sizes
    for i in 0..20 {
        // Calculate a size that varies between small, medium, and large
        let data_size = match i % 3 {
            0 => 50,     // Small
            1 => 500,    // Medium
            _ => 2000,   // Large
        };
        
        // Calculate a fee that varies between low, medium, and high
        let fee = match i % 3 {
            0 => 20,     // Low
            1 => 100,    // Medium
            _ => 500,    // High
        };
        
        // Create and sign transaction
        let mut tx = Transaction::new(
            sender_keypair.public_key,
            recipient,
            100, // amount
            fee,
            i as u64, // nonce
            vec![0; data_size], // data of varying size
        );
        tx.sign(&sender_keypair.private_key).unwrap();
        
        // Add to pool
        let result = pool.add_transaction(tx, &mut state);
        if result.is_err() {
            println!("Failed to add transaction {}: {}", i, result.unwrap_err());
        } else {
            // Update nonce for next transaction
            state.get_account_state(&sender_keypair.public_key).nonce += 1;
        }
    }
    
    // 2. Select transactions
    println!("\nSelecting transactions...");
    let selected = pool.select_transactions(10, &mut state);
    println!("Selected {} transactions", selected.len());
    
    // 3. Remove some transactions
    let hash_to_remove = {
        if let Some(tx) = pool.get_all_transactions().next() {
            Some(tx.hash())
        } else {
            None
        }
    };
    
    if let Some(hash) = hash_to_remove {
        println!("\nRemoving a transaction...");
        pool.remove_transaction(&hash);
    }
    
    // 4. Run maintenance
    println!("\nPerforming maintenance...");
    let removed = pool.perform_maintenance();
    println!("Maintenance removed {} transactions", removed);
    
    // Print metrics report
    println!("\n{}", pool.generate_metrics_report());
    
    // Print detailed operation timing statistics
    println!("\nDetailed Operation Timing:");
    let metrics = pool.metrics().get_metrics();
    for op_type in &[OperationType::Add, OperationType::Select, OperationType::Remove] {
        if let (Some(total), Some(count)) = (
            metrics.operation_timings.total_duration.get(op_type),
            metrics.operation_timings.operation_count.get(op_type)
        ) {
            if *count > 0 {
                let avg_us = total.as_micros() as f64 / *count as f64;
                println!("{:?} operations: {} calls, avg: {:.2} μs", op_type, count, avg_us);
            }
        }
    }
    
    // Print memory usage statistics
    println!("\nMemory Usage History:");
    for (time, usage) in metrics.memory_history.iter().take(5) {  // Show just first few entries
        println!("T+{} seconds: {} bytes", time, usage);
    }
    println!("...");
}
