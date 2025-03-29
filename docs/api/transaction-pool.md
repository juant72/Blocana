# Transaction Pool API Documentation

This document describes the Transaction Pool API for Blocana, part of Milestone 4: Transaction Management.

## Introduction

The Transaction Pool is responsible for managing unconfirmed transactions that are waiting to be included in blocks. It handles transaction validation, prioritization, and selection for block creation.

## API Reference

### Creating a Transaction Pool

```rust
// Create with default configuration
let pool = TransactionPool::new();

// Create with custom configuration
let config = TransactionPoolConfig {
    max_size: 5000,
    expiry_time: 3600, // 1 hour in seconds
    max_memory: 32 * 1024 * 1024, // 32 MB
    min_fee_per_byte: 1, // Minimum fee per byte for acceptance
};
let pool = TransactionPool::with_config(config);
```

### Adding Transactions

```rust
// Add a transaction to the pool
let result = pool.add_transaction(transaction, &blockchain_state)?;
if result {
    println!("Transaction added to pool");
} else {
    println!("Transaction rejected by pool");
}
```

### Transaction Selection

```rust
// Select up to 100 transactions for a block, in order of priority
let transactions = pool.select_transactions(100, &blockchain_state);
```

### Managing Transactions

```rust
// Check if a transaction is in the pool
if pool.contains(&tx_hash) {
    println!("Transaction is in the pool");
}

// Get a specific transaction
if let Some(tx) = pool.get_transaction(&tx_hash) {
    println!("Retrieved transaction: {:?}", tx);
}

// Get all transactions from a specific sender
let sender_txs = pool.get_transactions_from_sender(&sender_address);

// Remove a transaction
pool.remove_transaction(&tx_hash);

// Remove transactions after they're included in a block
// (removes all transactions with nonce ≥ the given nonce)
pool.remove_transactions_from_sender(&sender_address, nonce);

// Remove all expired transactions
let removed_count = pool.remove_expired();
```

### Pool Information

```rust
// Get the number of transactions in the pool
let size = pool.size();

// Get current memory usage
let memory_bytes = pool.memory_usage();

// Get detailed statistics
let stats = pool.stats();
println!("Transaction count: {}", stats.transaction_count);
println!("Memory usage: {} bytes", stats.memory_usage);
println!("Unique senders: {}", stats.unique_sender_count);
println!("Total fees: {}", stats.total_fees);
println!("Avg fee: {}", stats.avg_fee);
```

## Transaction Pool Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `max_size` | Maximum number of transactions in the pool | 5000 |
| `expiry_time` | Maximum transaction age in seconds | 3600 (1 hour) |
| `max_memory` | Maximum memory usage in bytes | 32MB |
| `min_fee_per_byte` | Minimum fee per byte for acceptance | 1 |

## Transaction Prioritization

Transactions in the pool are prioritized based on:

1. **Fee per byte** (primary): Higher fee-per-byte transactions have higher priority
2. **Total fee** (secondary): For transactions with the same fee-per-byte, higher total fee has priority
3. **Transaction hash** (tie-breaker): For deterministic ordering when fees are identical

## Error Handling

All transaction pool methods return a `Result` type that provides detailed error information on failure.
Common errors include:

- Invalid transaction signatures
- Incorrect nonces 
- Insufficient sender balance
- Pool memory limitations

