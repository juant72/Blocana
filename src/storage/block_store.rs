//! Block storage implementation
//!
//! This module provides a specialized interface for working with block storage.

use super::{BlockchainStorage, Error};
use crate::block::Block;
use crate::types::Hash;

/// A specialized store for block operations
pub struct BlockStore<'a> {
    /// Reference to the underlying storage
    storage: &'a BlockchainStorage,
}

impl<'a> BlockStore<'a> {
    /// Create a new block store
    pub fn new(storage: &'a BlockchainStorage) -> Self {
        Self { storage }
    }

    /// Store a block
    pub fn store_block(&self, block: &Block) -> Result<Hash, Error> {
        // Calculate block hash
        let hash = block.header.hash();

        // Store in the database
        self.storage.store_block(block)?;

        Ok(hash)
    }

    /// Get a block by its hash
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>, Error> {
        self.storage.get_block(hash)
    }

    /// Get a block by its height
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, Error> {
        self.storage.get_block_by_height(height)
    }

    /// Get the block hash at a specific height
    pub fn get_block_hash_by_height(&self, height: u64) -> Result<Hash, Error> {
        self.storage.get_block_hash_by_height(height)
    }

    /// Get the latest block
    pub fn get_latest_block(&self) -> Result<Option<Block>, Error> {
        let latest_height = self.storage.get_latest_height()?;
        if latest_height == 0 {
            return Ok(None);
        }

        self.storage.get_block_by_height(latest_height)
    }

    /// Get the latest block height
    pub fn get_latest_height(&self) -> Result<u64, Error> {
        self.storage.get_latest_height()
    }

    /// Check if a block exists
    pub fn block_exists(&self, hash: &Hash) -> Result<bool, Error> {
        let exists = self.storage.get_block(hash)?.is_some();
        Ok(exists)
    }

    /// Get blocks in a range of heights
    pub fn get_blocks_in_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<Block>, Error> {
        if end_height < start_height {
            return Err(Error::Other("Invalid height range".into()));
        }

        let mut blocks = Vec::new();
        for height in start_height..=end_height {
            if let Some(block) = self.storage.get_block_by_height(height)? {
                blocks.push(block);
            } else {
                break; // No more blocks
            }
        }

        Ok(blocks)
    }

    /// Verify chain integrity
    pub fn verify_chain_integrity(&self) -> Result<bool, Error> {
        self.storage.verify_integrity()
    }
}

#[cfg(test)]
mod tests {
    use super::super::StorageConfig;
    use super::*;
    use tempfile::tempdir;

    // Helper to create test transactions and blocks
    fn create_test_block(height: u64, prev_hash: Hash) -> Block {
        let validator = [0u8; 32];
        let transactions = Vec::new(); // Empty transactions for simplicity

        Block::new(prev_hash, height, transactions, validator).unwrap()
    }

    #[test]
    fn test_block_store_operations() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        // Wrap database operations in a block to ensure all variables are dropped
        {
            // Open the storage and create block store
            let storage = BlockchainStorage::open(&config).unwrap();
            let block_store = BlockStore::new(&storage);

            // Create and store a chain of blocks
            let genesis_hash = [0u8; 32];

            // Block 1
            let block1 = create_test_block(1, genesis_hash);
            let block1_hash = block_store.store_block(&block1).unwrap();

            // Block 2
            let block2 = create_test_block(2, block1_hash);
            let block2_hash = block_store.store_block(&block2).unwrap();

            // Block 3
            let block3 = create_test_block(3, block2_hash);
            let block3_hash = block_store.store_block(&block3).unwrap();

            // Test get_block
            let retrieved_block = block_store.get_block(&block2_hash).unwrap().unwrap();
            assert_eq!(retrieved_block.header.height, 2);

            // Test get_block_by_height
            let by_height = block_store.get_block_by_height(3).unwrap().unwrap();
            assert_eq!(by_height.header.hash(), block3_hash);

            // Test get_latest_block
            let latest = block_store.get_latest_block().unwrap().unwrap();
            assert_eq!(latest.header.height, 3);

            // Test get_latest_height
            assert_eq!(block_store.get_latest_height().unwrap(), 3);

            // Test block_exists
            assert!(block_store.block_exists(&block1_hash).unwrap());
            assert!(!block_store.block_exists(&[255u8; 32]).unwrap());

            // Test get_blocks_in_range
            let blocks = block_store.get_blocks_in_range(1, 3).unwrap();
            assert_eq!(blocks.len(), 3);
            assert_eq!(blocks[0].header.height, 1);
            assert_eq!(blocks[2].header.height, 3);
        }

        // Clean up
        temp_dir.close().unwrap();
    }
}
