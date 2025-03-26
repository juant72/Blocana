# Database Design Guide for Stage 1

This document details the SledDB implementation for Blocana's storage layer.

## SledDB Integration

SledDB is an embedded database that provides ACID guarantees with excellent performance characteristics. It uses a log-structured merge tree architecture which is ideal for blockchain's append-heavy workload.

### Database Trees

Create the following tree structures:

#### 1. Blocks Tree
- **Purpose**: Store complete blocks
- **Key**: Block hash (32 bytes)
- **Value**: Serialized block data
- **Access Pattern**: Random reads by hash, sequential reads by traversing prev_hash links

#### 2. Block Height Index
- **Purpose**: Map heights to block hashes for quick lookups
- **Key**: Block height (8 bytes, little-endian)
- **Value**: Block hash (32 bytes)
- **Access Pattern**: Random reads by height, sequential scans for iterating all blocks

#### 3. Transaction Index
- **Purpose**: Allow quick lookup of transactions
- **Key**: Transaction hash (32 bytes)
- **Value**: Block hash (32 bytes) + offset in block
- **Access Pattern**: Random reads by transaction hash

#### 4. Account State Tree
- **Purpose**: Store current account state
- **Key**: Account address (32 bytes)
- **Value**: Serialized account data (balance, nonce, etc.)
- **Access Pattern**: Random reads/writes by address

### Implementation Details

#### Opening and Configuring the Database
```rust
pub fn open_database(config: &StorageConfig) -> Result<sled::Db, Error> {
    // Create directory if it doesn't exist
    std::fs::create_dir_all(&config.db_path)?;
    
    // Configure database
    let db_config = sled::Config::new()
        .path(&config.db_path)
        .cache_capacity(config.cache_size_bytes)
        .flush_every_ms(Some(1000))
        .mode(sled::Mode::HighThroughput);
        
    // Open database
    let db = db_config.open()?;
    
    Ok(db)
}

pub fn open_trees(db: &sled::Db) -> Result<BlockchainTrees, Error> {
    let blocks = db.open_tree("blocks")?;
    let block_height = db.open_tree("block_height")?;
    let transactions = db.open_tree("transactions")?;
    let account_state = db.open_tree("account_state")?;
    
    Ok(BlockchainTrees {
        blocks,
        block_height,
        transactions,
        account_state,
    })
}
```

#### Serialization Helpers
```rust
pub fn serialize_block(block: &Block) -> Result<Vec<u8>, Error> {
    bincode::serialize(block).map_err(Error::Serialization)
}

pub fn deserialize_block(data: &[u8]) -> Result<Block, Error> {
    bincode::deserialize(data).map_err(Error::Serialization)
}
```

#### Transaction Processing

For writing blocks and transactions, use batch processing to ensure atomicity:

```rust
pub fn store_block(tree: &BlockchainTrees, block: &Block) -> Result<(), Error> {
    let block_bytes = serialize_block(block)?;
    let block_hash = block.header.hash();
    let height_bytes = block.header.height.to_le_bytes();
    
    // Create a batch for atomic operations
    let mut batch = sled::Batch::default();
    
    // Add block to blocks tree
    tree.blocks.insert(block_hash, block_bytes)?;
    
    // Add height -> hash mapping
    tree.block_height.insert(height_bytes, block_hash)?;
    
    // Index each transaction
    for (i, tx) in block.transactions.iter().enumerate() {
        let tx_hash = tx.hash();
        let tx_location = TxLocation {
            block_hash,
            index: i as u32,
        };
        let tx_loc_bytes = bincode::serialize(&tx_location)?;
        tree.transactions.insert(tx_hash, tx_loc_bytes)?;
    }
    
    Ok(())
}
```

### Performance Considerations

1. **Batching**: Use batches for multiple operations that need to be atomic
2. **Caching**: Adjust cache size based on available memory
3. **Compression**: Enable compression for larger storage savings
4. **Background Flushing**: Use `flush_async` for non-blocking durability
5. **Indexing**: Create indices only for common query patterns

### Error Handling

Implement a comprehensive error handling strategy:

```rust
#[derive(Debug)]
pub enum StorageError {
    /// IO error
    IO(std::io::Error),
    /// Database error
    Database(sled::Error),
    /// Serialization error
    Serialization(bincode::Error),
    /// Item not found
    NotFound,
    /// Other storage errors
    Other(String),
}

impl From<std::io::Error> for StorageError {
    // Implementation
}

impl From<sled::Error> for StorageError {
    // Implementation
}

impl From<bincode::Error> for StorageError {
    // Implementation
}
```

## Recovery and Data Integrity

### Crash Recovery

SledDB provides automatic recovery for crash scenarios:

```rust
// To ensure crash recovery is enabled
let config = sled::Config::new()
    .path(&config.db_path)
    .cache_capacity(config.cache_size_bytes)
    .flush_every_ms(Some(1000))
    .use_compression(true)
    .mode(sled::Mode::HighThroughput)
    .create_new(false);  // Open existing DB if available
```

### Data Integrity Verification

Implement periodic integrity checks:

```rust
pub fn verify_database_integrity(db: &BlockchainStorage) -> Result<bool, StorageError> {
    // Check block chain integrity
    let latest_height = db.get_latest_height()?;
    let mut current_hash = db.get_block_hash_by_height(latest_height)?;
    
    // Walk backwards through the chain
    for height in (0..latest_height).rev() {
        let block = db.get_block(&current_hash)?
            .ok_or(StorageError::Other("Block not found in integrity check".into()))?;
            
        // Verify this block points to the correct previous block
        if height > 0 {
            let expected_prev_hash = db.get_block_hash_by_height(height - 1)?;
            if block.header.prev_hash != expected_prev_hash {
                return Ok(false);
            }
        }
        
        current_hash = block.header.prev_hash;
    }
    
    Ok(true)
}
```

## Data Backup and Migration

### Backup Strategy

Implement a backup mechanism for the database:

```rust
pub fn backup_database(db: &sled::Db, backup_path: &str) -> Result<(), StorageError> {
    // Ensure backup directory exists
    std::fs::create_dir_all(backup_path)?;
    
    // Create a timestamped backup file
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StorageError::Other("Time went backwards".into()))?
        .as_secs();
    
    let backup_file = format!("{}/blocana-backup-{}.snap", backup_path, timestamp);
    
    // Generate a snapshot
    let snapshot = db.export();
    
    // Write to file
    std::fs::write(&backup_file, snapshot)
        .map_err(|e| StorageError::IO(e))?;
    
    Ok(())
}
```

### Database Migration

If schema changes are needed, implement a migration system:

```rust
pub fn check_and_migrate_database(db: &sled::Db, current_version: u32) -> Result<u32, StorageError> {
    // Get database version
    let version_tree = db.open_tree("metadata")?;
    let db_version = match version_tree.get("version")? {
        Some(bytes) => {
            let mut version_bytes = [0u8; 4];
            version_bytes.copy_from_slice(&bytes);
            u32::from_le_bytes(version_bytes)
        },
        None => {
            // New database, set initial version
            version_tree.insert("version", &1u32.to_le_bytes())?;
            1
        }
    };
    
    // If versions match, no migration needed
    if db_version == current_version {
        return Ok(current_version);
    }
    
    // Perform migrations sequentially
    let mut version = db_version;
    while version < current_version {
        match version {
            1 => migrate_v1_to_v2(db)?,
            2 => migrate_v2_to_v3(db)?,
            // Add more migration steps as needed
            _ => return Err(StorageError::Other(format!("Unknown version: {}", version))),
        }
        version += 1;
    }
    
    // Update database version
    version_tree.insert("version", &current_version.to_le_bytes())?;
    
    Ok(current_version)
}
```

## Conclusion

This database design guide provides a comprehensive approach to implementing the storage layer for Blocana using SledDB. Following these guidelines will result in a robust, efficient storage solution that can handle the demands of a blockchain environment while minimizing resource usage.

When implementing this design, remember that proper error handling, atomic operations, and careful performance optimization are essential for ensuring the reliability and efficiency of the blockchain storage system.

--- End of Document ---
