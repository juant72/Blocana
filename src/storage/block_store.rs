//! Block storage implementation for the Blocana blockchain
//!
//! This module provides a specialized interface for working with block storage,
//! offering methods tailored to block operations while abstracting the underlying
//! storage details.

use super::{BlockchainStorage, Error};
use crate::block::Block;
use crate::types::Hash;

/// A specialized store for block operations
///
/// Provides a higher-level interface for working with blocks in storage,
/// abstracting the underlying database operations.
pub struct BlockStore<'a> {
    /// Reference to the underlying storage
    storage: &'a BlockchainStorage,
}

impl<'a> BlockStore<'a> {
    /// Creates a new block store.
    ///
    /// # Parameters
    /// * `storage` - Reference to the blockchain storage
    ///
    /// # Returns
    /// A new `BlockStore` instance
    pub fn new(storage: &'a BlockchainStorage) -> Self {
        Self { storage }
    }

    /// Stores a block and returns its hash.
    ///
    /// # Parameters
    /// * `block` - The block to store
    ///
    /// # Returns
    /// The hash of the stored block
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn store_block(&self, block: &Block) -> Result<Hash, Error> {
        // Calculate block hash
        let hash = block.header.hash();

        // Store in the database
        self.storage.store_block(block)?;

        Ok(hash)
    }

    /// Gets a block by its hash.
    ///
    /// # Parameters
    /// * `hash` - The hash of the block to retrieve
    ///
    /// # Returns
    /// The block if found, None if not found
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn get_block(&self, hash: &Hash) -> Result<Option<Block>, Error> {
        self.storage.get_block(hash)
    }

    /// Gets a block by its height.
    ///
    /// # Parameters
    /// * `height` - The height of the block to retrieve
    ///
    /// # Returns
    /// The block if found, None if not found
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, Error> {
        self.storage.get_block_by_height(height)
    }

    /// Gets the block hash at a specific height.
    ///
    /// # Parameters
    /// * `height` - The block height
    ///
    /// # Returns
    /// The hash of the block at the specified height
    ///
    /// # Errors
    /// Returns an error if no block exists at the given height or the storage operation fails
    pub fn get_block_hash_by_height(&self, height: u64) -> Result<Hash, Error> {
        self.storage.get_block_hash_by_height(height)
    }

    /// Gets the latest block in the blockchain.
    ///
    /// # Returns
    /// The latest block if any blocks exist, None if the blockchain is empty
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn get_latest_block(&self) -> Result<Option<Block>, Error> {
        let latest_height = self.storage.get_latest_height()?;
        if latest_height == 0 {
            return Ok(None);
        }

        self.storage.get_block_by_height(latest_height)
    }

    /// Gets the height of the latest block.
    ///
    /// # Returns
    /// The height of the latest block, or 0 if no blocks exist
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn get_latest_height(&self) -> Result<u64, Error> {
        self.storage.get_latest_height()
    }

    /// Checks if a block with the given hash exists.
    ///
    /// # Parameters
    /// * `hash` - The block hash to check
    ///
    /// # Returns
    /// `true` if the block exists, `false` otherwise
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn block_exists(&self, hash: &Hash) -> Result<bool, Error> {
        let exists = self.storage.get_block(hash)?.is_some();
        Ok(exists)
    }

    /// Gets blocks within a range of heights.
    ///
    /// # Parameters
    /// * `start_height` - The starting height (inclusive)
    /// * `end_height` - The ending height (inclusive)
    ///
    /// # Returns
    /// A vector of blocks in the specified range
    ///
    /// # Errors
    /// Returns an error if:
    /// - `end_height` is less than `start_height`
    /// - The storage operation fails
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

    /// Verifies the integrity of the blockchain.
    ///
    /// # Returns
    /// `true` if the blockchain is internally consistent, `false` otherwise
    ///
    /// # Errors
    /// Returns an error if the verification process fails due to storage errors
    pub fn verify_chain_integrity(&self) -> Result<bool, Error> {
        self.storage.verify_integrity()
    }

    /// Get blocks in a time range if timestamp index is available
    #[cfg(feature = "timestamp_index")]
    pub fn get_blocks_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
        limit: usize,
    ) -> Result<Vec<Block>, Error> {
        self.storage
            .get_blocks_by_time_range(start_time, end_time, limit)
    }

    /// Count blocks within a timestamp range
    #[cfg(feature = "timestamp_index")]
    pub fn count_blocks_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<usize, Error> {
        self.storage
            .count_blocks_by_time_range(start_time, end_time)
    }

    /// Find a block close to the given timestamp if timestamp index is available
    #[cfg(feature = "timestamp_index")]
    pub fn find_block_by_timestamp(&self, timestamp: u64) -> Result<Option<Block>, Error> {
        self.storage.find_block_by_timestamp(timestamp)
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

    #[test]
    #[cfg(feature = "timestamp_index")]
    fn test_block_store_timestamp_operations() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };

        {
            // Open the storage and create block store
            let storage = BlockchainStorage::open(&config).unwrap();
            let block_store = BlockStore::new(&storage);

            // Create and store a chain of blocks with different timestamps
            let genesis_hash = [0u8; 32];
            let timestamps = [
                1617235200000, // 2021-04-01 00:00:00
                1617235260000, // 2021-04-01 00:01:00
                1617235320000, // 2021-04-01 00:02:00
            ];

            let mut prev_hash = genesis_hash;
            for (i, &timestamp) in timestamps.iter().enumerate() {
                let height = i as u64 + 1;

                // Create a block with controlled timestamp
                let mut block = create_test_block(height, prev_hash);
                block.header.timestamp = timestamp;

                // Store block
                prev_hash = block_store.store_block(&block).unwrap();
            }

            // Test get_blocks_by_time_range
            let blocks = block_store
                .get_blocks_by_time_range(
                    1617235200000, // 00:00:00
                    1617235260000, // 00:01:00
                    10,
                )
                .unwrap();

            assert_eq!(blocks.len(), 2);
            assert_eq!(blocks[0].header.timestamp, 1617235200000);
            assert_eq!(blocks[1].header.timestamp, 1617235260000);

            // Test find_block_by_timestamp
            let found_block = block_store.find_block_by_timestamp(1617235250000).unwrap();
            assert!(found_block.is_some());
            assert_eq!(found_block.unwrap().header.timestamp, 1617235200000);
        }

        // Clean up
        temp_dir.close().unwrap();
    }
}
