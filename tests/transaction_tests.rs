//! Integration tests for transaction module functionality
//!
//! These tests verify that the transaction and transaction pool
//! modules work correctly with other components of the system.

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
};

// Test transaction creation, signing, and validation
#[test]
fn test_transaction_lifecycle() {
    // Generate test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = KeyPair::generate().unwrap();
    
    // Create unsigned transaction
    let mut tx = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount
        10,  // fee
        0,   // nonce
        vec![1, 2, 3], // some data
    );
    
    // Initially, the signature should be all zeros
    assert_eq!(tx.signature, [0u8; 64]);
    
    // Sign the transaction
    tx.sign(&sender.private_key).unwrap();
    
    // Now the signature should be non-zero
    assert_ne!(tx.signature, [0u8; 64]);
    
    // Verify the transaction
    let result = tx.verify();
    assert!(result.is_ok());
    
    // Tamper with the transaction
    let mut tampered_tx = tx.clone();
    tampered_tx.amount = 200;
    
    // Verification should fail
    let result = tampered_tx.verify();
    assert!(result.is_err());
}

// Test transaction pool operations
#[test]
fn test_transaction_pool_basic_operations() {
    // Create a transaction pool with default configuration
    let config = TransactionPoolConfig::default();
    let mut pool = TransactionPool::with_config(config);
    
    // Create a blockchain state for testing
    let mut state = BlockchainState::new();
    
    // Generate test accounts
    let sender = KeyPair::generate().unwrap();
    let recipient = KeyPair::generate().unwrap();
    
    // Add balance to sender
    state.get_account_state(&sender.public_key).balance = 1000;
    
    // Create and sign a valid transaction
    // Se aumenta el fee de 10 a 200 para que fee_per_byte >= min_fee_per_byte
    let mut tx = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount
        200, // fee (aumentado)
        0,   // nonce
        vec![],
    );
    tx.sign(&sender.private_key).unwrap();
    
    // Add the transaction to the pool (using &mut state, según lo requiere add_transaction)
    let result = pool.add_transaction(tx.clone(), &mut state);
    assert!(result.is_ok());
    let tx_hash = result.unwrap();
    assert_eq!(tx_hash, tx.hash());
    
    // Verify the transaction was added
    assert_eq!(pool.len(), 1);
    
    // Try to add the same transaction again; this should fail (duplicate)
    let result = pool.add_transaction(tx.clone(), &mut state);
    assert!(result.is_err());
    
    // Verify pool size didn't change
    assert_eq!(pool.len(), 1);
    
    // Select transactions for a block (using &mut state)
    let selected = pool.select_transactions(10, &mut state);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].hash(), tx.hash());
    
    // Remove the transaction from the pool
    let removed = pool.remove_transaction(&tx.hash());
    assert!(removed);
    
    // Verify the pool is now empty
    assert_eq!(pool.len(), 0);
}

// Test transaction prioritization by fee
#[test]
fn test_transaction_fee_prioritization() {
    // Configuración: usar fees altos para cumplir el mínimo (min_fee_per_byte = 1)
    let config = TransactionPoolConfig::default();
    // También podrías modificar config.min_fee_per_byte si lo deseas
    let mut pool = TransactionPool::with_config(config);

    // Create a blockchain state for testing
    let mut state = BlockchainState::new();

    // Generate test accounts
    let sender1 = KeyPair::generate().unwrap();
    let sender2 = KeyPair::generate().unwrap();
    let sender3 = KeyPair::generate().unwrap();
    let recipient = KeyPair::generate().unwrap();

    // Add balances a los enviados
    state.get_account_state(&sender1.public_key).balance = 1000;
    state.get_account_state(&sender2.public_key).balance = 1000;
    state.get_account_state(&sender3.public_key).balance = 1000;

    // Crear transacciones con fees altos para asegurar fee_per_byte ≥ 1
    let mut tx1 = Transaction::new(
        sender1.public_key, recipient.public_key,
        100, 200, // tx1: fee 200 → fee_per_byte = 1
        0, vec![],
    );

    let mut tx2 = Transaction::new(
        sender2.public_key, recipient.public_key,
        100, 400, // tx2: fee 400 → fee_per_byte = 2 (la más alta)
        0, vec![],
    );

    let mut tx3 = Transaction::new(
        sender3.public_key, recipient.public_key,
        100, 300, // tx3: fee 300 → fee_per_byte = 1
        0, vec![],
    );

    // Firmar todas las transacciones
    tx1.sign(&sender1.private_key).unwrap();
    tx2.sign(&sender2.private_key).unwrap();
    tx3.sign(&sender3.private_key).unwrap();

    // Agregar las transacciones al pool
    pool.add_transaction(tx1.clone(), &mut state).unwrap();
    pool.add_transaction(tx2.clone(), &mut state).unwrap();
    pool.add_transaction(tx3.clone(), &mut state).unwrap();

    // Seleccionar solo una transacción: debe ser la de mayor fee, es decir tx2
    let selected_one = pool.select_transactions(1, &mut state);
    assert_eq!(selected_one.len(), 1);
    assert_eq!(selected_one[0].hash(), tx2.hash());

    // Seleccionar dos transacciones: el orden esperado es:
    // [tx2 (fee_per_byte = 2), tx1 (fee_per_byte = 1, agregado antes que tx3)]
    let selected_two = pool.select_transactions(2, &mut state);
    assert_eq!(selected_two.len(), 2);
    assert_eq!(selected_two[0].hash(), tx2.hash()); // Primero tx2
    assert_eq!(selected_two[1].hash(), tx1.hash()); // Luego tx1
}
