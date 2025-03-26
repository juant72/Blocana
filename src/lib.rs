//! Blocana: A lightweight, high-performance blockchain optimized for resource-constrained environments
//! 
//! This library implements the core components of the Blocana blockchain.

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
    
    // Additional methods would be implemented here
}

/// Error types for Blocana operations
#[derive(Debug)]
pub enum Error {
    Storage(storage::Error),
    Network(network::Error),
    Consensus(consensus::Error),
    Validation(String),
    Configuration(String),
}

// Implement From traits for error conversion
// ...
