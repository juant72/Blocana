//! Tests for transaction pool metrics functionality
//!
//! This module validates that the transaction pool metrics system properly
//! records and reports on transaction pool operations.

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
    transaction::metrics::OperationType,
};

/// Helper function to create a test transaction
fn create_test_transaction(
    sender_keypair: &KeyPair, 
    recipient: [u8; 32], 
    amount: u64, 
    fee: u64, 
    nonce: u64,
    data_size: usize
) -> Transaction {
    let mut tx = Transaction::new(
        sender_keypair.public_key,
        recipient,
        amount,
        fee,
        nonce,
        vec![0u8; data_size], // Data of specified size
    );
    tx.sign(&sender_keypair.private_key).unwrap();
    tx
}

#[test]
fn test_metrics_basic_recording() {
    // Create pool with metrics and disable min_fee_per_byte for testing
    let config = TransactionPoolConfig {
        min_fee_per_byte: 0, // Deshabilitar requisito de tarifa mínima para pruebas
        ..Default::default()
    };
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Get initial metrics
    let initial_metrics = pool.metrics().get_metrics();
    assert_eq!(initial_metrics.transactions_added, 0);
    
    // Add a transaction
    let tx = create_test_transaction(&sender, recipient, 100, 10, 0, 50);
    pool.add_transaction(tx, &mut state).unwrap();
    
    // Verify metrics were updated
    let metrics = pool.metrics().get_metrics();
    assert_eq!(metrics.transactions_added, 1);
    assert!(metrics.avg_processing_time_us > 0);
    
    // Operation count should include the add operation
    let add_count = *metrics.operation_timings.operation_count
        .get(&OperationType::Add)
        .unwrap_or(&0);
    assert_eq!(add_count, 1);
    
    // Memory usage should be tracked
    assert!(metrics.peak_memory_usage > 0);
    assert_eq!(metrics.count_history.len(), 1);
    assert_eq!(metrics.count_history[0].1, 1);
}

#[test]
fn test_transaction_removal_metrics() {
    // Create pool with metrics and disable min_fee_per_byte for testing
    let config = TransactionPoolConfig {
        min_fee_per_byte: 0, // Deshabilitar requisito de tarifa mínima para pruebas
        ..Default::default()
    };
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Add transactions
    let tx1 = create_test_transaction(&sender, recipient, 100, 10, 0, 50);
    let tx2 = create_test_transaction(&sender, recipient, 100, 10, 1, 50);
    
    state.get_account_state(&sender.public_key).nonce = 0;
    let hash1 = pool.add_transaction(tx1, &mut state).unwrap();
    
    state.get_account_state(&sender.public_key).nonce = 1;
    let _hash2 = pool.add_transaction(tx2, &mut state).unwrap();
    
    // Remove one transaction
    pool.remove_transaction(&hash1);
    
    // Check metrics
    let metrics = pool.metrics().get_metrics();
    assert_eq!(metrics.transactions_removed, 1);
    assert_eq!(metrics.transactions_added, 2);
    
    // Transaction count should be updated in metrics
    assert_eq!(pool.len(), 1);
    assert_eq!(metrics.count_history.last().unwrap().1, 1);
}

#[test]
fn test_fee_distribution_metrics() {
    // Create pool with metrics and disable min_fee_per_byte for testing
    let config = TransactionPoolConfig {
        min_fee_per_byte: 0, // Deshabilitar requisito de tarifa mínima para pruebas
        ..Default::default()
    };
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 10000;
    
    // Add transactions with different fees
    // Low fee transaction (small, low fee)
    let tx1 = create_test_transaction(&sender, recipient, 100, 10, 0, 100);
    // Medium fee transaction (medium fee)
    let tx2 = create_test_transaction(&sender, recipient, 100, 50, 1, 100);
    // High fee transaction (high fee)
    let tx3 = create_test_transaction(&sender, recipient, 100, 200, 2, 100);
    
    state.get_account_state(&sender.public_key).nonce = 0;
    pool.add_transaction(tx1, &mut state).unwrap();
    
    state.get_account_state(&sender.public_key).nonce = 1;
    pool.add_transaction(tx2, &mut state).unwrap();
    
    state.get_account_state(&sender.public_key).nonce = 2;
    pool.add_transaction(tx3, &mut state).unwrap();
    
    // Check fee distribution metrics
    let metrics = pool.metrics().get_metrics();
    
    // Sum of all fee distributions should equal transactions added
    let total_fee_distributions: u64 = metrics.fee_distribution.values().sum();
    assert_eq!(total_fee_distributions, metrics.transactions_added);
    
    // Generate and validate report
    let report = pool.generate_metrics_report();
    assert!(report.contains("Transactions added:   3"));
    assert!(report.contains("Fee Distribution:"));
}

#[test]
fn test_operation_timing_metrics() {
    // Create pool with metrics and disable min_fee_per_byte for testing
    let config = TransactionPoolConfig {
        min_fee_per_byte: 0, // Deshabilitar requisito de tarifa mínima para pruebas
        ..Default::default()
    };
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Add a transaction
    let tx = create_test_transaction(&sender, recipient, 100, 10, 0, 50);
    pool.add_transaction(tx, &mut state).unwrap();
    
    // Select transactions
    pool.select_transactions(10, &mut state);
    
    // Optimize memory
    pool.optimize_memory();
    
    // Check operation timing metrics
    let metrics = pool.metrics().get_metrics();
    
    // Should have timing data for all operations we performed
    assert!(metrics.operation_timings.operation_count.get(&OperationType::Add).unwrap() > &0);
    assert!(metrics.operation_timings.operation_count.get(&OperationType::Select).unwrap() > &0);
    assert!(metrics.operation_timings.operation_count.get(&OperationType::Optimize).unwrap() > &0);
    
    // Duration should be recorded
    assert!(metrics.operation_timings.total_duration.get(&OperationType::Add).unwrap().as_nanos() > 0);
}

#[test]
fn test_memory_tracking() {
    // Create pool with low memory limit to force optimization
    let config = TransactionPoolConfig {
        max_memory: 5000, // 5KB limit
        max_size: 100,
        min_fee_per_byte: 0,
        expiry_time: 3600,
        replacement_fee_bump: 10, // Default or desired value for fee bump percentage
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test account
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 10000;
    
    // Add transactions until we approach the memory limit
    for i in 0..10 {
        let tx = create_test_transaction(&sender, recipient, 100, 10, i, 200);
        state.get_account_state(&sender.public_key).nonce = i;
        
        match pool.add_transaction(tx, &mut state) {
            Ok(_) => {},
            Err(_) => break, // Stop when we hit memory limit
        }
    }
    
    // Check memory history
    let metrics = pool.metrics().get_metrics();
    assert!(!metrics.memory_history.is_empty());
    
    // Memory usage should be tracked
    for (_, usage) in &metrics.memory_history {
        assert!(*usage > 0);
    }
    
    // Peak usage should be recorded
    assert!(metrics.peak_memory_usage > 0);
}
