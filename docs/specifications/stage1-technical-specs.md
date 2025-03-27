# Stage 1 Technical Specifications

This document provides the detailed technical specifications for Blocana Stage 1 components, serving as the definitive reference for implementation.

## Data Structures

### Block Structure

```rust
struct BlockHeader {
    version: u8,            // Block version number (currently 1)
    prev_hash: [u8; 32],    // SHA-256 hash of the previous block header
    merkle_root: [u8; 32],  // Merkle root of the transactions
    timestamp: u64,         // Time of block creation (milliseconds since Unix epoch)
    height: u64,            // Block height (genesis = 0)
    validator: [u8; 32],    // Public key of the block creator/validator
    signature: [u8; 64],    // Ed25519 signature of the header data
}

struct Block {
    header: BlockHeader,    // The block header
    transactions: Vec<Transaction>, // List of included transactions
}
```

### Transaction Structure

```rust
struct Transaction {
    version: u8,            // Transaction version (currently 1)
    sender: [u8; 32],       // Public key of the sender
    recipient: [u8; 32],    // Public key of the recipient
    amount: u64,            // Amount to transfer
    fee: u64,               // Fee to pay validator
    nonce: u64,             // Sender's transaction counter
    data: Vec<u8>,          // Optional data payload
    signature: [u8; 64],    // Ed25519 signature
}
```

## Cryptographic Standards

### Hash Function
- **Algorithm**: SHA-256
- **Input**: Arbitrary byte array
- **Output**: 32-byte hash
- **Library**: Use `sha2` crate version 0.10.6 or higher

### Digital Signatures
- **Algorithm**: Ed25519
- **Key Size**: 32-byte public keys, 32-byte private keys
- **Signature Size**: 64 bytes
- **Library**: Use `ed25519-dalek` crate version 1.0.1 or higher

## Data Serialization
- **Block Serialization Format**: Binary, with fields in order as defined in struct
- **Transaction Serialization Format**: Binary
- **Endianness**: Little-endian for multi-byte integer types
- **Chain Metadata**: Binary for storage, JSON for export/API

## Storage Design

### RocksDB Configuration
- **Column Families**: 
  - `blocks`: Maps block hash → block data
  - `block_height`: Maps height → block hash
  - `state`: Maps address → account state
  - `transactions`: Maps transaction hash → transaction data
- **Key Format**: Raw byte arrays
- **Value Format**: Compressed binary serialization
- **Performance Tuning**: 
  - Configure bloom filters for transaction lookups
  - Set appropriate cache sizes for blocks column family
  - Use appropriate compression (e.g., LZ4, Zstd) based on data type

### File Structure
- Base path: Configurable, default `"data"`
- Database files: `data/CURRENT`, `data/LOCK`, `data/LOG`, etc.
- Column family files: `data/[cf_name]` directories

## Merkle Tree Implementation
- **Algorithm**: Binary Merkle tree with SHA-256
- **Leaf Calculation**: Hash of transaction data
- **Node Calculation**: Hash of concatenated child hashes
- **Empty Tree Value**: Zero-filled 32-byte array
- **Handling Odd Nodes**: Duplicate the last node

## Performance Requirements
- **Block Generation Time**: < 10ms on reference hardware
- **Transaction Validation**: < 1ms per transaction
- **Storage Requirements**: < 100MB for first 10,000 blocks
- **Memory Usage**: < 50MB working set

## Error Handling Standards
- **Error Types**: Use enum with specific error variants
- **Propagation**: Use Result type with ? operator
- **Logging**: Log errors with context at appropriate levels
- **Error Categories**:
  - Cryptographic Errors (invalid signatures, hash mismatches)
  - Storage Errors (I/O failures, database errors)
  - Validation Errors (invalid blocks, transactions)
  - State Errors (inconsistent state)
  - Configuration Errors (invalid settings)
- **Error Context**: Include relevant state information with errors
- **Recovery Strategy**: Define recovery paths for non-fatal errors

## Account State Model

### Account Structure
```rust
struct AccountState {
    balance: u64,           // Current account balance
    nonce: u64,             // Transaction counter for replay protection
    code: Option<Vec<u8>>,  // Optional smart contract code (for future use)
    storage: HashMap<[u8;32], Vec<u8>>, // Account storage (for future use)
}
```

### State Transitions
- **Transaction Application**: Atomic changes to sender and recipient states
- **Validation Rules**: Balance sufficient, nonce sequential, signatures valid
- **Genesis State**: Initial account states defined in genesis block

## Genesis Block Configuration

### Genesis Parameters
- **Timestamp**: Fixed value for genesis (network launch time)
- **Initial Accounts**: Pre-funded accounts and balances
- **Network Parameters**: Initial difficulty, block time target
- **Validation Rules**: Special validation for genesis block (no previous hash)

### Genesis Creation
- Genesis block must be hardcoded or generated deterministically
- All nodes must agree on identical genesis block
- Genesis hash serves as network identifier

--- End of Document ---
