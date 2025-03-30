# Transaction Pool API Guide

This document provides detailed guidance on the Transaction Pool API's consistent usage patterns, error handling, and state management.

## API Design Principles

The Transaction Pool API follows these core principles:

1. **Consistency**: All methods follow the same patterns for parameters and return values
2. **Explicit State Management**: Methods clearly indicate when they modify state
3. **Comprehensive Error Handling**: All errors are propagated with context
4. **Memory Safety**: Resource usage is carefully tracked and controlled
5. **Thread Safety**: API is designed for potential concurrent usage

## Core API Patterns

### Error Handling Pattern

Methods that can fail return a `Result<T, Error>` where:

- Success returns a meaningful value (e.g., transaction hash, count of operations)
- Errors use the global `Error` enum with appropriate variants
- Error messages include context about what operation failed and why

Example:

```rust
// Good error handling pattern:
fn add_transaction(&mut self, tx: Transaction, state: &mut BlockchainState) -> Result<Hash, Error> {
    if self.txs.contains_key(&tx_hash) {
        return Err(Error::Validation("Transaction already in pool".into()));
    }
    // ...process transaction...
    Ok(tx_hash)
}
```

### State Management Pattern

Methods that need to check state:
- Take `state: &BlockchainState` when they only read state
- Take `state: &mut BlockchainState` when they need to validate against current state

Methods that update state:
- Clearly document which state changes they make
- Return errors rather than panicking when state constraints are violated

When state changes affect transaction validity:
- Call `revalidate_transactions()` to update transaction validity statuses

### Resource Management Pattern

Methods that change pool contents:
- Update memory usage tracking with added/removed sizes
- Check against configured limits before accepting new transactions
- Trigger optimization when approaching resource limits

## Common API Usage Patterns

### Initializing the Pool

```rust
// Basic initialization
let pool = TransactionPool::new();

// Custom configuration
let config = TransactionPoolConfig {
    max_size: 10_000,
    max_memory: 64 * 1024 * 1024, // 64MB
    expiry_time: 7200, // 2 hours
    min_fee_per_byte: 2,
};
let pool = TransactionPool::with_config(config);
```

### Adding Transactions

```rust
// Adding a single transaction
match pool.add_transaction(transaction, &mut state) {
    Ok(hash) => {
        log::info!("Added transaction: {}", hex::encode(&hash[0..8]));
    },
    Err(e) => {
        log::warn!("Failed to add transaction: {}", e);
        // Handle specific error types if needed
        match e {
            Error::Validation(_) => { /* Handle validation errors */ },
            Error::Crypto(_) => { /* Handle cryptographic errors */ },
            _ => { /* Handle other errors */ }
        }
    }
}

// Adding multiple transactions as a batch
let (successful, failed) = pool.add_transactions_batch(transactions, &mut state);
log::info!("Added {}/{} transactions successfully", 
    successful.len(), successful.len() + failed.len());

for (hash, error) in failed {
    log::debug!("Failed to add tx {}: {}", hex::encode(&hash[0..8]), error);
}
```

### Selecting Transactions for a Block

```rust
// Get transactions for a new block
let max_block_size = 1_000_000; // 1MB block size limit
let max_tx_count = 10_000;

// Get the actual blockchain state
let mut state = blockchain.get_current_state();

// Select transactions based on priority
let selected_txs = pool.select_transactions(max_tx_count, &mut state);

// Calculate total size of selected transactions
let total_size: usize = selected_txs.iter()
    .map(|tx| tx.estimate_size())
    .sum();

if total_size > max_block_size {
    // Too large, need to reduce
    // This is simplified; you'd implement a more sophisticated size-based selection
    let mut current_size = 0;
    let truncated_txs: Vec<_> = selected_txs.into_iter()
        .take_while(|tx| {
            let tx_size = tx.estimate_size();
            if current_size + tx_size <= max_block_size {
                current_size += tx_size;
                true
            } else {
                false
            }
        })
        .collect();
    
    log::info!("Selected {} transactions totaling {} bytes", 
        truncated_txs.len(), current_size);
}
```

### Managing Pool Lifecycle

```rust
// Regular maintenance cycle
fn perform_regular_maintenance(pool: &mut TransactionPool, blockchain: &Blockchain) {
    // Get current state to revalidate against
    let state = blockchain.get_current_state();
    
    // Update transaction validity based on current state
    pool.revalidate_transactions(&state);
    
    // Remove expired transactions
    let expired_count = pool.remove_expired();
    if expired_count > 0 {
        log::info!("Removed {} expired transactions", expired_count);
    }
    
    // Optimize memory usage if needed
    let optimized_count = pool.optimize_memory();
    if optimized_count > 0 {
        log::info!("Optimized {} transactions to reduce memory usage", optimized_count);
    }
    
    // Complete maintenance
    let maintenance_count = pool.perform_maintenance();
    if maintenance_count > 0 {
        log::info!("Maintenance removed {} transactions", maintenance_count);
    }
    
    // Log pool status
    let stats = pool.stats();
    log::info!("Pool status: {} txs, {} senders, {} bytes, avg fee: {}", 
        stats.transaction_count, stats.unique_sender_count,
        stats.memory_usage, stats.avg_fee);
}
```

### Post-Block Processing

```rust
// Update pool after a block is added to the chain
fn process_new_block(pool: &mut TransactionPool, block: &Block, state: &BlockchainState) {
    // Remove transactions that were included in the block
    for tx in &block.transactions {
        // Either remove specific transactions
        pool.remove_transaction(&tx.hash());
        
        // Or remove all transactions for a sender with nonce >= the one in the block
        pool.remove_transactions_from_sender(&tx.sender, tx.nonce);
    }
    
    // Revalidate remaining transactions against new state
    pool.revalidate_transactions(state);
    
    // Perform maintenance to clean up internal structures
    pool.perform_maintenance();
}
```

### Handling Network Received Transactions

```rust
// Process transactions received from the network
fn handle_network_transactions(
    pool: &mut TransactionPool, 
    serialized_txs: Vec<Vec<u8>>,
    state: &mut BlockchainState
) {
    log::debug!("Received {} transactions from network", serialized_txs.len());
    
    // Deserialize and process transactions
    let mut valid_txs = Vec::new();
    let mut failed_count = 0;
    
    for tx_data in serialized_txs {
        // Deserialize transaction
        match bincode::deserialize::<Transaction>(&tx_data) {
            Ok(tx) => valid_txs.push(tx),
            Err(e) => {
                log::debug!("Failed to deserialize transaction: {}", e);
                failed_count += 1;
                continue;
            }
        }
    }
    
    // Add valid transactions in a batch
    let (successful, failed) = pool.add_transactions_batch(valid_txs, state);
    
    log::info!(
        "Network transactions processed: {}/{} successful, {} deserialization failures",
        successful.len(), 
        successful.len() + failed.len(), 
        failed_count
    );
}
```

## Common Error Scenarios and Handling

### Transaction Already in Pool

```rust
match pool.add_transaction(tx, &mut state) {
    Ok(_) => println!("Added successfully"),
    Err(e) => {
        if let Error::Validation(msg) = &e {
            if msg.contains("already in pool") {
                // Duplicate transaction, not an error worth reporting
                return;
            }
        }
        // Log other errors
        log::warn!("Failed to add transaction: {}", e);
    }
}
```

### Pool Memory or Size Limits Reached

```rust
match pool.add_transaction(tx, &mut state) {
    Ok(_) => println!("Added successfully"),
    Err(e) => {
        if let Error::Validation(msg) = &e {
            if msg.contains("memory limit") || msg.contains("pool full") {
                // Perform maintenance and try again
                let removed = pool.optimize_memory();
                log::info!("Pool optimization removed {} transactions", removed);
                
                // Try again if optimization freed up space
                if removed > 0 {
                    return pool.add_transaction(tx, &mut state);
                }
                
                // Otherwise, tell the sender to try again later
                return Err(Error::Temporary("Pool is full, try again later".into()));
            }
        }
        // Other errors
        Err(e)
    }
}
```

### Invalid Transaction Nonce

```rust
match pool.add_transaction(tx, &mut state) {
    Ok(_) => println!("Added successfully"),
    Err(e) => {
        if let Error::Validation(msg) = &e {
            if msg.contains("Invalid nonce") {
                if msg.contains("expected") && msg.contains("got") {
                    // Parse the expected nonce from error message
                    // This is a simplification - better to extract from error if API supports it
                    let parts: Vec<_> = msg.split_whitespace().collect();
                    if parts.len() > 2 {
                        let expected_nonce = parts[2].trim_matches(',');
                        log::warn!("Transaction has incorrect nonce. Expected: {}", expected_nonce);
                    }
                }
            }
        }
        Err(e)
    }
}
```

## Performance Optimization Patterns

### Batch Processing Multiple Transactions

```rust
// Instead of:
for tx in transactions {
    pool.add_transaction(tx, &mut state)?;
}

// Do this:
let (successful, failed) = pool.add_transactions_batch(transactions, &mut state);
```

### Minimizing State Access

```rust
// Poor performance (lots of state lookups):
for tx in &transactions {
    if pool.contains(&tx.hash()) {
        continue;
    }
    pool.add_transaction(tx.clone(), &mut state)?;
}

// Better (single batch operation):
let (successful, failed) = pool.add_transactions_batch(transactions, &mut state);
```

### Scheduled Maintenance

```rust
// Create a timer to run periodic maintenance
let timer = std::time::Instant::now();
let maintenance_interval = std::time::Duration::from_secs(60); // 1 minute

// In your main processing loop:
if timer.elapsed() >= maintenance_interval {
    pool.perform_maintenance();
    timer = std::time::Instant::now(); // Reset timer
}
```

## Thread Safety Considerations

The transaction pool is designed to be used safely in a multi-threaded environment with proper synchronization:

```rust
use std::sync::{Arc, Mutex};

// Thread-safe pool
let pool = Arc::new(Mutex::new(TransactionPool::new()));

// In thread 1: Add transactions
{
    let mut pool_guard = pool.lock().unwrap();
    pool_guard.add_transaction(tx1, &mut state)?;
}

// In thread 2: Select transactions
{
    let mut pool_guard = pool.lock().unwrap();
    let selected = pool_guard.select_transactions(100, &mut state);
}
```

## Memory Management Best Practices

### Monitoring Pool Memory Usage

```rust
// Log memory usage periodically
let memory_usage = pool.memory_usage();
let tx_count = pool.len();

log::info!(
    "Pool memory: {:.2} MB for {} transactions ({:.2} KB/tx average)",
    memory_usage as f64 / 1_048_576.0,
    tx_count,
    if tx_count > 0 { memory_usage as f64 / tx_count as f64 / 1024.0 } else { 0.0 }
);

// Alert if approaching limits
if memory_usage > pool.config().max_memory * 90 / 100 {
    log::warn!("Transaction pool memory usage above 90%");
}
```

### Custom Configuration for Resource Constraints

```rust
// For low-memory environments (e.g., IoT devices)
let config = TransactionPoolConfig {
    max_memory: 8 * 1024 * 1024, // 8MB
    max_size: 1000,
    expiry_time: 1800, // 30 minutes
    min_fee_per_byte: 2, // Higher minimum fee to control growth
};

// For high-throughput validators
let config = TransactionPoolConfig {
    max_memory: 1024 * 1024 * 1024, // 1GB
    max_size: 100000,
    expiry_time: 7200, // 2 hours
    min_fee_per_byte: 1,
};
```

## Conclusion

Following these API usage patterns ensures consistent, efficient, and error-resistant use of the transaction pool. These patterns promote a uniform approach to transaction management throughout the codebase, making it easier to maintain and extend the system's functionality.

For more detailed information about specific methods, refer to the [Transaction Pool API Reference](../api/transaction-pool.md).

--- End of Document ---
