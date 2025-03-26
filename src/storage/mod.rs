//! Storage functionality for the Blocana blockchain
//!
//! This module contains the storage layer implementation.

use crate::block::{Block, Hash};

/// Configuration for the storage layer
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Path to the database directory
    pub db_path: String,
    /// Maximum size of the database in bytes
    pub max_db_size: usize,
    /// Should prune old blocks
    pub enable_pruning: bool,
    /// Number of blocks to keep when pruning
    pub pruning_depth: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "data".into(),
            max_db_size: 1024 * 1024 * 100, // 100 MB
            enable_pruning: true,
            pruning_depth: 1000, // Keep last 1000 blocks
        }
    }
}

/// Main storage for the blockchain
pub struct BlockchainStorage {
    /// Storage configuration
    config: StorageConfig,
    /// Block store
    block_store: BlockStore,
    /// State store
    state_store: StateStore,
}

impl BlockchainStorage {
    /// Create a new blockchain storage
    pub fn new(config: &StorageConfig) -> Result<Self, Error> {
        // In a real implementation, we would initialize SledDB here
        let block_store = BlockStore::new(&config.db_path)?;
        let state_store = StateStore::new(&config.db_path)?;
        
        Ok(Self {
            config: config.clone(),
            block_store,
            state_store,
        })
    }
    
    /// Get a block by its hash
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>, Error> {
        self.block_store.get(hash)
    }
    
    /// Store a block
    pub fn store_block(&mut self, block: &Block) -> Result<(), Error> {
        self.block_store.put(block)
    }
}

/// Store for blocks
pub struct BlockStore {
    /// Path to the database
    db_path: String,
}

impl BlockStore {
    /// Create a new block store
    pub fn new(db_path: &str) -> Result<Self, Error> {
        // In a real implementation, we would initialize SledDB here
        Ok(Self {
            db_path: db_path.into(),
        })
    }
    
    /// Get a block by its hash
    pub fn get(&self, hash: &Hash) -> Result<Option<Block>, Error> {
        // In a real implementation, we would fetch from SledDB
        Ok(None)
    }
    
    /// Store a block
    pub fn put(&mut self, block: &Block) -> Result<(), Error> {
        // In a real implementation, we would store in SledDB
        Ok(())
    }
}

/// Store for state
pub struct StateStore {
    /// Path to the database
    db_path: String,
}

impl StateStore {
    /// Create a new state store
    pub fn new(db_path: &str) -> Result<Self, Error> {
        // In a real implementation, we would initialize SledDB here
        Ok(Self {
            db_path: db_path.into(),
        })
    }
}

/// Error types for storage operations
#[derive(Debug)]
pub enum Error {
    /// IO error
    IO(std::io::Error),
    /// Database error
    Database(String),
    /// Serialization error
    Serialization(String),
    /// Other errors
    Other(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}
