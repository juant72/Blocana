//! Blockchain state management
//!
//! This module handles the account state and state transitions in the blockchain.

use crate::transaction::Transaction;
use crate::types::PublicKeyBytes;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Account state structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    /// Account balance
    pub balance: u64,
    /// Transaction counter for replay protection
    pub nonce: u64,
    /// Optional smart contract code (for future use)
    pub code: Option<Vec<u8>>,
    /// Account storage (for future smart contract use)
    pub storage: HashMap<[u8; 32], Vec<u8>>,
}

impl AccountState {
    /// Create a new empty account state
    pub fn new() -> Self {
        Self {
            balance: 0,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
        }
    }
    
    /// Create a new account with initial balance
    pub fn with_balance(balance: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
        }
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global blockchain state
#[derive(Debug, Clone, Default)]
pub struct BlockchainState {
    /// Mapping of account addresses to their states
    pub accounts: HashMap<PublicKeyBytes, AccountState>,
}

impl BlockchainState {
    /// Create a new empty blockchain state
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }
    
    /// Get account state, creates a new empty account if it doesn't exist
    pub fn get_account_state(&mut self, address: &PublicKeyBytes) -> &mut AccountState {
        self.accounts.entry(*address).or_insert_with(AccountState::new)
    }
    
    /// Apply a transaction to the state
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), crate::Error> {
        // Get or create sender account
        let sender_account = self.get_account_state(&tx.sender);
        
        // Verify nonce
        if tx.nonce != sender_account.nonce {
            return Err(crate::Error::Validation(format!(
                "Invalid nonce: expected {}, got {}",
                sender_account.nonce,
                tx.nonce
            )));
        }
        
        // Verify balance
        let total_deduction = tx.amount.saturating_add(tx.fee);
        if sender_account.balance < total_deduction {
            return Err(crate::Error::Validation(format!(
                "Insufficient balance: has {}, needs {}",
                sender_account.balance,
                total_deduction
            )));
        }
        
        // Deduct from sender
        sender_account.balance = sender_account.balance.saturating_sub(total_deduction);
        // Increment sender's nonce
        sender_account.nonce += 1;
        
        // Add to recipient (create if doesn't exist)
        let recipient_account = self.get_account_state(&tx.recipient);
        recipient_account.balance = recipient_account.balance.saturating_add(tx.amount);
        
        // Note: Fees are collected separately by validators
        
        Ok(())
    }
    
    /// Apply a block's transactions to the state
    pub fn apply_block(&mut self, block: &crate::block::Block) -> Result<(), crate::Error> {
        for tx in &block.transactions {
            self.apply_transaction(tx)?;
        }
        
        Ok(())
    }
    
    /// Create genesis state with initial account balances
    pub fn genesis_state(initial_balances: HashMap<PublicKeyBytes, u64>) -> Self {
        let mut state = Self::new();
        
        for (address, balance) in initial_balances {
            state.accounts.insert(address, AccountState::with_balance(balance));
        }
        
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_account_state_creation() {
        let account = AccountState::new();
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
        assert!(account.code.is_none());
        assert!(account.storage.is_empty());
        
        let account_with_balance = AccountState::with_balance(1000);
        assert_eq!(account_with_balance.balance, 1000);
        assert_eq!(account_with_balance.nonce, 0);
    }
    
    #[test]
    fn test_blockchain_state() {
        let mut state = BlockchainState::new();
        let address = [1u8; 32];
        
        // Test getting an account that doesn't exist yet
        let account = state.get_account_state(&address);
        assert_eq!(account.balance, 0);
        
        // Update the account
        account.balance = 500;
        
        // Verify it was updated
        let retrieved_account = state.get_account_state(&address);
        assert_eq!(retrieved_account.balance, 500);
    }
    
    #[test]
    fn test_apply_transaction() {
        let mut state = BlockchainState::new();
        
        // Create sender with initial balance
        let sender = [1u8; 32];
        let recipient = [2u8; 32];
        
        state.accounts.insert(
            sender, 
            AccountState::with_balance(1000)
        );
        
        // Create a valid transaction
        let tx = Transaction::new(
            sender,
            recipient,
            500,
            10,
            0, // Correct nonce
            vec![],
        );
        
        // Apply transaction
        let result = state.apply_transaction(&tx);
        assert!(result.is_ok());
        
        // Check account states
        let sender_balance = {
            let sender_account = state.get_account_state(&sender);
            sender_account.balance
        };
        
        let recipient_balance = {
            let recipient_account = state.get_account_state(&recipient);
            recipient_account.balance
        };
        
        // Assertions using the copied values
        assert_eq!(sender_balance, 490); // 1000 - 500 - 10(fee)
        assert_eq!(recipient_balance, 500); // received 500
    }
}
