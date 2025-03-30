//! Transaction pool for managing pending transactions

use log::debug;
use std::collections::{HashMap, HashSet, BTreeMap};
use crate::transaction::Transaction;
use crate::types::{Hash, PublicKeyBytes};
use std::time::{Instant, Duration};
use crate::state::BlockchainState;
use crate::Error;
use bincode;

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
#[derive(Clone)]
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
    /// Transactions indexed by sender address
    by_address: HashMap<Hash, HashSet<Hash>>,
    /// Current memory usage estimate
    memory_usage: usize,
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
            by_address: HashMap::new(),
            memory_usage: 0,
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
        let pooled_tx = PooledTransaction {
            transaction: tx.clone(),
            added_time: Instant::now(),
            is_valid: true,
        };
        
        // Add to primary index
        self.txs.insert(tx_hash, pooled_tx);
        
        // Add to fee-ordered list
        let tx_with_fee = TransactionWithFee {
            tx_hash,
            fee_per_byte,
        };
        self.by_fee.push(tx_with_fee);
        
        // Add to sender index
        self.by_address
            .entry(tx.sender)
            .or_insert_with(HashSet::new)
            .insert(tx_hash);
        
        // Update memory usage estimate
        self.memory_usage += tx_size as usize + std::mem::size_of::<PooledTransaction>() + 
                             std::mem::size_of::<TransactionWithFee>();
        
        // Check memory limit and optimize if needed
        if self.memory_usage > self.config.max_memory {
            debug!("Transaction pool memory limit reached, attempting optimization");
            if self.optimize_memory() == 0 {
                // If we couldn't optimize (remove any transactions), reject this one
                return Err(Error::Validation("Cannot add transaction due to memory constraints".into()));
            }
            
            // Double-check we're still within limits
            if self.memory_usage > self.config.max_memory {
                return Err(Error::Validation("Cannot add transaction due to memory constraints".into()));
            }
        }
        
        // Sort by fee (highest first)
        self.by_fee.sort_by(|a, b| b.fee_per_byte.cmp(&a.fee_per_byte));
        
        debug!("Added transaction to pool: {}", hex::encode(&tx_hash[0..4]));
        Ok(tx_hash)
    }
    
    /// Select transactions for inclusion in a block
    ///
    /// This method carefully handles transaction dependencies, ensuring transactions
    /// from the same sender are selected in the correct nonce order.
    ///
    /// # Parameters
    /// * `max_count` - Maximum number of transactions to select
    /// * `state` - Current blockchain state for validation
    ///
    /// # Returns
    /// A vector of valid transactions, in order of priority
    pub fn select_transactions(
        &self, 
        max_count: usize, 
        state: &mut BlockchainState
    ) -> Vec<Transaction> {
        let mut result = Vec::new();
        let mut sender_states: HashMap<PublicKeyBytes, (u64, u64)> = HashMap::new();
        
        // Create a sorted list of transactions by fee
        let mut sorted_txs: Vec<&PooledTransaction> = self.txs.values()
            .filter(|tx| tx.is_valid)
            .collect();
        
        // Sort by fee per byte (descending), then by timestamp (ascending)
        sorted_txs.sort_by(|a, b| {
            let a_fee_per_byte = self.calculate_fee_per_byte(&a.transaction);
            let b_fee_per_byte = self.calculate_fee_per_byte(&b.transaction);
            
            b_fee_per_byte.cmp(&a_fee_per_byte)
                .then_with(|| a.added_time.cmp(&b.added_time))
        });
        
        // First pass - organize by sender and nonce into potential inclusion sets
        let mut sender_queues: HashMap<PublicKeyBytes, BTreeMap<u64, &PooledTransaction>> = HashMap::new();
        
        for pooled_tx in &sorted_txs {
            let sender = pooled_tx.transaction.sender;
            let nonce = pooled_tx.transaction.nonce;
            
            sender_queues
                .entry(sender)
                .or_insert_with(BTreeMap::new)
                .insert(nonce, pooled_tx);
        }
        
        // Second pass - select transactions respecting dependencies
        let mut selected_count = 0;
        let mut remaining_txs = true;
        
        while remaining_txs && selected_count < max_count {
            remaining_txs = false;
            
            // Calculate fee-based priority for the next transaction from each sender
            let mut sender_priorities: Vec<(PublicKeyBytes, u64, &PooledTransaction)> = Vec::new();
            
            for (&sender, queue) in &sender_queues {
                if queue.is_empty() {
                    continue;
                }
                
                // Get the current state for this sender
                let (_current_balance, current_nonce) = match sender_states.get(&sender) {
                    Some(&state_data) => state_data,
                    None => {
                        let account = state.get_account_state(&sender);
                        (account.balance, account.nonce)
                    }
                };
                
                // Look for the next sequential transaction
                if let Some((&tx_nonce, tx)) = queue.iter().next() {
                    if tx_nonce == current_nonce {
                        // This transaction is next in sequence
                        let fee_per_byte = self.calculate_fee_per_byte(&tx.transaction);
                        sender_priorities.push((sender, fee_per_byte, tx));
                        remaining_txs = true;
                    }
                }
            }
            
            // If no valid transactions found, break
            if !remaining_txs {
                break;
            }
            
            // Sort by fee priority (descending)
            sender_priorities.sort_by(|a, b| b.1.cmp(&a.1));
            
            // Try to select the highest priority transaction
            if let Some((sender, _, pooled_tx)) = sender_priorities.first() {
                let tx = &pooled_tx.transaction;
                let tx_nonce = tx.nonce;
                
                // Get current account state
                let (mut current_balance, current_nonce) = sender_states
                    .get(sender)
                    .copied()
                    .unwrap_or_else(|| {
                        let account = state.get_account_state(sender);
                        (account.balance, account.nonce)
                    });
                
                // Verify nonce is correct
                if tx_nonce != current_nonce {
                    // Remove this transaction from consideration
                    if let Some(queue) = sender_queues.get_mut(sender) {
                        queue.remove(&tx_nonce);
                    }
                    continue;
                }
                
                // Verify balance is sufficient
                let total_cost = tx.amount.saturating_add(tx.fee);
                if current_balance < total_cost {
                    // Remove this transaction from consideration
                    if let Some(queue) = sender_queues.get_mut(sender) {
                        queue.remove(&tx_nonce);
                    }
                    continue;
                }
                
                // Transaction is valid - add it to results
                result.push(tx.clone());
                selected_count += 1;
                
                // Update sender state in our tracking map
                current_balance = current_balance.saturating_sub(total_cost);
                sender_states.insert(*sender, (current_balance, current_nonce + 1));
                
                // Remove this transaction from consideration
                if let Some(queue) = sender_queues.get_mut(sender) {
                    queue.remove(&tx_nonce);
                }
            }
        }
        
        result
    }
    
    /// Calculate fee per byte for a transaction
    fn calculate_fee_per_byte(&self, tx: &Transaction) -> u64 {
        let size = tx.estimate_size() as u64;
        if size == 0 {
            return tx.fee; // Avoid division by zero
        }
        tx.fee / size
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
    
    
    // Get memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }
    
    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, hash: &Hash) -> bool {
        // Remove from main index and get the transaction
        let pooled_tx = match self.txs.remove(hash) {
            Some(tx) => tx,
            None => return false,
        };
        
        let tx = &pooled_tx.transaction;
        
        // Update memory usage
        let tx_size = tx.estimate_size();
        self.memory_usage = self.memory_usage.saturating_sub(
            tx_size + std::mem::size_of::<PooledTransaction>() + 
            std::mem::size_of::<TransactionWithFee>()
        );
        
        // Remove from sender index
        if let Some(sender_txs) = self.by_address.get_mut(&tx.sender) {
            sender_txs.remove(hash);
            if sender_txs.is_empty() {
                self.by_address.remove(&tx.sender);
            }
        }
        
        // Note: We don't immediately remove from by_fee (binary heap)
        // Instead, we'll filter them out when selecting transactions
        // This avoids O(n) removal cost from the heap
        
        true
    }
    
    // Implementation moved to a single location below
    
    /// Optimizes memory usage if it exceeds the configured threshold
    ///
    /// This method is automatically called when adding transactions,
    /// but can also be called manually if needed.
    ///
    /// # Returns
    /// Number of transactions removed during optimization
    pub fn optimize_memory(&mut self) -> usize {
        // If we're below 90% of the memory limit, no action needed
        if self.memory_usage <= (self.config.max_memory * 9 / 10) {
            return 0;
        }
        
        // Calculate how much memory to free
        // Target: reduce to 80% of max memory
        let target_memory = self.config.max_memory * 8 / 10;
        let memory_to_free = self.memory_usage.saturating_sub(target_memory);
        
        // If nothing to free, return early
        if memory_to_free == 0 {
            return 0;
        }
        
        debug!("Memory usage ({} bytes) exceeds target, optimizing pool", self.memory_usage);
        
        // Estimate how many transactions to remove based on average size
        let avg_tx_size = if self.txs.is_empty() {
            200 // Reasonable default if no transactions
        } else {
            self.memory_usage / self.txs.len()
        };
        
        let tx_count_to_remove = (memory_to_free / avg_tx_size).max(1);
        debug!("Removing approximately {} transactions to free memory", tx_count_to_remove);
        
        // Remove the lowest-priority transactions
        self.remove_lowest_priority_transactions(tx_count_to_remove)
    }
    
    /// Remove lowest priority transactions from the pool
    ///
    /// # Parameters
    /// * `count` - Maximum number of transactions to remove
    ///
    /// # Returns
    /// The actual number of transactions removed
    fn remove_lowest_priority_transactions(&mut self, count: usize) -> usize {
        if self.txs.is_empty() {
            return 0;
        }
        
        // Create a copy of by_fee in vector form so we can sort
        let mut fee_entries: Vec<TransactionWithFee> = self.by_fee.iter().cloned().collect();
        
        // Sort by fee per byte (ascending) so lowest fee transactions are first
        fee_entries.sort_by(|a, b| {
            a.fee_per_byte.cmp(&b.fee_per_byte)
                .then_with(|| a.tx_hash.cmp(&b.tx_hash))
        });
        
        // Take the lowest fee transactions up to count
        let to_remove: Vec<_> = fee_entries.into_iter()
            .take(count)
            .map(|entry| entry.tx_hash)
            .collect();
        
        // Keep track of how many we actually removed
        let mut removed = 0;
        
        // Remove the selected transactions
        for hash in to_remove {
            if self.remove_transaction(&hash) {
                removed += 1;
            }
        }
        
        debug!("Memory optimization removed {} transactions", removed);
        removed
    }
    
    /// Periodic maintenance for the transaction pool
    ///
    /// This method performs regular maintenance tasks:
    /// - Removing expired transactions
    /// - Optimizing memory usage
    /// - Cleaning up internal data structures
    ///
    /// It's recommended to call this method periodically
    /// (e.g., once per minute or after processing each block)
    ///
    /// # Returns
    /// Number of transactions removed during maintenance
    pub fn perform_maintenance(&mut self) -> usize {
        let mut removed = 0;
        
        // Remove expired transactions
        removed += self.remove_expired();
        
        // Optimize memory usage if needed
        removed += self.optimize_memory();
        
        // Clean up the priority queue if needed
        if removed > 0 && self.by_fee.len() > self.txs.len() * 2 {
            // If we have a lot of "ghost" entries in the binary heap,
            // rebuild it to save memory and improve performance
            let valid_entries: Vec<_> = self.by_fee.iter()
                .filter(|entry| self.txs.contains_key(&entry.tx_hash))
                .cloned()
                .collect();
            
            self.by_fee.clear();
            for entry in valid_entries {
                self.by_fee.push(entry);
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
    
    /// Add multiple transactions to the pool in a batch operation
    ///
    /// This is more efficient than adding transactions individually when many transactions
    /// need to be added at once, as it allows for optimized database operations and
    /// minimizes redundant calculations.
    ///
    /// # Parameters
    /// * `transactions` - Vector of transactions to add
    /// * `state` - Current blockchain state (for validation)
    ///
    /// # Returns
    /// A tuple containing successful and failed transaction results
    pub fn add_transactions_batch(
        &mut self, 
        transactions: Vec<Transaction>,
        state: &mut BlockchainState
    ) -> (Vec<Hash>, Vec<(Hash, crate::Error)>) {
        let mut successful = Vec::new();
        let mut failed = Vec::new();
        
        // Create a local copy of the state that we can modify temporarily
        // to track cumulative changes as we process the batch
        let mut local_state = state.clone();
        
        // Sort transactions by sender and nonce to handle dependency chains properly
        let mut sorted_txs: Vec<_> = transactions.into_iter()
            .map(|tx| {
                let hash = tx.hash();
                (tx.sender, tx.nonce, hash, tx)
            })
            .collect();
        
        // Sort by sender first, then by nonce for each sender
        sorted_txs.sort_by(|a, b| {
            let sender_cmp = a.0.cmp(&b.0);
            if sender_cmp == std::cmp::Ordering::Equal {
                a.1.cmp(&b.1)
            } else {
                sender_cmp
            }
        });
        
        // Process each transaction in order
        for (_sender, _nonce, hash, tx) in sorted_txs {
            match self.add_transaction(tx, &mut local_state) {
                Ok(_) => {
                    successful.push(hash);
                    // Also apply changes to the original state
                    match state.apply_transaction(&self.txs.get(&hash).unwrap().transaction) {
                        Ok(_) => {},
                        Err(e) => {
                            log::error!("Failed to apply transaction to original state: {}", e);
                        }
                    }
                },
                Err(e) => {
                    failed.push((hash, e));
                }
            }
        }
        
        // If we have a lot of successful transactions, perform maintenance to clean up
        if successful.len() > 50 {
            self.perform_maintenance();
        }
        
        // Return results
        (successful, failed)
    }
    
    /// Add multiple transactions to the pool from serialized data
    ///
    /// This method is especially useful when receiving batched transactions
    /// from the network or an API endpoint. It deserializes each transaction
    /// and adds it to the pool.
    ///
    /// # Parameters
    /// * `transaction_data` - Vector of serialized transaction bytes
    /// * `state` - Current blockchain state (for validation)
    ///
    /// # Returns
    /// A tuple containing successful and failed transaction results
    pub fn add_serialized_transactions_batch(
        &mut self,
        transaction_data: Vec<Vec<u8>>,
        state: &mut BlockchainState
    ) -> (Vec<Hash>, Vec<(usize, crate::Error)>) {
        let mut successful = Vec::new();
        let mut failed = Vec::new();
        
        // First, deserialize all transactions
        let mut transactions = Vec::with_capacity(transaction_data.len());
        
        for (idx, data) in transaction_data.into_iter().enumerate() {
            match bincode::decode_from_slice::<Transaction, _>(&data, bincode::config::standard()) {
                Ok((tx, _)) => transactions.push(tx),
                Err(e) => {
                    failed.push((idx, crate::Error::Serialization(format!("Deserialization error: {}", e))));
                }
            }
        }
        
        // Process the deserialized transactions
        let (tx_successful, tx_failed) = self.add_transactions_batch(transactions, state);
        
        // Combine the results
        successful.extend(tx_successful);
        failed.extend(tx_failed.into_iter().map(|(_, e)| (0, e))); // Using 0 as index placeholder since original index is lost
        
        (successful, failed)
    }
}
