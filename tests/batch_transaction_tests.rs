//! Tests for batch transaction processing in the transaction pool
//!
//! This module verifies that the transaction pool correctly handles batches
//! of transactions, respecting dependencies and ensuring proper state updates.

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
};

#[test]
fn test_batch_add_independent_transactions() {
    // Create pool and state
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Create 10 different sender/recipient pairs
    let senders: Vec<KeyPair> = (0..10).map(|_| KeyPair::generate().unwrap()).collect();
    let recipients: Vec<KeyPair> = (0..10).map(|_| KeyPair::generate().unwrap()).collect();
    
    // Add balance to senders
    for sender in &senders {
        state.get_account_state(&sender.public_key).balance = 1000;
    }
    
    // Create batch of transactions from different senders
    let mut batch = Vec::new();
    for i in 0..10 {
        let mut tx = Transaction::new(
            senders[i].public_key,
            recipients[i].public_key,
            100, // amount
            10,  // fee
            0,   // nonce
            vec![],
        );
        tx.sign(&senders[i].private_key).unwrap();
        batch.push(tx);
    }
    
    // Add transactions in batch
    let (successful, failed) = pool.add_transactions_batch(batch, &mut state);
    
    // All should succeed
    assert_eq!(successful.len(), 10);
    assert_eq!(failed.len(), 0);
    
    // Pool should contain 10 transactions
    assert_eq!(pool.len(), 10);
    
    // State should be updated for all senders
    for sender in &senders {
        let account = state.get_account_state(&sender.public_key);
        assert_eq!(account.balance, 890); // 1000 - 100 - 10 fee
        assert_eq!(account.nonce, 1);
    }
}

#[test]
fn test_batch_add_dependent_transactions() {
    // Create pool and state
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Create a sender and recipient
    let sender = KeyPair::generate().unwrap();
    let recipient = KeyPair::generate().unwrap();
    
    // Add initial balance
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create a batch of transactions with sequential nonces
    let mut batch = Vec::new();
    for i in 0..5 {
        let mut tx = Transaction::new(
            sender.public_key,
            recipient.public_key,
            50, // amount
            10, // fee
            i,  // sequential nonce
            vec![],
        );
        tx.sign(&sender.private_key).unwrap();
        batch.push(tx);
    }
    
    // Add transactions in batch but in reverse nonce order
    // to test dependency handling
    batch.reverse();
    
    // Add transactions in batch
    let (successful, failed) = pool.add_transactions_batch(batch, &mut state);
    
    // All should succeed due to proper sorting
    assert_eq!(successful.len(), 5);
    assert_eq!(failed.len(), 0);
    
    // Pool should contain 5 transactions
    assert_eq!(pool.len(), 5);
    
    // State should reflect all transactions
    let sender_account = state.get_account_state(&sender.public_key);
    assert_eq!(sender_account.balance, 700); // 1000 - (50+10)*5
    assert_eq!(sender_account.nonce, 5);
    
    // Check recipient received funds
    let recipient_account = state.get_account_state(&recipient.public_key);
    assert_eq!(recipient_account.balance, 250); // 50*5
}

#[test]
fn test_batch_add_mixed_success_failure() {
    // Create pool and state
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Create sender with limited balance
    let sender = KeyPair::generate().unwrap();
    let recipient = KeyPair::generate().unwrap();
    
    // Add just enough balance for 3 transactions
    state.get_account_state(&sender.public_key).balance = 180; // (50+10)*3
    
    // Create 5 transactions (more than balance allows)
    let mut batch = Vec::new();
    for i in 0..5 {
        let mut tx = Transaction::new(
            sender.public_key,
            recipient.public_key,
            50, // amount
            10, // fee
            i,  // sequential nonce
            vec![],
        );
        tx.sign(&sender.private_key).unwrap();
        batch.push(tx);
    }
    
    // Add transactions in batch
    let (successful, failed) = pool.add_transactions_batch(batch, &mut state);
    
    // Only the first 3 should succeed
    assert_eq!(successful.len(), 3);
    assert_eq!(failed.len(), 2);
    
    // Pool should contain 3 transactions
    assert_eq!(pool.len(), 3);
    
    // State should reflect successful transactions
    let sender_account = state.get_account_state(&sender.public_key);
    assert_eq!(sender_account.balance, 0); // 180 - (50+10)*3
    assert_eq!(sender_account.nonce, 3);
}

#[test]
fn test_batch_performance() {
    // Create pool and state
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Create many senders
    const NUM_TX: usize = 200;
    let senders: Vec<KeyPair> = (0..NUM_TX).map(|_| KeyPair::generate().unwrap()).collect();
    let recipient = KeyPair::generate().unwrap();
    
    // Add balance to senders
    for sender in &senders {
        state.get_account_state(&sender.public_key).balance = 1000;
    }
    
    // Create batch of transactions
    let mut batch = Vec::new();
    for i in 0..NUM_TX {
        let mut tx = Transaction::new(
            senders[i].public_key,
            recipient.public_key,
            100, // amount
            10,  // fee
            0,   // nonce
            vec![],
        );
        tx.sign(&senders[i].private_key).unwrap();
        batch.push(tx);
    }
    
    // First measure time for individual adds
    let mut pool2 = TransactionPool::new();
    let mut state2 = state.clone();
    
    let start = std::time::Instant::now();
    for tx in batch.clone() {
        pool2.add_transaction(tx, &mut state2).unwrap();
    }
    let individual_time = start.elapsed();
    
    // Now measure batch add time
    let start = std::time::Instant::now();
    let (successful, _failed) = pool.add_transactions_batch(batch, &mut state);
    let batch_time = start.elapsed();
    
    // Verify results
    assert_eq!(successful.len(), NUM_TX);
    assert_eq!(pool.len(), NUM_TX);
    
    // Print performance comparison (batch should be faster)
    println!(
        "Individual add: {:?} for {} transactions ({:?} per tx)", 
        individual_time, 
        NUM_TX, 
        individual_time / NUM_TX as u32
    );
    println!(
        "Batch add: {:?} for {} transactions ({:?} per tx)", 
        batch_time, 
        NUM_TX, 
        batch_time / NUM_TX as u32
    );
    
    // Just to ensure compiler doesn't optimize away comparisons
    assert!(batch_time <= individual_time * 2, "Batch processing should not be dramatically slower");
}
