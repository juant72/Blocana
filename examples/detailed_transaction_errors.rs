//! Example demonstrating detailed transaction error handling
//!
//! This example shows how to use the enhanced transaction error system
//! to provide better feedback and diagnostics.
//!
//! Run with: cargo run --example detailed_transaction_errors

use blocana::{
    crypto::KeyPair,
    state::BlockchainState,
    transaction::{Transaction, pool::{TransactionPool, TransactionPoolConfig}},
    transaction::error::TransactionError,
    Error,
};


// Extension trait to add error classification methods
trait TransactionErrorExt {
    fn is_balance_error(&self) -> bool;
    fn is_nonce_error(&self) -> bool;
    fn is_fee_error(&self) -> bool;
    fn expected_nonce(&self) -> Option<u64>;
    fn minimum_required_fee(&self) -> Option<u64>;
    fn log_context(&self) -> String;
}

// Implementar el trait para Error en lugar de para TransactionError
impl TransactionErrorExt for Error {
    fn is_balance_error(&self) -> bool {
        if let Error::Validation(msg) = self {
            msg.contains("Insufficient balance")
        } else {
            false
        }
    }
    
    fn is_nonce_error(&self) -> bool {
        if let Error::Validation(msg) = self {
            msg.contains("Invalid nonce")
        } else {
            false
        }
    }
    
    fn is_fee_error(&self) -> bool {
        if let Error::Validation(msg) = self {
            msg.contains("Fee too low")
        } else {
            false
        }
    }
    
    fn expected_nonce(&self) -> Option<u64> {
        if let Error::Validation(msg) = self {
            if msg.contains("Invalid nonce: expected ") {
                // Try to extract the nonce from error message
                let parts: Vec<&str> = msg.split("expected ").collect();
                if parts.len() > 1 {
                    let nonce_part = parts[1].split(',').next()?;
                    return nonce_part.parse::<u64>().ok();
                }
            }
        }
        None
    }
    
    fn minimum_required_fee(&self) -> Option<u64> {
        if let Error::Validation(msg) = self {
            if msg.contains("Fee too low") {
                // Extract minimum fee from error message
                let parts: Vec<&str> = msg.split("minimum is ").collect();
                if parts.len() > 1 {
                    let fee_str = parts[1];
                    return fee_str.parse::<u64>().ok();
                }
            }
        }
        None
    }
    
    fn log_context(&self) -> String {
        format!("Error details: {}", self)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    println!("Detailed Transaction Error Handling Example");
    
    // Create a transaction pool with default configuration
    let mut pool = TransactionPool::new();
    let mut state = BlockchainState::new();
    
    // Generate keypairs for testing
    let sender = KeyPair::generate()?;
    let recipient = KeyPair::generate()?;
    
    println!("\nDemonstrating different error scenarios:");
    
    // Scenario 1: Insufficient balance
    println!("\n1. Insufficient Balance Error");
    
    // Set up sender with insufficient balance
    state.get_account_state(&sender.public_key).balance = 50;
    
    // Create transaction with amount greater than balance
    let mut tx1 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount (exceeds balance)
        10,  // fee
        0,   // nonce
        vec![],
    );
    tx1.sign(&sender.private_key)?;
    
    // Validate transaction using new detailed API
    match pool.verify_transaction(&tx1, &mut state) {
        Ok(_) => println!("Transaction is valid (unexpected)"),
        Err(e) => {
            println!("Error: {}", e);
            
            if e.is_balance_error() {
                // We can handle balance errors specifically
                println!("This is a balance error - we can suggest solutions:");
                println!("  - Reduce the amount you're sending");
                println!("  - Add more funds to your account");
            }
        }
    }
    
    // Scenario 2: Invalid nonce
    println!("\n2. Invalid Nonce Error");
    
    // Set up sender with sufficient balance but different nonce
    state.get_account_state(&sender.public_key).balance = 1000;
    state.get_account_state(&sender.public_key).nonce = 5;
    
    // Create transaction with incorrect nonce
    let mut tx2 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount
        10,  // fee
        0,   // nonce (should be 5)
        vec![],
    );
    tx2.sign(&sender.private_key)?;
    
    // Validate transaction using new detailed API
    match pool.verify_transaction(&tx2, &mut state) {
        Ok(_) => println!("Transaction is valid (unexpected)"),
        Err(e) => {
            println!("Error: {}", e);
            
            if e.is_nonce_error() {
                // Extract the expected nonce for a helpful message
                if let Some(expected) = e.expected_nonce() {
                    println!("This is a nonce error - your transaction uses the wrong nonce.");
                    println!("  - Your current account nonce is: {}", expected);
                    println!("  - Please update your transaction with this nonce value");
                }
            }
        }
    }
    
    // Scenario 3: Fee too low
    println!("\n3. Fee Too Low Error");
    
    // Create a pool with minimum fee requirement
    let config = TransactionPoolConfig {
        min_fee_per_byte: 5, // 5 units per byte minimum
        ..Default::default()
    };
    let pool_with_fee = TransactionPool::with_config(config);
    
    // Create transaction with insufficient fee
    let data = vec![0u8; 100]; // 100 bytes of data
    let mut tx3 = Transaction::new(
        sender.public_key,
        recipient.public_key,
        100, // amount
        10,  // fee (too low for 100 bytes of data)
        5,   // correct nonce
        data,
    );
    tx3.sign(&sender.private_key)?;
    
    // Validate transaction using new detailed API
    match pool_with_fee.verify_transaction(&tx3, &mut state) {
        Ok(_) => println!("Transaction is valid (unexpected)"),
        Err(e) => {
            println!("Error: {}", e);
            
            if e.is_fee_error() {
                // Extract the minimum required fee for a helpful message
                if let Some(min_fee) = e.minimum_required_fee() {
                    let tx_size = tx3.estimate_size() as u64;
                    let total_min_fee = min_fee * tx_size; // Calculate total min fee
                    
                    println!("This is a fee error - your transaction fee is too low.");
                    println!("  - Transaction size: {} bytes", tx_size);
                    println!("  - Your fee: {} (approx. {} per byte)", tx3.fee, tx3.fee / tx_size);
                    println!("  - Minimum required: {} per byte, totaling {} for this transaction", 
                             min_fee, total_min_fee);
                    println!("  - Please increase your fee to at least {}", total_min_fee);
                }
            }
        }
    }
    
    // Demonstrate using the original API with the new error handling internally
    println!("\n4. Using Original API (with enhanced errors internally)");
    
    match pool.add_transaction(tx1.clone(), &mut state) {
        Ok(_) => println!("Transaction added successfully (unexpected)"),
        Err(e) => println!("Standard error: {}", e),
    }
    
    // Demonstrate logging context
    println!("\n5. Enhanced Error Logging");
    
    // Create an insufficient balance error
    let error = TransactionError::InsufficientBalance {
        sender: sender.public_key,
        balance: 50,
        required: 110,
    };
    
    // Show both standard and enhanced logging
    println!("Standard error message: {}", error);
    println!("Enhanced log context:   {}", error.log_context());
    
    Ok(())
}