//! Blocana: A lightweight, high-performance blockchain optimized for resource-constrained environments
//! 
//! This library implements the core components of the Blocana blockchain.

// Añadir esta línea al principio para suprimir advertencias de código muerto
#![allow(dead_code)]

pub mod block;
pub mod consensus;
pub mod network;
pub mod storage;
pub mod transaction;
pub mod vm;

/// Re-exports of the most commonly used types
pub use block::{Block, BlockHeader};
pub use consensus::{Consensus, PoETConsensus};
pub use network::{Node, NodeConfig};
pub use storage::{BlockStore, StateStore};
pub use transaction::{Transaction, TransactionVerifier};

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Configuration for the Blocana blockchain
#[derive(Debug, Clone)]
pub struct BlockchainConfig {
    /// Network identifier
    pub network_id: u8,
    /// Maximum block size in bytes
    pub max_block_size: usize,
    /// Target block time in milliseconds
    pub target_block_time_ms: u64,
    /// Maximum number of transactions per block
    pub max_txs_per_block: usize,
    /// Storage configuration
    pub storage_config: storage::StorageConfig,
    /// Network configuration
    pub network_config: network::NetworkConfig,
    /// Consensus configuration
    pub consensus_config: consensus::ConsensusConfig,
}

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            network_id: 1,
            max_block_size: 1024,  // 1KB max block size
            target_block_time_ms: 500, // 500ms target block time
            max_txs_per_block: 100,
            storage_config: storage::StorageConfig::default(),
            network_config: network::NetworkConfig::default(),
            consensus_config: consensus::ConsensusConfig::default(),
        }
    }
}

/// Main blockchain instance
pub struct Blockchain {
    config: BlockchainConfig,
    consensus: Box<dyn Consensus>,
    storage: storage::BlockchainStorage,
    network: network::NetworkManager,
}

impl Blockchain {
    /// Create a new blockchain instance with the given configuration
    pub fn new(config: BlockchainConfig) -> Result<Self, Error> {
        // Initialize storage
        let storage = storage::BlockchainStorage::new(&config.storage_config)?;
        
        // Initialize network
        let network = network::NetworkManager::new(&config.network_config)?;
        
        // Initialize consensus
        let consensus: Box<dyn Consensus> = match config.consensus_config.algorithm {
            consensus::ConsensusAlgorithm::PoET => Box::new(PoETConsensus::new(&config.consensus_config)?),
            // Other consensus mechanisms can be added here
        };
        
        Ok(Self {
            config,
            consensus,
            storage,
            network,
        })
    }
    
    /// Start the blockchain node
    pub fn start(&mut self) -> Result<(), Error> {
        // Start network services
        self.network.start()?;
        
        // Initialize consensus mechanism
        self.consensus.initialize(&self.storage)?;
        
        // Start consensus process
        self.consensus.start()?;
        
        Ok(())
    }
    
    /// Generate a new block with pending transactions
    pub fn generate_block(&mut self) -> Result<Block, Error> {
        // Get the latest block hash (using a placeholder for now)
        let previous_hash = [0u8; 32]; // In a real implementation, we'd get this from storage
        let height = 1; // In a real implementation, we'd get the current height
        
        // Collect pending transactions (using empty vec for now)
        let transactions = Vec::new(); // In a real implementation, we'd get pending transactions
        
        // Use the consensus mechanism to generate a block
        let block = self.consensus.generate_block(transactions, previous_hash, height)?;
        
        // Store the block
        self.storage.store_block(&block)?;
        
        // Broadcast the block to peers (not implemented yet)
        println!("Generated block at height {} with {} transactions", 
                 height, block.transactions.len());
        
        Ok(block)
    }
    
    /// Create a new transaction
    pub fn create_transaction(&mut self, recipient: [u8; 32], amount: u64) -> Result<(), Error> {
        // Create dummy sender (in a real app, this would be the user's wallet)
        let sender = [1u8; 32];
        let fee = 1; // Minimal fee
        let nonce = 1; // In a real app, we'd track nonces per account
        
        // Create the transaction
        let mut tx = Transaction::new(
            sender,
            recipient,
            amount,
            fee,
            nonce,
            Vec::new(), // No data payload
        );
        
        // Sign the transaction (not really implemented)
        let dummy_private_key = [0u8; 32];
        tx.sign(&dummy_private_key)?;
        
        // Add to pending transactions (not implemented yet)
        println!("Created transaction: {} → {} (amount: {})", 
                 hex_fmt(&sender), hex_fmt(&recipient), amount);
        
        Ok(())
    }
    
    /// Print blockchain status
    pub fn print_status(&self) {
        println!("Blockchain Status:");
        println!("  Network ID: {}", self.config.network_id);
        println!("  Max block size: {} bytes", self.config.max_block_size);
        println!("  Target block time: {}ms", self.config.target_block_time_ms);
        // In a real implementation, we'd print current height, pending txs count, etc.
    }
    
    /// Print connected peers
    pub fn print_peers(&self) {
        println!("Connected Peers:");
        println!("  (None - P2P networking not fully implemented yet)");
        // In a real implementation, we'd print the list of connected peers
    }
}

/// Utility function to format byte arrays as hex strings
fn hex_fmt(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes.iter().take(4) {
        s.push_str(&format!("{:02x}", b));
    }
    s.push_str("..."); // Truncate for readability
    s
}

/// Error types for Blocana operations
#[derive(Debug)]
pub enum Error {
    Storage(storage::Error),
    Network(network::Error),
    Consensus(consensus::Error),
    VM(vm::Error),
    Validation(String),
    Configuration(String),
}

// Implement From traits for error conversion
impl From<storage::Error> for Error {
    fn from(err: storage::Error) -> Self {
        Error::Storage(err)
    }
}

impl From<network::Error> for Error {
    fn from(err: network::Error) -> Self {
        Error::Network(err)
    }
}

impl From<consensus::Error> for Error {
    fn from(err: consensus::Error) -> Self {
        Error::Consensus(err)
    }
}

impl From<vm::Error> for Error {
    fn from(err: vm::Error) -> Self {
        Error::VM(err)
    }
}
