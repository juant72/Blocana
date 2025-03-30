//! Transaction pool for managing pending transactions

use log::debug;
use std::collections::HashMap;
use crate::transaction::Transaction;
use crate::types::Hash;
use std::time::{Instant, Duration};
use crate::state::BlockchainState;
use crate::Error;
use std::cmp::Ordering;

/// Configuration for the transaction pool
#[derive(Debug, Clone)]
pub struct TransactionPoolConfig {
    /// Maximum number of transactions in the pool
    pub max_size: usize,
    /// Maximum transaction age before expiry (in seconds)
    pub expiry_time: u64,
    /// Maximum memory size of pool in bytes
    pub max_memory: usize,
    /// Minimum fee per byte for acceptance
    pub min_fee_per_byte: u64,
}

impl Default for TransactionPoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,
            expiry_time: 3600, // 1 hour
            max_memory: 32 * 1024 * 1024, // 32 MB
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
    /// Insufficient balance
    InsufficientBalance,
    /// Other errors
    Other(String),
}

// Implementar Display para PoolError
impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::PoolFull => write!(f, "Transaction pool is full"),
            PoolError::DuplicateTransaction => write!(f, "Transaction already exists in pool"),
            PoolError::FeeTooLow => write!(f, "Transaction fee is too low"),
            PoolError::InvalidSignature => write!(f, "Transaction has invalid signature"),
            PoolError::InvalidFormat => write!(f, "Transaction has invalid format"),
            PoolError::InsufficientBalance => write!(f, "Insufficient balance for transaction"),
            PoolError::Other(s) => write!(f, "Other pool error: {}", s),
        }
    }
}

// También es buena práctica implementar std::error::Error
impl std::error::Error for PoolError {}

/// A pooled transaction with metadata
struct PooledTransaction {
    /// The transaction
    transaction: Transaction,
    /// When the transaction was added
    added_time: Instant,
    /// Whether the transaction is valid
    is_valid: bool,
}

/// Fee-indexed transaction entry
struct TransactionWithFee {
    /// Transaction hash
    tx_hash: Hash,
    /// Fee per byte for priority sorting
    fee_per_byte: u64,
}

/// A pool for storing pending transactions
pub struct TransactionPool {
    /// Transaction pool configuration
    config: TransactionPoolConfig,
    /// Pending transactions with their timestamps
    txs: HashMap<Hash, PooledTransaction>,
    /// Transactions ordered by fee (for mining priority)
    by_fee: Vec<TransactionWithFee>, 
}

impl TransactionPool {
    /// Creates a new transaction pool with default configuration
    pub fn new() -> Self {
        Self::with_config(TransactionPoolConfig::default())
    }

    /// Creates a transaction pool with custom configuration
    pub fn with_config(config: TransactionPoolConfig) -> Self {
        Self {
            txs: HashMap::new(),
            by_fee: Vec::new(),
            config,
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(&mut self, tx: Transaction, state: &mut BlockchainState) -> Result<Hash, Error> {
        // Check if pool is full
        if self.txs.len() >= self.config.max_size {
            return Err(PoolError::PoolFull.into());
        }
        
        // Validate transaction
        self.validate_transaction(&tx)?;
        
        // Calculate fee per byte
        let tx_size = tx.estimate_size() as u64;
        let fee_per_byte = if tx_size > 0 { tx.fee / tx_size } else { tx.fee };
        
        if fee_per_byte < self.config.min_fee_per_byte {
            return Err(PoolError::FeeTooLow.into());
        }
        
        // Calculate transaction hash
        let tx_hash = tx.hash();
        
        // Check for duplicate
        if self.txs.contains_key(&tx_hash) {
            return Err(PoolError::DuplicateTransaction.into());
        }
        
        // Check if sender has enough balance
        let sender_state = state.get_account_state(&tx.sender);
        let required = tx.amount.saturating_add(tx.fee);
        let is_valid = sender_state.balance >= required;
        
        if !is_valid {
            debug!(
                "Insufficient balance: has {}, needs {}",
                sender_state.balance, required
            );
            return Err(PoolError::InsufficientBalance.into());
        }
        
        // Store transaction with timestamp
        self.txs.insert(tx_hash, PooledTransaction {
            transaction: tx,
            added_time: Instant::now(),
            is_valid: true,
        });
        
        // Add to fee-ordered list
        self.by_fee.push(TransactionWithFee {
            tx_hash,
            fee_per_byte,
        });
        
        // Sort by fee (highest first)
        self.by_fee.sort_by(|a, b| b.fee_per_byte.cmp(&a.fee_per_byte));
        
        Ok(tx_hash)
    }
    
    /// Select transactions for inclusion in a block
    pub fn select_transactions(
        &self, 
        max_count: usize, 
        state: &mut BlockchainState
    ) -> Vec<Transaction> {
        // Explicit type annotation
        let mut result: Vec<Transaction> = Vec::new();
        let mut used_senders = HashMap::new();
        
        // Create a sorted list of transactions by fee
        let mut sorted_txs = Vec::new();
        for tx_with_fee in &self.by_fee {
            if let Some(pooled_tx) = self.txs.get(&tx_with_fee.tx_hash) {
                sorted_txs.push((tx_with_fee, pooled_tx));
            }
        }
        
        // Sort by fee first, then by time
        sorted_txs.sort_by(|a, b| {
            a.0.fee_per_byte.cmp(&b.0.fee_per_byte).reverse()
                .then(a.1.added_time.cmp(&b.1.added_time))
        });
        
        // Iterate in order of priority
        for (_, pooled_tx) in sorted_txs {
            if result.len() >= max_count {
                break;
            }
            
            let tx = &pooled_tx.transaction;
            let sender = &tx.sender;
            
            // Check if we already included a transaction from this sender
            let expected_nonce = match used_senders.get(sender) {
                Some(&nonce) => nonce,
                None => {
                    // Get current nonce from state
                    state.get_account_state(sender).nonce
                }
            };
            
            // Check nonce is sequential
            if tx.nonce != expected_nonce {
                continue; // Skip this transaction
            }
            
            // Get sender's current balance
            let mut balance = state.get_account_state(sender).balance;
            
            // Deduct amounts from transactions we've already selected
            for prev_tx in &result {
                if &prev_tx.sender == sender {
                    balance = balance.saturating_sub(prev_tx.amount + prev_tx.fee);
                }
            }
            
            // Check if sender has enough balance
            if balance < tx.amount + tx.fee {
                continue; // Skip this transaction
            }
            
            // Add transaction to result
            result.push(tx.clone());
            
            // Track the next expected nonce for this sender
            used_senders.insert(*sender, tx.nonce + 1);
        }
        
        result
    }
    
    /// Remove expired transactions
    pub fn remove_expired(&mut self) -> usize {
        let max_age = Duration::from_secs(self.config.expiry_time);
        let now = Instant::now();
        let mut expired_hashes = Vec::new();
        
        for (hash, pooled_tx) in &self.txs {
            if now.duration_since(pooled_tx.added_time) > max_age {
                expired_hashes.push(*hash);
            }
        }
        
        let count = expired_hashes.len();
        for hash in expired_hashes {
            self.remove_transaction(&hash);
        }
        
        count
    }
    
    /// Validate a transaction before adding to pool
    fn validate_transaction(&self, tx: &Transaction) -> Result<(), PoolError> {
        // Validate signature
        if tx.verify().is_err() {
            return Err(PoolError::InvalidSignature);
        }
        
        // Basic validation
        if tx.amount == 0 && tx.data.is_empty() {
            return Err(PoolError::InvalidFormat);
        }
        
        Ok(())
    }
    
    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, hash: &Hash) -> bool {
        if self.txs.remove(hash).is_some() {
            self.by_fee.retain(|tx| &tx.tx_hash != hash);
            true
        } else {
            false
        }
    }
    
    /// Remove lowest priority transactions when pool is full
    fn remove_lowest_priority_transactions(&mut self, count: usize) -> usize {
        if self.txs.is_empty() {
            return 0;
        }
        
        // Create a sorted list by fee (lowest first)
        let mut sorted_txs: Vec<_> = self.txs.keys().collect();
        sorted_txs.sort_by(|a, b| {
            // Use proper dereferencing for HashMap::get
            if let (Some(tx_a), Some(tx_b)) = (self.txs.get(*a), self.txs.get(*b)) {
                let size_a = tx_a.transaction.estimate_size() as u64;
                let size_b = tx_b.transaction.estimate_size() as u64;
                let fee_per_byte_a = if size_a > 0 { tx_a.transaction.fee / size_a } else { tx_a.transaction.fee };
                let fee_per_byte_b = if size_b > 0 { tx_b.transaction.fee / size_b } else { tx_b.transaction.fee };
                
                fee_per_byte_a.cmp(&fee_per_byte_b) // Sort by fee_per_byte ascending (lowest first)
            } else {
                Ordering::Equal
            }
        });
        
        // Take the lowest fee transactions up to count
        let to_remove: Vec<Hash> = sorted_txs.into_iter()
            .take(count)
            .cloned()
            .collect();
        
        // Remove the selected transactions
        let mut removed = 0;
        for hash in to_remove {
            if self.remove_transaction(&hash) {
                removed += 1;
            }
        }
        
        removed
    }
    
    /// Get the number of transactions in the pool
    pub fn len(&self) -> usize {
        self.txs.len()
    }
    
    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }
    
    /// Get a transaction from the pool
    pub fn get_transaction(&self, hash: &Hash) -> Option<&Transaction> {
        self.txs.get(hash).map(|pooled_tx| &pooled_tx.transaction)
    }
    
    /// Get all transactions currently in the pool
    ///
    /// # Returns
    /// An iterator over all transactions
    pub fn get_all_transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.txs.values().map(|pooled_tx| &pooled_tx.transaction)
    }
    
    /// Revalidate transactions against the current state
    ///
    /// This is typically called after a block is processed to update
    /// the validity status of pending transactions.
    ///
    /// # Parameters
    /// * `state` - Current blockchain state
    pub fn revalidate_transactions(&mut self, state: &mut BlockchainState) {
        for (tx_hash, pooled_tx) in self.txs.iter_mut() {
            let tx = &pooled_tx.transaction;
            
            // Get sender's current balance and nonce
            let sender_state = state.get_account_state(&tx.sender);
            
            // Check if sender has enough balance
            let required = tx.amount.saturating_add(tx.fee);
            let has_sufficient_balance = sender_state.balance >= required;
            
            // Check if nonce is still valid (should be current nonce)
            let has_valid_nonce = tx.nonce == sender_state.nonce;
            
            // Update transaction validity
            pooled_tx.is_valid = has_sufficient_balance && has_valid_nonce;
            
            if !pooled_tx.is_valid {
                debug!("Transaction {} invalidated during revalidation", hex::encode(&tx_hash[0..4]));
            }
        }
    }
}
