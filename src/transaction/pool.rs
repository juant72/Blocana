//! Transaction pool for managing pending transactions

use std::collections::HashMap;
use crate::transaction::Transaction;
use crate::types::Hash;
use std::time::{Instant, Duration};

/// Configuration for the transaction pool
#[derive(Debug, Clone)]
pub struct TransactionPoolConfig {
    /// Maximum number of transactions in the pool
    pub max_transactions: usize,
    /// Maximum transaction lifetime in seconds
    pub max_transaction_age_secs: u64,
    /// Minimum fee per byte for acceptance
    pub min_fee_per_byte: u64,
}

impl Default for TransactionPoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: 5000,
            max_transaction_age_secs: 3600, // 1 hour
            min_fee_per_byte: 1,
        }
    }
}

/// Error types for transaction pool operations
#[derive(Debug)]
pub enum PoolError {
    /// Pool is full
    PoolFull,
    /// Transaction already exists in pool
    DuplicateTransaction,
    /// Transaction fee is too low
    FeeTooLow,
    /// Transaction has invalid signature
    InvalidSignature,
    /// Transaction has invalid format
    InvalidFormat,
    /// Other errors
    Other(String),
}

/// A pool for storing pending transactions
pub struct TransactionPool {
    /// Transaction pool configuration
    config: TransactionPoolConfig,
    /// Pending transactions
    transactions: HashMap<Hash, (Transaction, Instant)>,
    /// Transactions ordered by fee (for mining priority)
    transactions_by_fee: Vec<(Hash, u64)>, // (tx_hash, fee_per_byte)
}

impl TransactionPool {
    /// Create a new transaction pool
    pub fn new(config: TransactionPoolConfig) -> Self {
        Self {
            config,
            transactions: HashMap::new(),
            transactions_by_fee: Vec::new(),
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(&mut self, tx: Transaction) -> Result<Hash, PoolError> {
        // Check if pool is full
        if self.transactions.len() >= self.config.max_transactions {
            return Err(PoolError::PoolFull);
        }
        
        // Validate transaction
        self.validate_transaction(&tx)?;
        
        // Calculate fee per byte
        let fee_per_byte = tx.fee / tx.serialized_size() as u64;
        if fee_per_byte < self.config.min_fee_per_byte {
            return Err(PoolError::FeeTooLow);
        }
        
        // Calculate transaction hash
        let tx_hash = tx.hash();
        
        // Check for duplicate
        if self.transactions.contains_key(&tx_hash) {
            return Err(PoolError::DuplicateTransaction);
        }
        
        // Store transaction with timestamp
        self.transactions.insert(tx_hash, (tx, Instant::now()));
        
        // Add to fee-ordered list
        let pos = self.transactions_by_fee.partition_point(|(_, fee)| *fee < fee_per_byte);
        self.transactions_by_fee.insert(pos, (tx_hash, fee_per_byte));
        
        Ok(tx_hash)
    }
    
    /// Get best transactions for inclusion in a block
    pub fn get_transactions_for_block(&self, max_size: usize) -> Vec<Transaction> {
        let mut result = Vec::new();
        let mut total_size = 0;
        
        // Get transactions in fee-per-byte order (highest first)
        for (hash, _) in self.transactions_by_fee.iter().rev() {
            if let Some((tx, _)) = self.transactions.get(hash) {
                let tx_size = tx.serialized_size();
                if total_size + tx_size <= max_size {
                    result.push(tx.clone());
                    total_size += tx_size;
                } else {
                    // Block is full
                    break;
                }
            }
        }
        
        result
    }
    
    /// Remove expired transactions
    pub fn remove_expired(&mut self) -> usize {
        let max_age = Duration::from_secs(self.config.max_transaction_age_secs);
        let now = Instant::now();
        let expired_hashes: Vec<Hash> = self.transactions
            .iter()
            .filter(|(_, (_, timestamp))| now.duration_since(*timestamp) > max_age)
            .map(|(hash, _)| *hash)
            .collect();
        
        for hash in &expired_hashes {
            self.transactions.remove(hash);
            if let Some(pos) = self.transactions_by_fee
                .iter()
                .position(|(h, _)| h == hash) {
                self.transactions_by_fee.remove(pos);
            }
        }
        
        expired_hashes.len()
    }
    
    /// Remove transactions that were included in a block
    pub fn remove_transactions(&mut self, tx_hashes: &[Hash]) -> usize {
        let mut removed = 0;
        
        for hash in tx_hashes {
            if self.transactions.remove(hash).is_some() {
                removed += 1;
                if let Some(pos) = self.transactions_by_fee
                    .iter()
                    .position(|(h, _)| h == hash) {
                    self.transactions_by_fee.remove(pos);
                }
            }
        }
        
        removed
    }
    
    /// Validate a transaction before adding to pool
    fn validate_transaction(&self, tx: &Transaction) -> Result<(), PoolError> {
        // Validate signature
        if let Err(_) = tx.verify() {
            return Err(PoolError::InvalidSignature);
        }
        
        // Check nonce (we would need account state for this in a real implementation)
        // Check balance (we would need account state for this in a real implementation)
        
        Ok(())
    }
    
    /// Get the number of transactions in the pool
    pub fn len(&self) -> usize {
        self.transactions.len()
    }
    
    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
    
    /// Get a transaction from the pool
    pub fn get_transaction(&self, hash: &Hash) -> Option<&Transaction> {
        self.transactions.get(hash).map(|(tx, _)| tx)
    }
}
