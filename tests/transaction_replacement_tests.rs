//! Tests for transaction replacement functionality
//!
//! This module verifies that the transaction pool correctly handles
//! transaction replacements according to the fee bump policy.

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
};

/// Helper to create test transaction
fn create_test_transaction(
    sender_keypair: &KeyPair,
    recipient: &[u8; 32],
    amount: u64,
    fee: u64,
    nonce: u64
) -> Transaction {
    let mut tx = Transaction::new(
        sender_keypair.public_key,
        *recipient,
        amount,
        fee,
        nonce,
        vec![], // Empty data for simplicity
    );
    tx.sign(&sender_keypair.private_key).unwrap();
    tx
}

#[test]
fn test_basic_transaction_replacement() {
    // Create a pool with 10% required fee bump
    let config = TransactionPoolConfig {
        replacement_fee_bump: 10, // 10% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create initial transaction
    let tx1 = create_test_transaction(&sender, &recipient, 100, 50, 0);
    
    // Add first transaction
    let tx1_hash = pool.add_transaction(tx1.clone(), &mut state).unwrap();
    
    // Create replacement with same nonce but higher fee (60 > 50 * 1.1)
    let tx2 = create_test_transaction(&sender, &recipient, 100, 60, 0);
    
    // Try to add without replacement - should fail
    let result = pool.add_transaction(tx2.clone(), &mut state);
    assert!(result.is_err());
    
    // Try with replacement - should succeed
    let tx2_hash = pool.add_transaction_with_replacement(tx2.clone(), &mut state, true).unwrap();
    
    // Verify tx1 was removed and tx2 was added
    assert!(!pool.get_all_transactions().any(|tx| tx.hash() == tx1_hash));
    assert!(pool.get_all_transactions().any(|tx| tx.hash() == tx2_hash));
    assert_eq!(pool.len(), 1);
}

#[test]
fn test_insufficient_fee_bump() {
    // Create a pool with 10% required fee bump
    let config = TransactionPoolConfig {
        replacement_fee_bump: 10, // 10% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create initial transaction
    let tx1 = create_test_transaction(&sender, &recipient, 100, 50, 0);
    
    // Add first transaction
    let tx1_hash = pool.add_transaction(tx1.clone(), &mut state).unwrap();
    
    // Create replacement with same nonce but insufficient fee increase (54 < 50 * 1.1 = 55)
    let tx2 = create_test_transaction(&sender, &recipient, 100, 54, 0);
    
    // Try with replacement - should fail due to insufficient fee
    let result = pool.add_transaction_with_replacement(tx2.clone(), &mut state, true);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Replacement fee too low"));
    
    // Verify tx1 is still in the pool
    assert!(pool.get_all_transactions().any(|tx| tx.hash() == tx1_hash));
    assert_eq!(pool.len(), 1);
    
    // Now try with sufficient fee
    let tx3 = create_test_transaction(&sender, &recipient, 100, 60, 0);
    let tx3_hash = pool.add_transaction_with_replacement(tx3.clone(), &mut state, true).unwrap();
    
    // Verify tx1 was removed and tx3 was added
    assert!(!pool.get_all_transactions().any(|tx| tx.hash() == tx1_hash));
    assert!(pool.get_all_transactions().any(|tx| tx.hash() == tx3_hash));
}

#[test]
fn test_replace_multiple_transactions() {
    // Create a pool with 10% required fee bump
    let config = TransactionPoolConfig {
        replacement_fee_bump: 10, // 10% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 10000;
    state.get_account_state(&sender.public_key).nonce = 5; // Start at nonce 5
    
    // Add several transactions with consecutive nonces
    let tx1 = create_test_transaction(&sender, &recipient, 100, 50, 5);
    pool.add_transaction(tx1, &mut state).unwrap();
    
    state.get_account_state(&sender.public_key).nonce = 6;
    let tx2 = create_test_transaction(&sender, &recipient, 100, 50, 6);
    pool.add_transaction(tx2, &mut state).unwrap();
    
    state.get_account_state(&sender.public_key).nonce = 7;
    let tx3 = create_test_transaction(&sender, &recipient, 100, 50, 7);
    pool.add_transaction(tx3, &mut state).unwrap();
    
    // Reset state nonce for testing
    state.get_account_state(&sender.public_key).nonce = 5;
    
    // Verify we have 3 transactions in the pool
    assert_eq!(pool.len(), 3);
    
    // Replace the second transaction
    let tx2_replacement = create_test_transaction(&sender, &recipient, 100, 60, 6);
    pool.add_transaction_with_replacement(tx2_replacement, &mut state, true).unwrap();
    
    // We should still have 3 transactions
    assert_eq!(pool.len(), 3);
    
    // The replaced transaction should have the higher fee
    let tx_with_nonce_6 = pool.find_transaction_by_sender_and_nonce(&sender.public_key, 6).unwrap();
    assert_eq!(tx_with_nonce_6.fee, 60);
}

#[test]
fn test_replacement_with_different_parameters() {
    // Create a pool with 10% required fee bump
    let config = TransactionPoolConfig {
        replacement_fee_bump: 10, // 10% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient1 = [1u8; 32];
    let recipient2 = [2u8; 32]; // Different recipient
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create initial transaction
    let tx1 = create_test_transaction(&sender, &recipient1, 100, 50, 0);
    
    // Add first transaction
    pool.add_transaction(tx1, &mut state).unwrap();
    
    // Create replacement with different recipient and amount, but same nonce and higher fee
    let tx2 = create_test_transaction(&sender, &recipient2, 200, 60, 0);
    
    // Replace transaction
    pool.add_transaction_with_replacement(tx2.clone(), &mut state, true).unwrap();
    
    // Verify the replacement was successful
    let tx_in_pool = pool.find_transaction_by_sender_and_nonce(&sender.public_key, 0).unwrap();
    assert_eq!(tx_in_pool.recipient, recipient2); // Should have new recipient
    assert_eq!(tx_in_pool.amount, 200); // Should have new amount
}

#[test]
fn test_high_fee_bump_requirement() {
    // Create a pool with high fee bump requirement
    let config = TransactionPoolConfig {
        replacement_fee_bump: 50, // 50% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create initial transaction
    let tx1 = create_test_transaction(&sender, &recipient, 100, 50, 0);
    
    // Add first transaction
    pool.add_transaction(tx1, &mut state).unwrap();
    
    // Create replacement with 40% higher fee (not enough for 50% requirement)
    let tx2 = create_test_transaction(&sender, &recipient, 100, 70, 0);
    
    // Try to replace - should fail
    let result = pool.add_transaction_with_replacement(tx2, &mut state, true);
    assert!(result.is_err());
    
    // Create replacement with 60% higher fee (enough for 50% requirement)
    let tx3 = create_test_transaction(&sender, &recipient, 100, 80, 0);
    
    // Try to replace - should succeed
    let result = pool.add_transaction_with_replacement(tx3, &mut state, true);
    assert!(result.is_ok());
}

#[test]
fn test_find_transaction_by_sender_and_nonce() {
    // Create a pool with high fee bump requirement
    let config = TransactionPoolConfig {
        replacement_fee_bump: 50, // 50% fee bump required
        min_fee_per_byte: 0, // Disable min fee for this test
        ..Default::default()
    };
    
    let mut pool = TransactionPool::with_config(config);
    let mut state = BlockchainState::new();
    
    // Create test accounts
    let sender1 = KeyPair::generate().unwrap();
    let sender2 = KeyPair::generate().unwrap();
    let recipient = [1u8; 32];
    
    // Add balance to senders
    state.get_account_state(&sender1.public_key).balance = 1000;
    state.get_account_state(&sender2.public_key).balance = 1000;
    
    // Add transactions with various senders and nonces
    let tx1 = create_test_transaction(&sender1, &recipient, 100, 10, 0);
    let tx2 = create_test_transaction(&sender1, &recipient, 100, 10, 1);
    let tx3 = create_test_transaction(&sender2, &recipient, 100, 10, 0);
    
    pool.add_transaction(tx1, &mut state).unwrap();
    
    state.get_account_state(&sender1.public_key).nonce = 1;
    pool.add_transaction(tx2, &mut state).unwrap();
    
    state.get_account_state(&sender2.public_key).nonce = 0;
    pool.add_transaction(tx3, &mut state).unwrap();
    
    // Test finding by sender and nonce
    let found1 = pool.find_transaction_by_sender_and_nonce(&sender1.public_key, 0);
    assert!(found1.is_some());
    assert_eq!(found1.unwrap().nonce, 0);
    
    let found2 = pool.find_transaction_by_sender_and_nonce(&sender1.public_key, 1);
    assert!(found2.is_some());
    assert_eq!(found2.unwrap().nonce, 1);
    
    let found3 = pool.find_transaction_by_sender_and_nonce(&sender2.public_key, 0);
    assert!(found3.is_some());
    
    // Test with non-existent transactions
    let not_found1 = pool.find_transaction_by_sender_and_nonce(&sender1.public_key, 2);
    assert!(not_found1.is_none());
    
    let not_found2 = pool.find_transaction_by_sender_and_nonce(&[9u8; 32], 0);
    assert!(not_found2.is_none());
}
