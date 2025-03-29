//! Storage layer for the Blocana blockchain
//!
//! This module provides persistence mechanisms for blockchain data using RocksDB.
//! It handles storage and retrieval of blocks, transactions, and account state.

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
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "data/blocana_db".to_string(),
            max_open_files: 1000,
            write_buffer_size: 64 * 1024 * 1024, // 64MB
            max_write_buffer_number: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// Structure to hold column family handles
pub struct BlockchainColumnFamilies<'a> {
    pub blocks: &'a ColumnFamily,
    pub block_height: &'a ColumnFamily,
    pub transactions: &'a ColumnFamily,
    pub account_state: &'a ColumnFamily,
}

/// Transaction location information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct TxLocation {
    /// Block hash containing the transaction
    pub block_hash: Hash,
    /// Index of transaction within the block
    pub index: u32,
}

/// Main storage interface for the blockchain
pub struct BlockchainStorage {
    /// RocksDB database instance
    db: DB,
}

impl BlockchainStorage {
    /// Open the blockchain storage
    pub fn open(config: &StorageConfig) -> Result<Self, Error> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config.db_path)?;

        // Define column families
        let cf_names = ["blocks", "block_height", "transactions", "account_state"];

        // Configure database options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_keep_log_file_num(10);
        opts.set_max_open_files(config.max_open_files);
        opts.set_write_buffer_size(config.write_buffer_size);
        opts.set_max_write_buffer_number(config.max_write_buffer_number);
        opts.set_target_file_size_base(config.target_file_size_base);

        // Open database with column families
        let db = DB::open_cf(&opts, &config.db_path, cf_names)?;

        Ok(Self { db })
    }

    /// Open the storage with custom column family options
    pub fn open_with_cf_options<P: AsRef<Path>>(
        path: P,
        options: Options,
        cf_descriptors: Vec<ColumnFamilyDescriptor>,
    ) -> Result<Self, Error> {
        let db = DB::open_cf_descriptors(&options, path, cf_descriptors)?;
        Ok(Self { db })
    }

    /// Get column family handles
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

        let account_state = self.db.cf_handle("account_state").ok_or_else(|| {
            Error::Database("Column family 'account_state' not found".to_string())
        })?;

        Ok(BlockchainColumnFamilies {
            blocks,
            block_height,
            transactions,
            account_state,
        })
    }

    /// Store a block in the database
    pub fn store_block(&self, block: &Block) -> Result<(), Error> {
        let cfs = self.get_column_families()?;

        let block_bytes = bincode::encode_to_vec(block, bincode::config::standard())?;
        let block_hash = block.header.hash();
        let height_bytes = block.header.height.to_le_bytes();

        // Create a write batch for atomic operations
        let mut batch = WriteBatch::default();

        // Add block to blocks column family
        batch.put_cf(cfs.blocks, block_hash, &block_bytes);

        // Add height -> hash mapping
        batch.put_cf(cfs.block_height, &height_bytes, block_hash);

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

    /// Get a block by its hash
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

    /// Get a block by its height
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

    /// Get the block hash at a specific height
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

    /// Get the latest block height
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

    /// Get a transaction by its hash
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

    /// Store account state
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

    /// Get account state
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

    /// Create a database backup
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

    /// Restore from a backup
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

    /// Verify database integrity
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

    /// Get raw database handle
    pub fn raw_db(&self) -> &DB {
        &self.db
    }
}

/// Create column family options optimized for blockchain storage
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
}

/// Store interface for working with blocks
pub mod block_store;

/// Store interface for working with account state
pub mod state_store;
