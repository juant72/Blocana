//! Blocana: A lightweight, high-performance blockchain optimized for resource-constrained environments
//! 
//! This library implements the core components of the Blocana blockchain.

#![allow(dead_code)]

// First, declare the types module which other modules will depend on
pub mod types;

// Now declare the remaining modules
pub mod crypto;
pub mod block;
pub mod transaction;
pub mod state;
pub mod consensus;
pub mod network;
pub mod storage;
pub mod vm;

// Re-exports of the most commonly used types
pub use types::{Hash, PublicKeyBytes, PrivateKeyBytes, SignatureBytes};
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
    pub network_id: u64,
    /// Maximum size of a block in bytes
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
            max_block_size: 1_000_000,
            target_block_time_ms: 500,
            max_txs_per_block: 1000,
            storage_config: storage::StorageConfig::default(),
            network_config: network::NetworkConfig::default(),
            consensus_config: consensus::ConsensusConfig::default(),
        }
    }
}

/// Main blockchain instance
pub struct Blockchain {
    /// Blockchain configuration
    pub config: BlockchainConfig,
}

impl Blockchain {
    /// Create a new blockchain instance
    pub fn new(config: BlockchainConfig) -> Result<Self, Error> {
        Ok(Self { config })
    }
}

/// Utility function to format byte arrays as hex strings
pub fn hex_fmt(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Error types for Blocana operations
#[derive(Debug)]
pub enum Error {
    /// IO error
    IO(std::io::Error),
    /// Cryptographic error
    Crypto(String),
    /// Database error
    DB(String),
    /// Validation error
    Validation(String),
    /// Network error
    Network(String),
    /// Configuration error
    Config(String),
    /// Serialization error
    Serialization(String),
    /// Consensus error
    Consensus(String),
    /// Other error type
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IO(e) => write!(f, "IO error: {}", e),
            Error::Crypto(s) => write!(f, "Crypto error: {}", s),
            Error::DB(s) => write!(f, "Database error: {}", s),
            Error::Validation(s) => write!(f, "Validation error: {}", s),
            Error::Network(s) => write!(f, "Network error: {}", s),
            Error::Config(s) => write!(f, "Configuration error: {}", s),
            Error::Serialization(s) => write!(f, "Serialization error: {}", s),
            Error::Consensus(s) => write!(f, "Consensus error: {}", s),
            Error::Other(s) => write!(f, "Other error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

// Implement From traits for error conversion
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl From<storage::Error> for Error {
    fn from(err: storage::Error) -> Self {
        match err {
            storage::Error::IO(e) => Error::IO(e),
            storage::Error::Database(s) => Error::DB(s),
            storage::Error::Serialization(s) => Error::Serialization(s),
            storage::Error::Other(s) => Error::Other(s),
        }
    }
}

impl From<network::Error> for Error {
    fn from(err: network::Error) -> Self {
        Error::Network(format!("{:?}", err))
    }
}

impl From<consensus::Error> for Error {
    fn from(err: consensus::Error) -> Self {
        Error::Consensus(format!("{:?}", err))
    }
}

impl From<vm::Error> for Error {
    fn from(err: vm::Error) -> Self {
        Error::Other(format!("{:?}", err))
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Serialization(format!("{}", err))
    }
}

/// Define placeholder modules for those not yet implemented

/// Mock consensus module
pub mod consensus {
    #[derive(Debug)]
    pub enum Error {}

    #[derive(Debug, Default, Clone)]
    pub struct ConsensusConfig {}
    
    pub trait Consensus {}

    pub struct PoETConsensus {}
    impl Consensus for PoETConsensus {}
}

/// Mock VM module for compilation
pub mod vm {
    #[derive(Debug)]
    pub enum Error {
        InvalidBytecode(String),
        ExecutionError(String),
        ResourceLimitExceeded(String),
        Other(String),
    }

    pub struct VirtualMachine;

    impl VirtualMachine {
        pub fn new() -> Result<Self, Error> {
            Ok(Self {})
        }

        pub fn execute(&self, _bytecode: &[u8], _function: &str, _args: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::Other("VM not implemented".into()))
        }
    }
}
