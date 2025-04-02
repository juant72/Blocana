//! Example demonstrating detailed transaction error handling
//!
//! This example shows how to use both the standard API and the enhanced
//! error reporting system side-by-side
//!
//! Run with: cargo run --example transaction_detailed_errors

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig, TransactionError}},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    println!("Transaction Error Handling Example");
    
    // Create a transaction pool with default configuration
    let pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Generate keypairs for testing
    let sender = KeyPair::generate()?;
    let recipient = KeyPair::generate()?;
    
    // Set up sender with insufficient balance
    state.get_account_state(&sender.public_key).balance = 50;
    
    // Create transaction with amount greater than balance
    let mut tx = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount (exceeds balance)
        10,  // fee
        0,   // nonce
        vec![],
    );
    tx.sign(&sender.private_key)?;
    
    println!("\nDemostrando dos formas diferentes de validar transacciones:");
    
    // Approach 1: Using standard API
    println!("\n1. API Estándar (errores compatibles hacia atrás)");
    match pool.verify_transaction(&tx, &mut state) {
        Ok(_) => println!("Transacción válida"),
        Err(e) => println!("Error: {}", e),
    }
    
    // Approach 2: Using detailed error API
    println!("\n2. API Detallada (errores enriquecidos)");
    match pool.validate_transaction(&tx, &mut state) {  // Cambiar a &mut state
        Ok(_) => println!("Transacción válida"),
        Err(e) => {
            println!("Error: {}", e);
            
            // Verificar si es un error de balance usando pattern matching
            match &e {
                TransactionError::InsufficientBalance { balance, required, .. } => {
                    println!("\nDetalles de error de balance:");
                    println!("  Balance disponible: {}", balance);
                    println!("  Balance requerido: {}", required);
                    println!("  Déficit: {}", required - balance);
                    
                    println!("\nPosibles soluciones:");
                    println!("  1. Reducir el monto de la transacción");
                    println!("  2. Esperar a recibir fondos adicionales");
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}
