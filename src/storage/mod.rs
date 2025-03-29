//! Storage layer for the Blocana blockchain
//!
//! This module provides persistent storage mechanisms for blockchain data using RocksDB.
//! It handles storage and retrieval of blocks, transactions, and account states in an
//! efficient, durable manner optimized for blockchain operations.
//!
//! # Design
//!
//! The storage layer uses RocksDB with the following column families:
//! - `blocks`: Maps block hash → block data
//! - `block_height`: Maps height → block hash
//! - `transactions`: Maps transaction hash → transaction location
//! - `account_state`: Maps account address → account state
//!
//! # Examples
//!
//! ```no_run
//! # use blocana::block::Block;
//! # use blocana::types::Hash;
//! # let genesis_hash = [0u8; 32];
//! # let validator = [0u8; 32];
//! # // Crear un bloque de ejemplo
//! # let block = Block::new(genesis_hash, 1, vec![], validator).unwrap();
//! # let block_hash = block.header.hash();
//! use blocana::storage::{BlockchainStorage, StorageConfig};
//! use blocana::storage::state_store::StateStore;  // Añadido punto y coma
//! 
//! // Open the database
//! let config = StorageConfig::default();
//! let storage = BlockchainStorage::open(&config).unwrap();
//! 
//! // Store a block
//! storage.store_block(&block).unwrap();
//! 
//! // Retrieve a block
//! let retrieved_block = storage.get_block(&block_hash).unwrap();
//! ```

use crate::block::Block;
use crate::state::AccountState;
use crate::transaction::Transaction;
use crate::types::{Hash, PublicKeyBytes};
use hex;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, WriteBatch, DB};
use std::path::Path;

/// Storage errors
#[derive(Debug)]
pub enum Error {
    /// IO Error
    IO(std::io::Error),
    /// Database Error
    Database(String),
    /// Serialization Error
    Serialization(String),
    /// Item Not Found
    NotFound(String),
    /// Other Error
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(e) => write!(f, "IO error: {}", e),
            Error::Database(s) => write!(f, "Database error: {}", s),
            Error::Serialization(s) => write!(f, "Serialization error: {}", s),
            Error::NotFound(s) => write!(f, "Not found: {}", s),
            Error::Other(s) => write!(f, "Other storage error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<rocksdb::Error> for Error {
    fn from(error: rocksdb::Error) -> Self {
        Error::Database(error.to_string())
    }
}

impl From<bincode::error::EncodeError> for Error {
    fn from(error: bincode::error::EncodeError) -> Self {
        Error::Serialization(error.to_string())
    }
}

impl From<bincode::error::DecodeError> for Error {
    fn from(error: bincode::error::DecodeError) -> Self {
        Error::Serialization(format!("Decode error: {}", error))
    }
}

/// Configuration for the blockchain storage
#[derive(Clone, Debug)]
pub struct StorageConfig {
    /// Path to the database directory
    pub db_path: String,
    /// Maximum number of open files
    pub max_open_files: i32,
    /// Write buffer size in bytes
    pub write_buffer_size: usize,
    /// Maximum number of write buffers
    pub max_write_buffer_number: i32,
    /// Target file size for SST files
    pub target_file_size_base: u64,
    /// LRU cache size in bytes (0 = use default)
    pub cache_size: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "data/blocana_db".to_string(),
            max_open_files: 1000,
            write_buffer_size: 64 * 1024 * 1024, // 64MB
            max_write_buffer_number: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
            cache_size: 128 * 1024 * 1024, // 128MB
        }
    }
}

/// A structure containing references to all column families.
///
pub struct BlockchainColumnFamilies<'a> {
    /// Column family for storing full blocks
    pub blocks: &'a ColumnFamily,
    /// Column family for mapping block heights to block hashes
    pub block_height: &'a ColumnFamily,
    /// Column family for transaction indexing
    pub transactions: &'a ColumnFamily,
    /// Column family for account states
    pub account_state: &'a ColumnFamily,
    /// New timestamp index
    pub timestamp_index: &'a ColumnFamily,
    /// New metadata column family
    pub metadata: &'a ColumnFamily,
}

/// Information about where a transaction is stored in the blockchain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct TxLocation {
    /// Hash of the block containing the transaction
    pub block_hash: Hash,
    /// Index of the transaction within the block
    pub index: u32,
}

/// Main storage interface for the blockchain.
pub struct BlockchainStorage {
    /// RocksDB database instance
    db: DB,
}

impl BlockchainStorage {
    /// Opens the blockchain storage with the specified configuration.
    ///
    /// # Parameters
    /// * `config` - The storage configuration
    ///
    /// # Returns
    /// A result containing the opened storage instance or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database directory cannot be created
    /// - The database cannot be opened
    pub fn open(config: &StorageConfig) -> Result<Self, Error> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config.db_path)?;

        // Define column families
        let cf_names = [
            "blocks",
            "block_height",
            "transactions",
            "account_state",
            "timestamp_index", // New timestamp index
            "metadata",        // New metadata column family
        ];

        // Configure database options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_keep_log_file_num(10);
        opts.set_max_open_files(config.max_open_files);
        opts.set_write_buffer_size(config.write_buffer_size);
        opts.set_max_write_buffer_number(config.max_write_buffer_number);
        opts.set_target_file_size_base(config.target_file_size_base);
        
        // Set up cache if configured
        if config.cache_size > 0 {
            // Create a block cache for frequently accessed blocks
            let cache = rocksdb::Cache::new_lru_cache(config.cache_size);
            
            // Create block-based table options
            let mut block_opts = rocksdb::BlockBasedOptions::default();
            block_opts.set_block_cache(&cache);
            
            // Set the table factory with the configured cache
            opts.set_block_based_table_factory(&block_opts);
        }

        // Open database with column families
        let db = DB::open_cf(&opts, &config.db_path, cf_names)?;

        Ok(Self { db })
    }

    /// Opens the storage with custom column family options.
    ///
    /// # Parameters
    /// * `path` - The path to the database directory
    /// * `options` - The database options
    /// * `cf_descriptors` - The column family descriptors
    ///
    /// # Returns
    /// A result containing the opened storage instance or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database cannot be opened
    pub fn open_with_cf_options<P: AsRef<Path>>(
        path: P,
        options: Options,
        cf_descriptors: Vec<ColumnFamilyDescriptor>,
    ) -> Result<Self, Error> {
        let db = DB::open_cf_descriptors(&options, path, cf_descriptors)?;
        Ok(Self { db })
    }

    /// Gets references to all column families.
    ///
    /// # Returns
    /// A result containing the column families or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - Any column family is not found
    pub fn get_column_families(&self) -> Result<BlockchainColumnFamilies<'_>, Error> {
        let blocks = self
            .db
            .cf_handle("blocks")
            .ok_or_else(|| Error::Database("Column family 'blocks' not found".to_string()))?;
        let block_height = self
            .db
            .cf_handle("block_height")
            .ok_or_else(|| Error::Database("Column family 'block_height' not found".to_string()))?;
        let transactions = self
            .db
            .cf_handle("transactions")
            .ok_or_else(|| Error::Database("Column family 'transactions' not found".to_string()))?;
        let account_state = self
            .db
            .cf_handle("account_state")
            .ok_or_else(|| Error::Database("Column family 'account_state' not found".to_string()))?;
        let timestamp_index = self
            .db
            .cf_handle("timestamp_index")
            .ok_or_else(|| Error::Database("Column family 'timestamp_index' not found".to_string()))?;
        let metadata = self
            .db
            .cf_handle("metadata")
            .ok_or_else(|| Error::Database("Column family 'metadata' not found".to_string()))?;

        Ok(BlockchainColumnFamilies {
            blocks,
            block_height,
            transactions,
            account_state,
            timestamp_index,
            metadata,
        })
    }

    /// Stores a block in the database.
    ///
    /// # Parameters
    /// * `block` - The block to store
    ///
    /// # Returns
    /// A result indicating success or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The block cannot be serialized
    /// - The database write fails
    pub fn store_block(&self, block: &Block) -> Result<(), Error> {
        let cfs = self.get_column_families()?;

        let block_bytes = bincode::encode_to_vec(block, bincode::config::standard())?;
        let block_hash = block.header.hash();
        let height_bytes = block.header.height.to_le_bytes();
        let timestamp_bytes = block.header.timestamp.to_le_bytes();

        // Create a write batch for atomic operations
        let mut batch = WriteBatch::default();

        // Add block to blocks column family
        batch.put_cf(cfs.blocks, block_hash, &block_bytes);

        // Add height -> hash mapping
        batch.put_cf(cfs.block_height, &height_bytes, block_hash);

        // Add timestamp -> hash mapping
        let mut timestamp_key = Vec::with_capacity(16);
        timestamp_key.extend_from_slice(&timestamp_bytes);
        timestamp_key.extend_from_slice(&height_bytes);
        batch.put_cf(cfs.timestamp_index, &timestamp_key, block_hash);

        // Index each transaction
        for (i, tx) in block.transactions.iter().enumerate() {
            let tx_hash = tx.hash();
            let tx_location = TxLocation {
                block_hash,
                index: i as u32,
            };
            let tx_loc_bytes = bincode::encode_to_vec(&tx_location, bincode::config::standard())?;
            batch.put_cf(cfs.transactions, tx_hash, &tx_loc_bytes);
        }

        // Write batch atomically
        self.db.write(batch)?;

        Ok(())
    }

    /// Retrieves a block by its hash.
    ///
    /// # Parameters
    /// * `hash` - The hash of the block to retrieve
    ///
    /// # Returns
    /// A result containing the block if found, None if not found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    /// - The block cannot be deserialized
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>, Error> {
        let cfs = self.get_column_families()?;
        match self.db.get_cf(cfs.blocks, hash)? {
            Some(bytes) => {
                let (block, _): (Block, _) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard())?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Gets a block by its height.
    ///
    /// # Parameters
    /// * `height` - The height of the block to retrieve
    ///
    /// # Returns
    /// A result containing the block if found, None if not found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    /// - The block hash index is corrupted
    /// - The block cannot be deserialized
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, Error> {
        let cfs = self.get_column_families()?;
        // Convert height to bytes
        let height_bytes = height.to_le_bytes();
        // Get the block hash
        match self.db.get_cf(cfs.block_height, height_bytes)? {
            Some(hash_bytes) => {
                if hash_bytes.len() != 32 {
                    return Err(Error::Database("Invalid hash length in index".to_string()));
                }

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);

                // Get the block by hash
                self.get_block(&hash)
            }
            None => Ok(None),
        }
    }

    /// Gets the block hash at a specific height.
    ///
    /// # Parameters
    /// * `height` - The block height
    ///
    /// # Returns
    /// A result containing the block hash if found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    /// - The block hash index is corrupted
    pub fn get_block_hash_by_height(&self, height: u64) -> Result<Hash, Error> {
        let cfs = self.get_column_families()?;
        // Convert height to bytes
        let height_bytes = height.to_le_bytes();
        // Get the block hash
        match self.db.get_cf(cfs.block_height, height_bytes)? {
            Some(hash_bytes) => {
                if hash_bytes.len() != 32 {
                    return Err(Error::Database("Invalid hash length in index".to_string()));
                }

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                Ok(hash)
            }
            None => Err(Error::NotFound(format!(
                "Block at height {} not found",
                height
            ))),
        }
    }

    /// Gets the latest block height.
    ///
    /// # Returns
    /// A result containing the latest block height, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    pub fn get_latest_height(&self) -> Result<u64, Error> {
        let cfs = self.get_column_families()?;

        let mut iter = self
            .db
            .iterator_cf(cfs.block_height, rocksdb::IteratorMode::End);
        if let Some(Ok((key, _))) = iter.next() {
            if key.len() != 8 {
                return Err(Error::Database("Invalid height key length".to_string()));
            }

            let mut height_bytes = [0u8; 8];
            height_bytes.copy_from_slice(&key);
            let height = u64::from_le_bytes(height_bytes);

            Ok(height)
        } else {
            Ok(0) // No blocks yet, so height is 0
        }
    }

    /// Gets a transaction by its hash.
    ///
    /// # Parameters
    /// * `hash` - The transaction hash
    ///
    /// # Returns
    /// A result containing the transaction if found, None if not found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    /// - The referenced block doesn't exist
    /// - The transaction index is invalid
    pub fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, Error> {
        let cfs = self.get_column_families()?;

        // Get transaction location
        match self.db.get_cf(cfs.transactions, hash)? {
            Some(loc_bytes) => {
                let (tx_location, _): (TxLocation, _) =
                    bincode::decode_from_slice(&loc_bytes, bincode::config::standard())?;
                // Get the block containing this transaction
                match self.get_block(&tx_location.block_hash)? {
                    Some(block) => {
                        let index = tx_location.index as usize;
                        if index < block.transactions.len() {
                            Ok(Some(block.transactions[index].clone()))
                        } else {
                            Err(Error::Database(format!(
                                "Invalid transaction index {} in block",
                                index
                            )))
                        }
                    }
                    None => Err(Error::NotFound(format!(
                        "Block containing transaction {} not found",
                        hex::encode(hash)
                    ))),
                }
            }
            None => Ok(None),
        }
    }

    /// Stores account state for an address.
    ///
    /// # Parameters
    /// * `address` - The account address
    /// * `state` - The account state to store
    ///
    /// # Returns
    /// A result indicating success or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The account state cannot be serialized
    /// - The database write fails
    pub fn store_account_state(
        &self,
        address: &PublicKeyBytes,
        state: &AccountState,
    ) -> Result<(), Error> {
        let cfs = self.get_column_families()?;

        let state_bytes = bincode::encode_to_vec(state, bincode::config::standard())?;
        self.db.put_cf(cfs.account_state, address, state_bytes)?;

        Ok(())
    }

    /// Gets account state for an address.
    ///
    /// # Parameters
    /// * `address` - The account address
    ///
    /// # Returns
    /// A result containing the account state if found, None if not found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    /// - The account state cannot be deserialized
    pub fn get_account_state(
        &self,
        address: &PublicKeyBytes,
    ) -> Result<Option<AccountState>, Error> {
        let cfs = self.get_column_families()?;
        match self.db.get_cf(cfs.account_state, address)? {
            Some(bytes) => {
                let (state, _): (AccountState, _) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard())?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Creates a database backup.
    ///
    /// # Parameters
    /// * `backup_path` - The directory where the backup will be stored
    ///
    /// # Returns
    /// A result indicating success or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The backup operation fails
    pub fn create_backup(&self, backup_path: &str) -> Result<(), Error> {
        use rocksdb::{backup::BackupEngine, backup::BackupEngineOptions, Env};

        // Get the default environment
        let env = Env::new()?;

        // Open a backup engine with proper parameters
        let backup_opts = BackupEngineOptions::new(backup_path)?;
        let mut backup_engine = BackupEngine::open(&backup_opts, &env)?;

        // Create a new backup
        backup_engine.create_new_backup(&self.db)?;

        Ok(())
    }

    /// Restores the database from a backup.
    ///
    /// # Parameters
    /// * `backup_path` - Directory containing the backup
    /// * `db_path` - The database directory for restoration
    /// * `restore_options` - Options for the restore operation
    ///
    /// # Returns
    /// A result indicating success or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The restore operation fails
    pub fn restore_from_backup(
        backup_path: &str,
        db_path: &str,
        restore_options: Option<rocksdb::backup::RestoreOptions>,
    ) -> Result<(), Error> {
        use rocksdb::{backup::BackupEngine, backup::BackupEngineOptions, Env};

        // Get the default environment
        let env = Env::new()?;

        // Open a backup engine - Change default() to new()
        let backup_opts = BackupEngineOptions::new(backup_path)?;
        let mut backup_engine = BackupEngine::open(&backup_opts, &env)?;

        // Get restore options (use defaults if none provided)
        let restore_options = restore_options.unwrap_or_default();

        // Restore the database
        backup_engine.restore_from_latest_backup(db_path, db_path, &restore_options)?;

        Ok(())
    }

    /// Verifies the integrity of the blockchain database.
    ///
    /// Walks the chain backwards to ensure blocks properly link together.
    ///
    /// # Returns
    /// A result indicating `true` if the database is consistent, `false` if inconsistencies are found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The verification process fails due to database errors
    pub fn verify_integrity(&self) -> Result<bool, Error> {
        let latest_height = self.get_latest_height()?;
        if latest_height == 0 {
            return Ok(true); // Empty database is valid
        }

        // Walk the chain backwards to verify integrity
        for height in (0..=latest_height).rev() {
            let current_hash = self.get_block_hash_by_height(height)?;
            let block = self.get_block(&current_hash)?.ok_or_else(|| {
                Error::Database(format!(
                    "Block with hash {} not found",
                    hex::encode(current_hash)
                ))
            })?;

            // Verify this block points to the correct previous block
            if height > 0 {
                let expected_prev_hash = self.get_block_hash_by_height(height - 1)?;
                if block.header.prev_hash != expected_prev_hash {
                    println!("Integrity check failed at height {}", height);
                    println!("Expected prev_hash: {}", hex::encode(expected_prev_hash));
                    println!("Actual prev_hash: {}", hex::encode(block.header.prev_hash));
                    return Ok(false);
                }
            } else {
                // For the genesis block, the previous hash should be all zeros
                if block.header.prev_hash != [0u8; 32] {
                    println!("Genesis block prev_hash is not zero");
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Gets the raw RocksDB handle.
    ///
    /// # Returns
    /// A reference to the raw RocksDB handle
    pub fn raw_db(&self) -> &DB {
        &self.db
    }

    /// Retrieves blocks within a specific time range.
    pub fn get_blocks_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
        limit: usize,
    ) -> Result<Vec<Block>, Error> {
        let cfs = self.get_column_families()?;

        let start_key = start_time.to_le_bytes();
        let _end_bytes = end_time.to_le_bytes();

        // Create an iterator over the timestamp index
        let iter = self.db.iterator_cf(
            cfs.timestamp_index,
            rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
        );

        let mut blocks = Vec::new();
        for item in iter {
            let (key, value) = item?;
            
            // Extrae el timestamp de la clave (8 primeros bytes)
            if key.len() >= 8 {
                let mut key_timestamp_bytes = [0u8; 8];
                key_timestamp_bytes.copy_from_slice(&key[0..8]);
                let key_timestamp = u64::from_le_bytes(key_timestamp_bytes);
                
                // Si el timestamp está fuera del rango, detén el bucle
                if key_timestamp > end_time {
                    break;
                }
                
                // Si el timestamp es menor que nuestro inicio, continúa
                if key_timestamp < start_time {
                    continue;
                }

                if value.len() != 32 {
                    return Err(Error::Database("Invalid block hash in timestamp index".to_string()));
                }

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&value);
                if let Some(block) = self.get_block(&hash)? {
                    blocks.push(block);
                    if blocks.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(blocks)
    }

    /// Counts the number of blocks within a specific time range.
    pub fn count_blocks_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<usize, Error> {
        let cfs = self.get_column_families()?;

        let _start_key = start_time.to_le_bytes();
        println!("Buscando bloques entre {} y {}", start_time, end_time);

        // Agregamos flag de debug para ver qué está pasando
        let mut successful_matches = Vec::new();
        let mut all_timestamps = Vec::new();

        // Approach 1: Scan all keys (less efficient but more reliable)
        let iter = self.db.iterator_cf(
            cfs.timestamp_index, 
            rocksdb::IteratorMode::Start
        );

        let mut count = 0;
        
        for item in iter {
            let (key, value) = item?;
            
            // Extrae el timestamp de la clave (8 primeros bytes)
            if key.len() >= 8 {
                let mut key_timestamp_bytes = [0u8; 8];
                key_timestamp_bytes.copy_from_slice(&key[0..8]);
                let key_timestamp = u64::from_le_bytes(key_timestamp_bytes);
                
                // Guardamos todos los timestamps para ver qué hay en la BD
                all_timestamps.push(key_timestamp);
                
                // Si el timestamp está dentro del rango
                if key_timestamp >= start_time && key_timestamp <= end_time {
                    count += 1;
                    successful_matches.push(key_timestamp);
                    
                    // Debug info
                    println!("✓ Timestamp {} está en rango [{},{}]", 
                             key_timestamp, start_time, end_time);
                    
                    // Verificar que el valor es un hash válido
                    if value.len() == 32 {
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&value);
                        
                        // Intentamos recuperar el bloque para confirmar
                        if let Ok(Some(block)) = self.get_block(&hash) {
                            println!("  → Bloque altura {}, timestamp {}", 
                                     block.header.height, block.header.timestamp);
                        }
                    }
                } else {
                    println!("✗ Timestamp {} fuera de rango [{},{}]", 
                             key_timestamp, start_time, end_time);
                }
            }
        }
        
        println!("Todos los timestamps: {:?}", all_timestamps);
        println!("Matches: {:?}", successful_matches);
        
        Ok(count)
    }

    /// Finds a block by its exact timestamp.
    ///
    /// # Parameters
    /// * `timestamp` - The timestamp to search for
    ///
    /// # Returns
    /// A result containing the block if found, None if not found, or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database read fails
    pub fn find_block_by_timestamp(&self, timestamp: u64) -> Result<Option<Block>, Error> {
        let cfs = self.get_column_families()?;

        let timestamp_bytes = timestamp.to_le_bytes();
        let mut iter = self.db.iterator_cf(
            cfs.timestamp_index,
            rocksdb::IteratorMode::From(&timestamp_bytes, rocksdb::Direction::Forward),
        );

        // Check the first entry (equal or just after)
        if let Some(Ok((key, value))) = iter.next() {
            if key.len() >= 8 {
                let entry_ts = u64::from_le_bytes(key[..8].try_into().unwrap());
                if entry_ts == timestamp || entry_ts < timestamp + 1000 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&value);
                    return self.get_block(&hash);
                }
            }
        }

        // Check the previous entry (just before)
        let mut iter = self.db.iterator_cf(
            cfs.timestamp_index,
            rocksdb::IteratorMode::From(&timestamp_bytes, rocksdb::Direction::Reverse),
        );
        if let Some(Ok((_, value))) = iter.next() {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&value);
            return self.get_block(&hash);
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::transaction::Transaction;
    use tempfile::tempdir;

    fn create_test_transaction(index: u8) -> Transaction {
        let mut sender = [0u8; 32];
        let mut recipient = [0u8; 32];
        sender[0] = index;
        recipient[0] = index + 1;
        Transaction::new(sender, recipient, 100 * index as u64, 10, 0, vec![])
    }

    fn create_test_block(height: u64, prev_hash: Hash, tx_count: u8) -> Block {
        let mut transactions = Vec::new();
        for i in 0..tx_count {
            transactions.push(create_test_transaction(i + 1));
        }
        let validator = [0u8; 32];

        Block::new(prev_hash, height, transactions, validator).unwrap()
    }

    #[test]
    fn test_block_storage_retrieval() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        {
            // Open the storage
            let storage = BlockchainStorage::open(&config).unwrap();

            // Create a test block
            let genesis_hash = [0u8; 32];
            let block = create_test_block(1, genesis_hash, 5);
            let block_hash = block.header.hash();

            // Store the block
            storage.store_block(&block).unwrap();

            // Retrieve the block by hash
            let retrieved_block = storage.get_block(&block_hash).unwrap().unwrap();
            assert_eq!(retrieved_block.header.height, block.header.height);
            assert_eq!(retrieved_block.transactions.len(), block.transactions.len());

            // Retrieve by height
            let by_height = storage.get_block_by_height(1).unwrap().unwrap();
            assert_eq!(by_height.header.hash(), block_hash);
        }

        // Clean up (tempfile will do this automatically, but let's be explicit)
        temp_dir.close().unwrap();
    }

    #[test]
    fn test_transaction_indexing() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        {
            // Open the storage
            let storage = BlockchainStorage::open(&config).unwrap();

            // Create a test block with transactions
            let genesis_hash = [0u8; 32];
            let block = create_test_block(1, genesis_hash, 5);

            // Get the second transaction's hash
            let tx_hash = block.transactions[1].hash();

            // Store the block
            storage.store_block(&block).unwrap();

            // Retrieve the transaction
            let tx = storage.get_transaction(&tx_hash).unwrap().unwrap();

            // Verify it's the correct transaction
            assert_eq!(tx.hash(), tx_hash);
        }

        // Clean up
        temp_dir.close().unwrap();
    }

    #[test]
    fn test_account_state() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        {
            // Open the storage
            let storage = BlockchainStorage::open(&config).unwrap();

            // Create a test account address and state
            let address = [42u8; 32];
            let mut state = AccountState::new();
            state.balance = 1000;
            state.nonce = 5;

            // Store the state
            storage.store_account_state(&address, &state).unwrap();
        
            // Retrieve the state
            let retrieved_state = storage.get_account_state(&address).unwrap().unwrap();

            // Verify it's correct
            assert_eq!(retrieved_state.balance, 1000);
            assert_eq!(retrieved_state.nonce, 5);
        }

        // Clean up
        temp_dir.close().unwrap();
    }

    #[test]
    fn test_chain_integrity() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        {
            // Open the storage
            let storage = BlockchainStorage::open(&config).unwrap();
    
            // Create a chain of blocks
            let genesis_block = create_test_block(0, [0u8; 32], 1); // height 0, prev_hash zeros
            storage.store_block(&genesis_block).unwrap();
            println!("Genesis block hash: {}", hex::encode(genesis_block.header.hash()));

            let block1 = create_test_block(1, genesis_block.header.hash(), 2);
            let block1_hash = block1.header.hash();
            println!("Block 1 hash: {}", hex::encode(block1_hash));
            println!("Block 1 prev_hash: {}", hex::encode(block1.header.prev_hash));
    
            let block2 = create_test_block(2, block1_hash, 3);
            let block2_hash = block2.header.hash();
            println!("Block 2 hash: {}", hex::encode(block2_hash));
            println!("Block 2 prev_hash: {}", hex::encode(block2.header.prev_hash));
    
            let block3 = create_test_block(3, block2_hash, 1);
            println!("Block 3 hash: {}", hex::encode(block3.header.hash()));
            println!("Block 3 prev_hash: {}", hex::encode(block3.header.prev_hash));
    
            // Store the blocks
            storage.store_block(&block1).unwrap();
            storage.store_block(&block2).unwrap();
            storage.store_block(&block3).unwrap();

            // Verifica manualmente los enlaces
            for height in 1..=3 {
                let stored_hash = storage.get_block_hash_by_height(height).unwrap();
                let stored_block = storage.get_block(&stored_hash).unwrap().unwrap();
                let prev_block_hash = storage.get_block_hash_by_height(height - 1).unwrap();
                
                println!("Verificando bloque {}", height);
                println!("  Hash guardado: {}", hex::encode(stored_hash));
                println!("  prev_hash: {}", hex::encode(stored_block.header.prev_hash));
                println!("  Hash anterior esperado: {}", hex::encode(prev_block_hash));
                
                assert_eq!(stored_block.header.prev_hash, prev_block_hash, 
                           "Error en enlace entre bloque {} y {}", height, height-1);
            }

            // Verify integrity
            assert!(storage.verify_integrity().unwrap());
        }

        // Clean up
        temp_dir.close().unwrap();
    }

    #[test]
    fn test_timestamp_index() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };
        
        {
            // Open the storage
            let storage = BlockchainStorage::open(&config).unwrap();
            
            // Create blocks with different timestamps
            let genesis_hash = [0u8; 32];
            let validator = [0u8; 32];

            // Timestamps with 1-minute intervals
            let timestamps = [
                1617235200000, // 2021-04-01 00:00:00
                1617235260000, // 2021-04-01 00:01:00
                1617235320000, // 2021-04-01 00:02:00
            ];
            
            // Create and store blocks
            let mut prev_hash = genesis_hash;
            for (i, &timestamp) in timestamps.iter().enumerate() {
                let height = i as u64 + 1;
                
                // Create a block with controlled timestamp
                let mut block = Block::new(
                    prev_hash,
                    height,
                    vec![],
                    validator,
                ).unwrap();
                
                // Set custom timestamp
                block.header.timestamp = timestamp;
                println!("Creando bloque con timestamp: {}", timestamp);
                
                // Store block
                storage.store_block(&block).unwrap();
                
                prev_hash = block.header.hash();
            }
            
            // Test getting blocks by time range
            let blocks = storage.get_blocks_by_time_range(
                1617235200000, // 00:00:00
                1617235320000, // 00:02:00
                10
            ).unwrap();
            
            // Should return 3 blocks (00:00, 01:00, 02:00)
            println!("Bloques encontrados: {}", blocks.len());
            for (i, block) in blocks.iter().enumerate() {
                println!("Bloque {}: timestamp={}", i, block.header.timestamp);
            }
            assert_eq!(blocks.len(), 3);
            
            // Test counting blocks in time range
            let count = storage.count_blocks_by_time_range(
                 1617235200000, // 00:00:00
                 1617235320000, // 00:02:00
            ).unwrap();
            
            println!("Conteo final: {}", count);
            assert_eq!(count, 3);
        }
        
        // Clean up
        drop(temp_dir);
    }
}

/// Creates column family options optimized for blockchain storage.
///
pub fn configure_column_family_options() -> Vec<ColumnFamilyDescriptor> {
    // Default options
    let mut cf_opts = Options::default();
    cf_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);

    // Block column family (optimize for large values)
    let mut block_cf_opts = Options::default();
    block_cf_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);

    // Create block-based options with bloom filter for blocks
    let mut block_based_opts = rocksdb::BlockBasedOptions::default();
    block_based_opts.set_bloom_filter(10.0, false);
    block_cf_opts.set_block_based_table_factory(&block_based_opts);
    block_cf_opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB

    // Transaction column family (optimize for point lookups)
    let mut txs_cf_opts = Options::default();
    txs_cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

    // Create block-based options with bloom filter for transactions
    let mut txs_block_opts = rocksdb::BlockBasedOptions::default();
    txs_block_opts.set_bloom_filter(10.0, false);
    txs_cf_opts.set_block_based_table_factory(&txs_block_opts);

    // State column family (optimize for frequent updates)
    let mut state_cf_opts = Options::default();
    state_cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

    // Create block-based options with bloom filter for state
    let mut state_block_opts = rocksdb::BlockBasedOptions::default();
    state_block_opts.set_bloom_filter(10.0, false);
    state_cf_opts.set_block_based_table_factory(&state_block_opts);
    state_cf_opts.set_write_buffer_size(32 * 1024 * 1024); // 32MB

    vec![
        ColumnFamilyDescriptor::new("blocks", block_cf_opts),
        ColumnFamilyDescriptor::new("block_height", cf_opts.clone()),
        ColumnFamilyDescriptor::new("transactions", txs_cf_opts),
        ColumnFamilyDescriptor::new("account_state", state_cf_opts),
    ]
}

/// Store interface for working with blocks
pub mod block_store;

/// Store interface for working with account state
pub mod state_store;

// Re-export StateStore
pub use state_store::StateStore;

// Import the migration module
pub mod migration;

// Make ensure_compatible_schema public
pub use migration::ensure_compatible_schema;
