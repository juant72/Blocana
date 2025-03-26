//! Consensus mechanisms for Blocana blockchain
//!
//! This module contains the consensus interface and implementations.

mod poet;

pub use poet::PoETConsensus;
use crate::block::Block;
use crate::storage::BlockchainStorage;
use crate::transaction::Transaction;

/// Supported consensus algorithms
#[derive(Debug, Clone, Copy)]
pub enum ConsensusAlgorithm {
    /// Proof of Elapsed Time
    PoET,
}

/// Configuration for consensus mechanisms
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// The selected consensus algorithm
    pub algorithm: ConsensusAlgorithm,
    /// Target block time in milliseconds
    pub target_block_time_ms: u64,
    /// Maximum number of validators (if applicable)
    pub max_validators: u32,
    /// Minimum stake amount (if applicable)
    pub min_stake: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: ConsensusAlgorithm::PoET,
            target_block_time_ms: 500,
            max_validators: 100,
            min_stake: 1000,
        }
    }
}

/// Error types specific to consensus operations
#[derive(Debug)]
pub enum Error {
    /// Error during initialization
    Initialization(String),
    /// Error when creating a block
    BlockCreation(String),
    /// Error when validating a block
    BlockValidation(String),
    /// Error when signing a block
    BlockSigning(String),
    /// Consensus is already running
    AlreadyRunning,
    /// Consensus is not running
    NotRunning,
    /// Other errors
    Other(String),
}

/// Interface for consensus mechanisms
pub trait Consensus: Send + Sync {
    /// Initialize the consensus mechanism with the given storage
    fn initialize(&mut self, storage: &BlockchainStorage) -> Result<(), Error>;
    
    /// Start the consensus process
    fn start(&mut self) -> Result<(), Error>;
    
    /// Stop the consensus process
    fn stop(&mut self) -> Result<(), Error>;
    
    /// Generate a new block with the given transactions
    fn generate_block(&self, txs: Vec<Transaction>, previous_hash: [u8; 32], height: u64) -> Result<Block, Error>;
    
    /// Validate a block according to consensus rules
    fn validate_block(&self, block: &Block) -> Result<(), Error>;
    
    /// Check if consensus is currently running
    fn is_running(&self) -> bool;
    
    /// Check if this node should produce a block now
    fn should_produce_block(&self) -> bool;
}
