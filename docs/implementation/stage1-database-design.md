# sDatabase Design Guide for Stage 1

This document details the RocksDB implementation for Blocana's storage layer.

## RocksDB Integration

RocksDB is a high-performance embedded database that provides ACID guarantees with outstanding throughput characteristics. It uses a log-structured merge tree architecture which is ideal for blockchain's append-heavy workload and has been battle-tested in numerous production blockchain systems like Ethereum and Bitcoin Core.

### Database Column Families

Create the following column families:

#### 1. Blocks Column Family

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

#### 4. Account State Column Family

- **Purpose**: Store current account state
- **Key**: Account address (32 bytes)
- **Value**: Serialized account data (balance, nonce, etc.)
- **Access Pattern**: Random reads/writes by address

### Implementation Details

#### Opening and Configuring the Database

```rust
pub fn open_database(config: &StorageConfig) -> Result<rocksdb::DB, Error> {
    // Create directory if it doesn't exist
    std::fs::create_dir_all(&config.db_path)?;
  
    // Define column families
    let cf_names = ["blocks", "block_height", "transactions", "account_state"];
  
    // Configure database options
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_keep_log_file_num(10);
    opts.set_max_open_files(config.max_open_files);
    opts.set_write_buffer_size(config.write_buffer_size);
  
    // Open database with column families
    let db = rocksdb::DB::open_cf(&opts, &config.db_path, &cf_names)?;
  
    Ok(db)
}

pub fn get_column_families(db: &rocksdb::DB) -> BlockchainColumnFamilies {
    let blocks = db.cf_handle("blocks").expect("Column family exists");
    let block_height = db.cf_handle("block_height").expect("Column family exists");
    let transactions = db.cf_handle("transactions").expect("Column family exists");
    let account_state = db.cf_handle("account_state").expect("Column family exists");
  
    BlockchainColumnFamilies {
        blocks,
        block_height,
        transactions,
        account_state,
    }
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

For writing blocks and transactions, use batched writes to ensure atomicity:

```rust
pub fn store_block(db: &rocksdb::DB, cfs: &BlockchainColumnFamilies, block: &Block) -> Result<(), Error> {
    let block_bytes = serialize_block(block)?;
    let block_hash = block.header.hash();
    let height_bytes = block.header.height.to_le_bytes();
  
    // Create a write batch for atomic operations
    let mut batch = rocksdb::WriteBatch::default();
  
    // Add block to blocks column family
    batch.put_cf(&cfs.blocks, block_hash, &block_bytes);
  
    // Add height -> hash mapping
    batch.put_cf(&cfs.block_height, &height_bytes, block_hash);
  
    // Index each transaction
    for (i, tx) in block.transactions.iter().enumerate() {
        let tx_hash = tx.hash();
        let tx_location = TxLocation {
            block_hash,
            index: i as u32,
        };
        let tx_loc_bytes = bincode::serialize(&tx_location)?;
        batch.put_cf(&cfs.transactions, tx_hash, &tx_loc_bytes);
    }
  
    // Write batch atomically
    db.write(batch)?;
  
    Ok(())
}
```

### Performance Considerations

1. **Column Family Tuning**: Configure different column families with different options
2. **Block Cache**: Adjust block cache size based on available memory
3. **Bloom Filters**: Enable bloom filters for faster key lookups
4. **Compression**: Configure compression per column family
5. **Compaction Strategy**: Use appropriate compaction strategies for different data types

```rust
// Example of column family-specific tuning
pub fn configure_column_family_options() -> Vec<rocksdb::ColumnFamilyDescriptor> {
    let mut cf_opts = rocksdb::Options::default();
    cf_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
  
    let mut block_cf_opts = rocksdb::Options::default();
    block_cf_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
    block_cf_opts.set_bloom_filter_bits_per_key(10.0);
  
    let mut txs_cf_opts = rocksdb::Options::default();
    txs_cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    txs_cf_opts.set_bloom_filter_bits_per_key(10.0);
  
    let mut state_cf_opts = rocksdb::Options::default();
    state_cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    state_cf_opts.set_bloom_filter_bits_per_key(10.0);
  
    vec![
        rocksdb::ColumnFamilyDescriptor::new("blocks", block_cf_opts),
        rocksdb::ColumnFamilyDescriptor::new("block_height", cf_opts.clone()),
        rocksdb::ColumnFamilyDescriptor::new("transactions", txs_cf_opts),
        rocksdb::ColumnFamilyDescriptor::new("account_state", state_cf_opts),
    ]
}
```

### Error Handling

Implement a comprehensive error handling strategy:

```rust
#[derive(Debug)]
pub enum StorageError {
    /// IO error
    IO(std::io::Error),
    /// Database error
    Database(rocksdb::Error),
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

impl From<rocksdb::Error> for StorageError {
    // Implementation
}

impl From<bincode::Error> for StorageError {
    // Implementation
}
```

## Recovery and Data Integrity

### Crash Recovery

RocksDB provides robust crash recovery:

```rust
// To ensure WAL (Write-Ahead Log) is enabled for crash recovery
let mut opts = rocksdb::Options::default();
opts.create_if_missing(true);
opts.create_missing_column_families(true);
opts.set_wal_size_limit_mb(512);
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

RocksDB provides native backup capabilities:

```rust
pub fn backup_database(db: &rocksdb::DB, backup_path: &str) -> Result<(), StorageError> {
    // Create a backup engine
    let backup_engine = rocksdb::backup::BackupEngine::open(
        &rocksdb::backup::BackupEngineOptions::default(),
        &backup_path
    )?;
  
    // Create a new backup
    backup_engine.create_new_backup(db)?;
  
    // Optionally purge old backups
    backup_engine.purge_old_backups(5)?; // Keep only the 5 most recent backups
  
    Ok(())
}
```

### Database Migration

If schema changes are needed, implement a migration system:

```rust
pub fn check_and_migrate_database(db: &rocksdb::DB, current_version: u32) -> Result<u32, StorageError> {
    // Similar to previous implementation, but using RocksDB APIs
    // ...
}
```

## Conclusion

This database design guide provides a comprehensive approach to implementing the storage layer for Blocana using RocksDB. Following these guidelines will result in a high-performance, scalable storage solution that can handle the demands of a blockchain environment while providing excellent performance characteristics.

RocksDB's mature feature set, proven track record in blockchain systems, and excellent performance make it an ideal choice for Blocana's storage needs. When implementing this design, remember to take advantage of RocksDB's column families, tuning options, and advanced features like bloom filters and flexible compression to achieve optimal performance.

--- End of Document ---
