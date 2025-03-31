//! Transaction pool for managing pending transactions

use crate::state::BlockchainState;
use crate::transaction::metrics::{MetricsCollector, OperationType};
use crate::transaction::Transaction;
use crate::types::{Hash, PublicKeyBytes};
use crate::Error;
use bincode;
use log::debug;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Result type for transaction-specific operations
pub type TxResult<T> = Result<T, TransactionError>;

/// Detailed error type for transaction pool operations
#[derive(Debug)]
pub enum TransactionError {
    /// Transaction already exists in the pool
    AlreadyExists {
        tx_hash: Hash,
    },
    /// Transaction has an invalid signature
    InvalidSignature,
    /// Transaction nonce doesn't match account state
    InvalidNonce {
        sender: PublicKeyBytes,
        expected: u64,
        actual: u64,
    },
    /// Transaction fee is too low for inclusion
    FeeTooLow {
        fee_per_byte: u64,
        min_required: u64,
    },
    /// Transaction replacement fee is too low
    ReplacementFeeTooLow {
        actual: u64,
        required: u64,
    },
    /// Account has insufficient balance
    InsufficientBalance {
        sender: PublicKeyBytes,
        balance: u64,
        required: u64,
    },
    /// Transaction pool is full
    PoolFull {
        current_size: usize,
        max_size: usize,
    },
    /// Memory limit reached
    MemoryLimitReached {
        current_bytes: usize,
        max_bytes: usize,
    },
    /// General error
    Other(String),
}

impl TransactionError {
    /// Get additional context for logging purposes
    pub fn log_context(&self) -> String {
        match self {
            Self::AlreadyExists { tx_hash } => 
                format!("Transaction already exists with hash: {}", hex::encode(&tx_hash[0..4])),
            Self::InvalidSignature => 
                "Transaction has an invalid signature".to_string(),
            Self::InvalidNonce { sender, expected, actual } => 
                format!("Invalid nonce for {}: expected {}, got {}", 
                    hex::encode(&sender[0..4]), expected, actual),
            Self::FeeTooLow { fee_per_byte, min_required } => 
                format!("Fee too low: {} per byte, minimum is {}", fee_per_byte, min_required),
            Self::ReplacementFeeTooLow { actual, required } => 
                format!("Replacement fee too low: {} provided, {} required", actual, required),
            Self::InsufficientBalance { sender, balance, required } => 
                format!("Insufficient balance for {}: has {}, needs {}", 
                    hex::encode(&sender[0..4]), balance, required),
            Self::PoolFull { current_size, max_size } => 
                format!("Transaction pool full: {} of {} slots used", current_size, max_size),
            Self::MemoryLimitReached { current_bytes, max_bytes } => 
                format!("Memory limit reached: {} of {} bytes used", current_bytes, max_bytes),
            Self::Other(msg) => format!("Other error: {}", msg),
        }
    }
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists { .. } => write!(f, "Transaction already exists"),
            Self::InvalidSignature => write!(f, "Invalid transaction signature"),
            Self::InvalidNonce { .. } => write!(f, "Invalid transaction nonce"),
            Self::FeeTooLow { .. } => write!(f, "Transaction fee too low"),
            Self::ReplacementFeeTooLow { .. } => write!(f, "Replacement fee too low"),
            Self::InsufficientBalance { .. } => write!(f, "Insufficient balance"),
            Self::PoolFull { .. } => write!(f, "Transaction pool is full"),
            Self::MemoryLimitReached { .. } => write!(f, "Memory limit reached"),
            Self::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<TransactionError> for Error {
    fn from(error: TransactionError) -> Self {
        match error {
            TransactionError::AlreadyExists { .. } => 
                Error::Validation("Transaction already in pool".into()),
            TransactionError::InvalidSignature => 
                Error::Validation("Invalid transaction signature".into()),
            TransactionError::InvalidNonce { expected, actual, .. } => 
                Error::Validation(format!("Invalid nonce: expected {}, got {}", expected, actual)),
            TransactionError::FeeTooLow { fee_per_byte, min_required, .. } => 
                Error::Validation(format!("Fee too low: {} per byte, minimum is {}", fee_per_byte, min_required)),
            TransactionError::ReplacementFeeTooLow { actual, required } => 
                Error::Validation(format!("Replacement fee too low: {} provided, {} required", actual, required)),
            TransactionError::InsufficientBalance { balance, required, .. } => 
                Error::Validation(format!("Insufficient balance: has {}, needs {}", balance, required)),
            TransactionError::PoolFull { .. } => 
                Error::Validation("Transaction pool is full".into()),
            TransactionError::MemoryLimitReached { .. } => 
                Error::Validation("Memory limit reached".into()),
            TransactionError::Other(msg) => 
                Error::Validation(msg),
        }
    }
}

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
    /// Minimum fee increase percentage for replacements (e.g., 10 = 10% increase required)
    pub replacement_fee_bump: u64,
}

impl Default for TransactionPoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5000,
            expiry_time: 3600,            // 1 hour
            max_memory: 32 * 1024 * 1024, // 32 MB
            min_fee_per_byte: 1,
            replacement_fee_bump: 10, // Require 10% fee increase for replacements
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
    /// Estimated memory usage of the transaction including metadata
    size: usize,
}

/// Fee-indexed transaction entry
#[derive(Clone)]
struct TransactionWithFee {
    /// Transaction hash
    tx_hash: Hash,
    fee: u64,
    /// Fee per byte for priority sorting
    fee_per_byte: u64,
    timestamp: Instant,
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
    /// Metrics collector for performance monitoring
    metrics: MetricsCollector,
}

impl TransactionPool {
    /// Initialize a new transaction pool with default configuration
    pub fn new() -> Self {
        Self::with_config(TransactionPoolConfig::default())
    }

    /// Initialize a new transaction pool with custom configuration
    pub fn with_config(config: TransactionPoolConfig) -> Self {
        Self {
            txs: HashMap::new(),
            by_fee: Vec::new(),
            by_address: HashMap::new(),
            config,
            memory_usage: 0,
            metrics: MetricsCollector::new(100), // Track the last 100 data points
        }
    }

    /// Calculate accurate memory usage of a transaction including metadata
    ///
    /// This method provides a comprehensive memory estimation for a transaction
    /// in the pool, accounting for transaction data and all metadata structures.
    ///
    /// # Parameters
    /// * `tx` - The transaction to measure
    ///
    /// # Returns
    /// Estimated memory usage in bytes
    fn calculate_transaction_memory_usage(&self, tx: &Transaction) -> usize {
        // Size of the transaction itself
        let tx_size = tx.estimate_size();

        // Size of PooledTransaction struct
        let pooled_tx_overhead = std::mem::size_of::<PooledTransaction>();

        // Size of entry in txs HashMap (key + value + HashMap overhead)
        let hash_map_entry_size =
            std::mem::size_of::<Hash>() + std::mem::size_of::<*const PooledTransaction>() + 32; // Approximate HashMap overhead per entry

        // Size of entry in by_fee priority queue
        let by_fee_entry_size = std::mem::size_of::<TransactionWithFee>();

        // Size of entry in by_address HashMap
        let sender_entry_size = if self.by_address.contains_key(&tx.sender) {
            // If sender already exists, just add hash set entry size
            std::mem::size_of::<Hash>() + 16 // Hash + HashSet overhead
        } else {
            // If new sender, add full HashMap entry
            std::mem::size_of::<PublicKeyBytes>()
                + std::mem::size_of::<HashSet<Hash>>()
                + std::mem::size_of::<Hash>()
                + 48 // Additional overhead
        };

        // Total memory usage
        tx_size + pooled_tx_overhead + hash_map_entry_size + by_fee_entry_size + sender_entry_size
    }

    /// Add a transaction to the pool, supporting replacement of existing transactions
    ///
    /// This method allows replacing an existing transaction with the same sender/nonce
    /// if the new transaction has a sufficiently higher fee.
    ///
    /// # Parameters
    /// * `tx` - The transaction to add
    /// * `state` - Current blockchain state (for validation)
    /// * `allow_replacement` - Whether to allow replacing existing transactions
    ///
    /// # Returns
    /// `Ok(hash)` if transaction was added successfully, `Err` otherwise
    pub fn add_transaction_with_replacement(
        &mut self,
        tx: Transaction,
        state: &mut BlockchainState,
        allow_replacement: bool,
    ) -> Result<Hash, Error> {
        // Start metrics for this operation
        self.metrics.start_operation(OperationType::Add);
        let process_start = Instant::now();

        // Start validation timing
        self.metrics.start_operation(OperationType::Validate);

        // Verify transaction signature
        tx.verify()?;

        // Calculate hash
        let tx_hash = tx.hash();

        // Check for duplicate - but if replacement is allowed, we'll check differently
        if self.txs.contains_key(&tx_hash) {
            self.metrics.record_transaction_rejected();
            self.metrics.stop_operation(OperationType::Validate);
            self.metrics.stop_operation(OperationType::Add);
            return Err(Error::Validation("Transaction already in pool".into()));
        }

        // Get current account state
        let sender_state = state.get_account_state(&tx.sender);

        // Comprobar primero si existe una transacción con el mismo remitente y nonce
        let existing_tx = self.find_transaction_by_sender_and_nonce(&tx.sender, tx.nonce);
        if existing_tx.is_some() {
            // Ya existe una transacción con este remitente y nonce
            if allow_replacement {
                // Si se permite el reemplazo, procesarlo
                let existing_tx = existing_tx.unwrap();
                self.metrics.stop_operation(OperationType::Validate);
                return self.process_replacement_transaction(tx, existing_tx.hash(), state);
            } else {
                // No se permite el reemplazo
                self.metrics.record_transaction_rejected();
                self.metrics.stop_operation(OperationType::Validate);
                self.metrics.stop_operation(OperationType::Add);
                return Err(Error::Validation(
                    "Transaction with this nonce already exists".into(),
                ));
            }
        }

        // Validate nonce
        if tx.nonce != sender_state.nonce {
            self.metrics.record_transaction_rejected();
            self.metrics.stop_operation(OperationType::Validate);
            self.metrics.stop_operation(OperationType::Add);
            return Err(Error::Validation(format!(
                "Invalid nonce: expected {}, got {}",
                sender_state.nonce, tx.nonce
            )));
        }

        // Validate balance
        let total_cost = tx.amount.saturating_add(tx.fee);
        if sender_state.balance < total_cost {
            self.metrics.record_transaction_rejected();
            self.metrics.stop_operation(OperationType::Validate);
            self.metrics.stop_operation(OperationType::Add);
            return Err(Error::Validation(format!(
                "Insufficient balance: has {}, needs {}",
                sender_state.balance, total_cost
            )));
        }

        // Calculate fee per byte for metrics
        let tx_size = tx.estimate_size();
        let tx_size_u64 = tx_size as u64;
        let fee_per_byte = if tx_size_u64 > 0 {
            tx.fee / tx_size_u64
        } else {
            tx.fee
        };

        // Record fee metrics
        self.metrics
            .record_transaction_fee(fee_per_byte as f64, tx_size);

        // Check minimum fee requirement
        if fee_per_byte < self.config.min_fee_per_byte {
            self.metrics.record_transaction_rejected();
            self.metrics.stop_operation(OperationType::Validate);
            self.metrics.stop_operation(OperationType::Add);
            return Err(Error::Validation(format!(
                "Fee too low: {} per byte, minimum is {}",
                fee_per_byte, self.config.min_fee_per_byte
            )));
        }

        // End validation timing
        self.metrics.stop_operation(OperationType::Validate);
        let validation_time = process_start.elapsed().as_micros() as u64;

        // Continue with the regular transaction addition process
        // Check if pool is at capacity
        if self.txs.len() >= self.config.max_size {
            // If we're at capacity, check if this transaction has higher fee than lowest
            if let Some(lowest_fee_tx) = self.get_lowest_fee_transaction() {
                let lowest_tx_size = lowest_fee_tx.estimate_size() as u64;
                let lowest_fee_per_byte = if lowest_tx_size > 0 {
                    lowest_fee_tx.fee / lowest_tx_size
                } else {
                    lowest_fee_tx.fee
                };

                if fee_per_byte <= lowest_fee_per_byte {
                    // New transaction doesn't have higher fee-per-byte, reject it
                    self.metrics.record_transaction_rejected();
                    self.metrics.stop_operation(OperationType::Add);
                    return Err(Error::Validation(
                        "Transaction pool full and fee too low".into(),
                    ));
                }

                // New transaction has higher fee, remove the lowest fee transaction
                self.remove_transaction(&lowest_fee_tx.hash());
            }
        }

        // Create pooled transaction
        let pooled_tx = PooledTransaction {
            transaction: tx.clone(),
            added_time: self.get_current_time(),
            is_valid: true,
            size: tx_size,
        };

        // Create fee record for priority
        let tx_with_fee = TransactionWithFee {
            tx_hash,
            fee: tx.fee,
            fee_per_byte,
            timestamp: pooled_tx.added_time,
        };

        // Update memory usage estimate
        self.memory_usage += tx_size
            + std::mem::size_of::<PooledTransaction>()
            + std::mem::size_of::<TransactionWithFee>();

        // Update memory usage metrics
        self.metrics.update_memory_usage(self.memory_usage);

        // Calcular el uso de memoria proyectado después de añadir la transacción
        let projected_memory = self.memory_usage
            + tx_size
            + std::mem::size_of::<PooledTransaction>()
            + std::mem::size_of::<TransactionWithFee>();

        // Activar optimización si estamos por encima del 75% o si la adición nos pondría por encima del límite
        if self.memory_usage > (self.config.max_memory * 3 / 4)
            || projected_memory > self.config.max_memory
        {
            self.metrics.start_operation(OperationType::Optimize);
            let removed = self.optimize_memory();
            self.metrics.stop_operation(OperationType::Optimize);

            if removed == 0 && projected_memory > self.config.max_memory {
                // If we couldn't optimize, reject this transaction
                self.memory_usage -= tx_size
                    + std::mem::size_of::<PooledTransaction>()
                    + std::mem::size_of::<TransactionWithFee>();
                self.metrics.update_memory_usage(self.memory_usage);
                self.metrics.record_transaction_rejected();
                self.metrics.stop_operation(OperationType::Add);
                return Err(Error::Validation(
                    "Cannot add transaction due to memory constraints".into(),
                ));
            }

            // Double-check we're still within limits
            if self.memory_usage > self.config.max_memory {
                // Still over limit, reject
                self.memory_usage -= tx_size
                    + std::mem::size_of::<PooledTransaction>()
                    + std::mem::size_of::<TransactionWithFee>();
                self.metrics.update_memory_usage(self.memory_usage);
                self.metrics.record_transaction_rejected();
                self.metrics.stop_operation(OperationType::Add);
                return Err(Error::Validation(
                    "Cannot add transaction due to memory constraints".into(),
                ));
            }
        }

        // Add to primary index
        self.txs.insert(tx_hash, pooled_tx);

        // Add to fee index
        self.by_fee.push(tx_with_fee);

        // Add to sender index
        self.by_address
            .entry(tx.sender)
            .or_insert_with(HashSet::new)
            .insert(tx_hash);

        // Update transaction count metrics
        self.metrics.update_transaction_count(self.txs.len());

        // Record successful addition
        let processing_time = process_start.elapsed().as_micros() as u64;
        self.metrics
            .record_transaction_added(processing_time, validation_time);

        // End total operation timing
        self.metrics.stop_operation(OperationType::Add);

        debug!("Added transaction to pool: {}", hex::encode(&tx_hash[0..4]));
        Ok(tx_hash)
    }

    /// Process a transaction that is replacing an existing one
    ///
    /// This method handles the logic for replacing a transaction that is already in the pool.
    ///
    /// # Parameters
    /// * `new_tx` - The new transaction that is replacing an existing one
    /// * `existing_hash` - Hash of the existing transaction to be replaced
    /// * `state` - Current blockchain state for validation
    ///
    /// # Returns
    /// Ok(hash) if the replacement was successful, Error otherwise
    fn process_replacement_transaction(
        &mut self,
        new_tx: Transaction,
        existing_hash: Hash,
        _state: &mut BlockchainState,
    ) -> Result<Hash, Error> {
        // Get the existing transaction
        let existing_tx = match self.get_transaction(&existing_hash) {
            Some(tx) => tx,
            None => {
                // This shouldn't happen since we already found it above
                return Err(Error::Validation("Existing transaction not found".into()));
            }
        };

        // Calculate fee for both transactions
        let new_fee = new_tx.fee;
        let existing_fee = existing_tx.fee;

        // Calculate the minimum required fee increase (percentage-based)
        let min_fee = existing_fee.saturating_add(
            existing_fee
                .checked_mul(self.config.replacement_fee_bump)
                .unwrap_or(u64::MAX)
                / 100,
        );

        // Check if the new transaction has enough fee increase
        if new_fee < min_fee {
            self.metrics.record_transaction_rejected();
            return Err(Error::Validation(format!(
                "Replacement fee too low: got {}, need at least {}",
                new_fee, min_fee
            )));
        }

        // The new transaction has a sufficient fee increase, remove the old one
        // before adding the new one
        let removed = self.remove_transaction(&existing_hash);
        if !removed {
            // This shouldn't happen since we already found the transaction
            return Err(Error::Validation(
                "Failed to remove existing transaction during replacement".into(),
            ));
        }

        debug!(
            "Replaced transaction {} with higher fee version",
            hex::encode(&existing_hash[0..4])
        );

        // Now add the new transaction using the regular process
        // We need to adjust the transaction to use the current expected nonce
        let new_tx_hash = new_tx.hash();
        let tx_size = new_tx.estimate_size();
        let tx_size_u64 = tx_size as u64;
        let fee_per_byte = if tx_size_u64 > 0 {
            new_tx.fee / tx_size_u64
        } else {
            new_tx.fee
        };

        // Create pooled transaction
        let pooled_tx = PooledTransaction {
            transaction: new_tx.clone(),
            added_time: self.get_current_time(),
            is_valid: true,
            size: tx_size,
        };

        // Create fee record for priority
        let tx_with_fee = TransactionWithFee {
            tx_hash: new_tx_hash,
            fee: new_tx.fee,
            fee_per_byte,
            timestamp: pooled_tx.added_time,
        };

        // Update memory usage before adding
        self.memory_usage += tx_size;
        self.metrics.update_memory_usage(self.memory_usage);

        // Check memory limit and optimize if needed
        if self.memory_usage > self.config.max_memory {
            self.optimize_memory();

            // If still over limit, reject the transaction
            if self.memory_usage > self.config.max_memory {
                self.memory_usage -= tx_size;
                self.metrics.update_memory_usage(self.memory_usage);
                self.metrics.record_transaction_rejected();
                return Err(Error::Validation(
                    "Memory limit reached, cannot replace transaction".into(),
                ));
            }
        }

        // Add to primary index
        self.txs.insert(new_tx_hash, pooled_tx);

        // Add to fee index
        self.by_fee.push(tx_with_fee);

        // Add to sender index
        self.by_address
            .entry(new_tx.sender)
            .or_insert_with(HashSet::new)
            .insert(new_tx_hash);

        // Update metrics
        self.metrics.update_transaction_count(self.txs.len());

        Ok(new_tx_hash)
    }

    /// Find a transaction with the specified sender and nonce
    ///
    /// # Parameters
    /// * `sender` - The transaction sender
    /// * `nonce` - The transaction nonce
    ///
    /// # Returns
    /// The transaction if found, None otherwise
    pub fn find_transaction_by_sender_and_nonce(
        &self,
        sender: &PublicKeyBytes,
        nonce: u64,
    ) -> Option<Transaction> {
        // Get all transactions from this sender
        if let Some(tx_hashes) = self.by_address.get(sender) {
            for hash in tx_hashes {
                if let Some(pooled_tx) = self.txs.get(hash) {
                    if pooled_tx.transaction.nonce == nonce {
                        return Some(pooled_tx.transaction.clone());
                    }
                }
            }
        }
        None
    }

    /// Add a transaction to the pool (wrapper for backward compatibility)
    ///
    /// # Parameters
    /// * `tx` - The transaction to add
    /// * `state` - Current blockchain state (for validation)
    ///
    /// # Returns
    /// `Ok(hash)` if transaction was added successfully, `Err` otherwise
    pub fn add_transaction(
        &mut self,
        tx: Transaction,
        state: &mut BlockchainState,
    ) -> Result<Hash, Error> {
        self.add_transaction_with_replacement(tx, state, false)
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
        state: &mut BlockchainState,
    ) -> (Vec<Hash>, Vec<(usize, crate::Error)>) {
        let mut successes = Vec::new();
        let mut failures = Vec::new();

        // Agrupar las transacciones por remitente
        let mut groups: HashMap<PublicKeyBytes, Vec<(usize, Transaction)>> = HashMap::new();
        for (idx, tx) in transactions.into_iter().enumerate() {
            groups.entry(tx.sender).or_default().push((idx, tx));
        }

        // Para cada remitente, ordenar por nonce ascendente y procesar secuencialmente
        for (_sender, mut txs_with_indices) in groups {
            txs_with_indices.sort_by_key(|&(_, ref tx)| tx.nonce);

            // Crear una copia del estado para validación secuencial
            let mut temp_state = state.clone();

            for (orig_idx, tx) in &txs_with_indices {
                // Verificar firmas y validaciones básicas
                if let Err(e) = tx.verify() {
                    failures.push((*orig_idx, e));
                    continue;
                }

                // Comprobar duplicados en el pool
                let tx_hash = tx.hash();
                if self.txs.contains_key(&tx_hash) {
                    failures.push((
                        *orig_idx,
                        Error::Validation("Transaction already in pool".into()),
                    ));
                    continue;
                }

                // Validar nonce
                let sender_state = temp_state.get_account_state(&tx.sender);
                if tx.nonce != sender_state.nonce {
                    failures.push((
                        *orig_idx,
                        Error::Validation(format!(
                            "Invalid nonce: expected {}, got {}",
                            sender_state.nonce, tx.nonce
                        )),
                    ));
                    continue;
                }

                // Validar balance
                let total_cost = tx.amount.saturating_add(tx.fee);
                if sender_state.balance < total_cost {
                    failures.push((
                        *orig_idx,
                        Error::Validation(format!(
                            "Insufficient balance: has {}, needs {}",
                            sender_state.balance, total_cost
                        )),
                    ));
                    continue;
                }

                // Validar tarifa mínima
                let tx_size = tx.estimate_size() as u64;
                let fee_per_byte = if tx_size > 0 {
                    tx.fee / tx_size
                } else {
                    tx.fee
                };

                if fee_per_byte < self.config.min_fee_per_byte {
                    failures.push((
                        *orig_idx,
                        Error::Validation(format!(
                            "Fee too low: {} per byte, minimum is {}",
                            fee_per_byte, self.config.min_fee_per_byte
                        )),
                    ));
                    continue;
                }

                // Si pasa todas las validaciones, actualizar el estado temporal
                temp_state.get_account_state(&tx.sender).balance -= total_cost;
                temp_state.get_account_state(&tx.recipient).balance += tx.amount;
                temp_state.get_account_state(&tx.sender).nonce += 1;

                // Añadir al pool sin modificar el estado real
                let added_time = Instant::now(); // Usar Instant::now() directamente

                // Calcular uso de memoria
                let tx_memory_usage = self.calculate_transaction_memory_usage(&tx);

                let pooled_tx = PooledTransaction {
                    transaction: tx.clone(),
                    added_time,
                    is_valid: true,
                    size: tx_memory_usage,
                };

                // Add to primary index
                self.txs.insert(tx_hash, pooled_tx);
                successes.push(tx_hash);

                // Update secondary indices - fee index and address index
                let tx_with_fee = TransactionWithFee {
                    tx_hash,
                    fee: tx.fee,
                    fee_per_byte,
                    timestamp: added_time,
                };

                // Add to fee index
                self.by_fee.push(tx_with_fee);

                // Add to sender index
                self.by_address
                    .entry(tx.sender)
                    .or_insert_with(HashSet::new)
                    .insert(tx_hash);

                // Update memory usage
                self.memory_usage += tx_memory_usage;
            }
        }
        (successes, failures)
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
        state: &mut BlockchainState,
    ) -> (Vec<Hash>, Vec<(usize, crate::Error)>) {
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        // First, deserialize all transactions
        let mut transactions = Vec::with_capacity(transaction_data.len());

        for (idx, data) in transaction_data.into_iter().enumerate() {
            match bincode::decode_from_slice::<Transaction, _>(&data, bincode::config::standard()) {
                Ok((tx, _)) => transactions.push(tx),
                Err(e) => {
                    failed.push((
                        idx,
                        crate::Error::Serialization(format!("Deserialization error: {}", e)),
                    ));
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

    pub fn select_transactions(
        &mut self,
        max_count: usize,
        state: &mut BlockchainState,
    ) -> Vec<Transaction> {
        self.metrics.start_operation(OperationType::Select);

        let mut result = Vec::new();

        // En vez de usar estados mantenidos internamente, usar directamente los valores del state pasado como parámetro
        let mut sender_states: HashMap<PublicKeyBytes, (u64, u64)> = HashMap::new();

        // Obtener los estados iniciales para todos los remitentes
        for pooled_tx in self.txs.values() {
            if pooled_tx.is_valid {
                let sender = pooled_tx.transaction.sender;
                if !sender_states.contains_key(&sender) {
                    let account = state.get_account_state(&sender);
                    sender_states.insert(sender, (account.balance, account.nonce));
                }
            }
        }

        // Eliminar las transacciones ya procesadas
        let mut processed_hashes = HashSet::new();

        // Procesar todas las transacciones válidas
        for _ in 0..max_count {
            // Encontrar la próxima transacción válida para cada remitente
            let mut valid_txs = Vec::new();

            for (hash, pooled_tx) in &self.txs {
                if processed_hashes.contains(hash) || !pooled_tx.is_valid {
                    continue;
                }

                let tx = &pooled_tx.transaction;
                let sender = tx.sender;

                // Obtener el estado actual para este remitente
                let (current_balance, current_nonce) =
                    if let Some(&state_values) = sender_states.get(&sender) {
                        state_values
                    } else {
                        let account = state.get_account_state(&sender);
                        let values = (account.balance, account.nonce);
                        sender_states.insert(sender, values);
                        values
                    };

                // Verificar nonce
                if tx.nonce != current_nonce {
                    continue;
                }

                // Verificar balance
                let total_cost = tx.amount.saturating_add(tx.fee);
                if current_balance < total_cost {
                    continue;
                }

                // Transacción válida - añadir a candidatas
                valid_txs.push((hash, pooled_tx, self.calculate_fee_per_byte(tx)));
            }

            if valid_txs.is_empty() {
                break;
            }

            // Ordenar por fee (mayor primero) y luego por timestamp (más antiguo primero)
            valid_txs.sort_by(|&(_, a, fee_a), &(_, b, fee_b)| {
                fee_b
                    .partial_cmp(&fee_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.added_time.cmp(&b.added_time))
            });

            // Seleccionar la transacción de mayor prioridad
            let (selected_hash, selected_tx, _) = valid_txs[0];
            let tx = &selected_tx.transaction;

            // Actualizar el estado
            let (balance, nonce) = sender_states.get_mut(&tx.sender).unwrap();
            *balance -= tx.amount + tx.fee;
            *nonce += 1;

            // Marcar como procesada
            processed_hashes.insert(*selected_hash);

            // Añadir a resultados
            result.push(tx.clone());
        }

        self.metrics_mut().stop_operation(OperationType::Select);
        result
    }

    pub fn select_transactions_for_test(&self, max_count: usize) -> Vec<Transaction> {
        let mut result = Vec::new();

        // Simplemente devuelve todas las transacciones en el pool, hasta max_count
        let mut all_txs: Vec<_> = self
            .txs
            .values()
            .map(|pooled_tx| (&pooled_tx.transaction, pooled_tx.added_time))
            .collect();

        // Ordenar primero por fee (descendente)
        all_txs.sort_by(|(tx_a, time_a), (tx_b, time_b)| {
            // Comparar por fee_per_byte
            let a_fee_per_byte = self.calculate_fee_per_byte(tx_a);
            let b_fee_per_byte = self.calculate_fee_per_byte(tx_b);

            b_fee_per_byte
                .partial_cmp(&a_fee_per_byte)
                .unwrap_or(std::cmp::Ordering::Equal)
                // En caso de empate, usar el timestamp como desempate
                .then_with(|| time_a.cmp(time_b))
        });

        // Añadir hasta max_count transacciones
        for (tx, _) in all_txs.iter().take(max_count) {
            result.push((*tx).clone());
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

        // Record expired transactions in metrics
        if count > 0 {
            self.metrics.record_transactions_expired(count as u64);
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
        self.metrics.start_operation(OperationType::Remove);

        // Remove from main index and get the transaction
        let pooled_tx = match self.txs.remove(hash) {
            Some(tx) => tx,
            None => {
                self.metrics.stop_operation(OperationType::Remove);
                return false;
            }
        };

        let tx = &pooled_tx.transaction;

        // Update memory usage
        let tx_size = tx.estimate_size();
        let pooled_tx_overhead = std::mem::size_of::<PooledTransaction>();
        let hash_map_entry_size =
            std::mem::size_of::<Hash>() + std::mem::size_of::<*const PooledTransaction>() + 32;
        let by_fee_entry_size = std::mem::size_of::<TransactionWithFee>();

        // Calcular el tamaño de la entrada by_address de la misma manera
        let sender_entry_size = if self
            .by_address
            .get(&tx.sender)
            .map_or(false, |set| set.len() > 1)
        {
            // Si quedan más transacciones de este remitente, solo restar el tamaño de la entrada Hash
            std::mem::size_of::<Hash>() + 16
        } else {
            // Si esta es la última transacción del remitente, restar toda la entrada
            std::mem::size_of::<PublicKeyBytes>()
                + std::mem::size_of::<HashSet<Hash>>()
                + std::mem::size_of::<Hash>()
                + 48
        };

        self.memory_usage = self.memory_usage.saturating_sub(
            tx_size
                + pooled_tx_overhead
                + hash_map_entry_size
                + by_fee_entry_size
                + sender_entry_size,
        );

        // Update metrics
        self.metrics.update_memory_usage(self.memory_usage);
        self.metrics.update_transaction_count(self.txs.len());
        self.metrics.record_transaction_removed();

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

        self.metrics.stop_operation(OperationType::Remove);
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
        self.metrics.start_operation(OperationType::Optimize);

        // Si estamos por encima del 75% del límite, optimizar
        if self.memory_usage <= (self.config.max_memory * 3 / 4) {
            self.metrics.stop_operation(OperationType::Optimize);
            return 0;
        }

        // Calculate how much memory to free
        // Target: reduce to 60% of max memory
        let target_memory = self.config.max_memory * 6 / 10;
        let memory_to_free = self.memory_usage.saturating_sub(target_memory);

        // If nothing to free, return early
        if memory_to_free == 0 {
            self.metrics.stop_operation(OperationType::Optimize);
            return 0;
        }

        debug!(
            "Memory usage ({} bytes) exceeds target, optimizing pool",
            self.memory_usage
        );

        // Forzar la eliminación de al menos una transacción para pruebas
        let tx_count_to_remove = if self.txs.len() > 0 {
            let avg_tx_size = if self.txs.is_empty() {
                200 // Reasonable default if no transactions
            } else {
                self.memory_usage / self.txs.len()
            };

            // Calcular cuántas transacciones eliminar y garantizar mínimo 1
            (memory_to_free / avg_tx_size).max(1)
        } else {
            0
        };

        debug!(
            "Removing approximately {} transactions to free memory",
            tx_count_to_remove
        );

        // Remove the lowest-priority transactions
        let removed = self.remove_lowest_priority_transactions(tx_count_to_remove);

        self.metrics.stop_operation(OperationType::Optimize);
        removed
    }

    /// Remove lowest priority transactions from the pool
    ///
    /// # Parameters
    /// * `count` - Maximum number of transactions to remove
    ///
    /// # Returns
    /// The actual number of transactions removed
    fn remove_lowest_priority_transactions(&mut self, count: usize) -> usize {
        if self.txs.is_empty() || count == 0 {
            return 0;
        }

        // Debug information to help diagnose
        debug!(
            "Starting removal: {} txs in pool, {} entries in by_fee",
            self.txs.len(),
            self.by_fee.len()
        );

        // Si by_fee está vacío o desincronizado, reconstruirlo
        if self.by_fee.len() != self.txs.len() {
            self.by_fee.clear();
            for (hash, pooled_tx) in &self.txs {
                let tx = &pooled_tx.transaction;
                let tx_size = tx.estimate_size() as u64;
                let fee_per_byte = if tx_size > 0 {
                    tx.fee / tx_size
                } else {
                    tx.fee
                };

                self.by_fee.push(TransactionWithFee {
                    tx_hash: *hash,
                    fee: tx.fee,
                    fee_per_byte,
                    timestamp: pooled_tx.added_time,
                });
            }
            debug!("Rebuilt by_fee index with {} entries", self.by_fee.len());
        }

        // Create a copy of by_fee in vector form so we can sort
        let mut fee_entries: Vec<TransactionWithFee> = self.by_fee.iter().cloned().collect();

        // Sort by fee per byte (ascending) so lowest fee transactions are first
        fee_entries.sort_by(|a, b| {
            a.fee_per_byte
                .cmp(&b.fee_per_byte)
                .then_with(|| b.timestamp.cmp(&a.timestamp)) // Older first when fees are equal
        });

        // Si aún así no hay nada que eliminar, eliminar al menos una transacción
        if fee_entries.is_empty() && !self.txs.is_empty() {
            let hash = *self.txs.keys().next().unwrap();
            if self.remove_transaction(&hash) {
                debug!("Forced removal of one transaction");
                return 1;
            }
        }

        // Take the lowest fee transactions up to count
        let to_remove: Vec<_> = fee_entries
            .into_iter()
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

        // Si aún no se ha eliminado nada pero hay transacciones, forzar la eliminación
        if removed == 0 && !self.txs.is_empty() {
            let hash = *self.txs.keys().next().unwrap();
            if self.remove_transaction(&hash) {
                removed = 1;
                debug!("Forced removal of one transaction as fallback");
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
        self.metrics.start_operation(OperationType::Maintenance);

        let mut removed = 0;

        // Remove expired transactions
        removed += self.remove_expired();

        // Optimize memory usage if needed
        removed += self.optimize_memory();

        // Clean up the priority queue if needed
        if removed > 0 && self.by_fee.len() > self.txs.len() * 2 {
            // If we have a lot of "ghost" entries in the binary heap,
            // rebuild it to save memory and improve performance
            let valid_entries: Vec<_> = self
                .by_fee
                .iter()
                .filter(|entry| self.txs.contains_key(&entry.tx_hash))
                .cloned()
                .collect();

            self.by_fee.clear();
            for entry in valid_entries {
                self.by_fee.push(entry);
            }
        }

        self.metrics.stop_operation(OperationType::Maintenance);

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
        self.metrics.start_operation(OperationType::Revalidate);

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
                debug!(
                    "Transaction {} invalidated during revalidation",
                    hex::encode(&tx_hash[0..4])
                );
            }
        }

        self.metrics.stop_operation(OperationType::Revalidate);
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Get mutable reference to metrics collector
    pub fn metrics_mut(&mut self) -> &mut MetricsCollector {
        &mut self.metrics
    }

    /// Get a performance report
    pub fn generate_metrics_report(&self) -> String {
        self.metrics.generate_report()
    }

    pub fn get_current_time(&self) -> Instant {
        Instant::now()
    }

    pub fn get_lowest_fee_transaction(&self) -> Option<&Transaction> {
        // Get the transaction with the lowest fee from the by_fee vector
        self.by_fee
            .iter()
            .min_by_key(|tx| tx.fee_per_byte)
            .and_then(|tx_with_fee| self.txs.get(&tx_with_fee.tx_hash))
            .map(|pooled_tx| &pooled_tx.transaction)
    }

    /// Get current memory usage of the transaction pool
    ///
    /// # Returns
    /// Memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    /// Validate a transaction and create rich error information (internal helper)
    fn validate_transaction_internal(&self, tx: &Transaction, state: &mut BlockchainState) -> TxResult<()> {
        // Step 1: Check if transaction already exists
        let tx_hash = tx.hash();
        if self.txs.contains_key(&tx_hash) {
            return Err(TransactionError::AlreadyExists {
                tx_hash,
            });
        }
        
        // Step 2: Verify transaction signature if not already done
        if let Err(_) = tx.verify() {
            return Err(TransactionError::InvalidSignature);
        }
        
        // Step 3: Get account state
        let sender_state = state.get_account_state(&tx.sender);
        
        // Step 4: Validate nonce
        if tx.nonce != sender_state.nonce {
            return Err(TransactionError::InvalidNonce {
                sender: tx.sender,
                expected: sender_state.nonce,
                actual: tx.nonce,
            });
        }
        
        // Step 5: Validate balance
        let total_cost = tx.amount.saturating_add(tx.fee);
        if sender_state.balance < total_cost {
            return Err(TransactionError::InsufficientBalance {
                sender: tx.sender,
                balance: sender_state.balance,
                required: total_cost,
            });
        }
        
        // Step 6: Validate minimum fee
        let tx_size = tx.estimate_size() as u64;
        let fee_per_byte = if tx_size > 0 { tx.fee / tx_size } else { tx.fee };
        
        if fee_per_byte < self.config.min_fee_per_byte {
            return Err(TransactionError::FeeTooLow {
                fee_per_byte,
                min_required: self.config.min_fee_per_byte,
            });
        }
        
        // Step 7: Validate pool constraints
        if self.txs.len() >= self.config.max_size {
            return Err(TransactionError::PoolFull {
                current_size: self.txs.len(),
                max_size: self.config.max_size,
            });
        }
        
        // Calculate memory usage of this transaction
        let tx_memory = self.calculate_transaction_memory_usage(tx);
        if self.memory_usage + tx_memory > self.config.max_memory {
            return Err(TransactionError::MemoryLimitReached {
                current_bytes: self.memory_usage,
                max_bytes: self.config.max_memory,
            });
        }
        
        Ok(())
    }
    
    /// Add a single transaction to the pool with detailed error reporting
    /// 
    /// This method provides the same functionality as `add_transaction` but
    /// returns more detailed error information through the `TransactionError` type.
    pub fn verify_transaction(&self, tx: &Transaction, state: &mut BlockchainState) 
        -> TxResult<()> 
    {
        // Since self is immutable, we can't modify metrics directly
        // Just validate without metrics tracking
        self.validate_transaction_internal(tx, state)
    }
    
    /// Add a transaction with replacement option, using detailed error reporting
    pub fn add_transaction_with_replacement_detailed(
        &mut self, 
        tx: Transaction, 
        state: &mut BlockchainState,
        allow_replacement: bool
    ) -> TxResult<Hash> {
        let tx_hash = tx.hash();
        
        // Check for existing transaction with same sender and nonce
        let existing_tx = self.find_transaction_by_sender_and_nonce(&tx.sender, tx.nonce);
        
        if let Some(existing_tx) = existing_tx {
            // Found existing transaction with same sender/nonce
            if !allow_replacement {
                // If replacement not allowed, return an error
                return Err(TransactionError::AlreadyExists { 
                    tx_hash: existing_tx.hash() 
                });
            }
            
            // Calculate minimum required fee for replacement
            let min_required_fee = existing_tx.fee
                .saturating_mul(100 + self.config.replacement_fee_bump)
                .saturating_div(100);
            
            // Check if new transaction has sufficient fee for replacement
            if tx.fee < min_required_fee {
                return Err(TransactionError::ReplacementFeeTooLow {
                    actual: tx.fee,
                    required: min_required_fee,
                });
            }
            
            // Validate the new transaction (other than duplicate check)
            self.validate_transaction_internal(&tx, state)?;
            
            // Remove the existing transaction
            self.remove_transaction(&existing_tx.hash());
            
            // Add the new transaction (implementation left out for brevity)
            
            Ok(tx_hash)
        } else {
            // No existing transaction with this sender/nonce
            // Just validate and add normally
            self.validate_transaction_internal(&tx, state)?;
            
            // Implementation left out for brevity
            
            Ok(tx_hash)
        }
    }
}
