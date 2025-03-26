//! Block structures and functionality for the Blocana blockchain
//!
//! This module contains the core block structures and related functionality.

use crate::transaction::Transaction;
use std::time::{SystemTime, UNIX_EPOCH};

/// Hash type used throughout the blockchain
pub type Hash = [u8; 32];

/// Block header containing metadata
#[derive(Clone, Debug)]
pub struct BlockHeader {
    /// Protocol version
    pub version: u8,
    /// Previous block hash
    pub prev_hash: Hash,
    /// Merkle root of transactions
    pub merkle_root: Hash,
    /// Block timestamp (ms since UNIX epoch)
    pub timestamp: u64,
    /// Block height
    pub height: u64,
    /// Validator public key
    pub validator: [u8; 32],
    /// Validator signature
    pub signature: [u8; 64],
}

impl BlockHeader {
    /// Create a new block header
    pub fn new(
        version: u8,
        prev_hash: Hash,
        merkle_root: Hash,
        height: u64,
        validator: [u8; 32],
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        Self {
            version,
            prev_hash,
            merkle_root,
            timestamp,
            height,
            validator,
            signature: [0u8; 64],
        }
    }
    
    /// Sign the block header with the given private key
    pub fn sign(&mut self, private_key: &[u8; 32]) -> Result<(), crate::Error> {
        // Implement signing logic here
        // This is just a placeholder
        self.signature = [1u8; 64];
        Ok(())
    }
    
    /// Compute the hash of this block header
    pub fn hash(&self) -> Hash {
        // Implement efficient hashing
        // For simplicity, this is just a placeholder
        [0u8; 32]
    }
}

/// A full block in the Blocana blockchain
#[derive(Clone, Debug)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    /// Block transactions
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Create a new block with the given transactions
    pub fn new(
        prev_hash: Hash,
        height: u64,
        transactions: Vec<Transaction>,
        validator: [u8; 32],
    ) -> Result<Self, crate::Error> {
        // Compute Merkle root from transactions
        let merkle_root = compute_merkle_root(&transactions)?;
        
        let header = BlockHeader::new(
            1, // Current version
            prev_hash,
            merkle_root,
            height,
            validator,
        );
        
        Ok(Self {
            header,
            transactions,
        })
    }
    
    /// Validate the block structure and signatures
    pub fn validate(&self) -> Result<(), crate::Error> {
        // Verify merkle root matches transactions
        let computed_root = compute_merkle_root(&self.transactions)?;
        if computed_root != self.header.merkle_root {
            return Err(crate::Error::Validation("Invalid merkle root".into()));
        }
        
        // Verify validator signature
        // (Implementation would go here)
        
        Ok(())
    }
    
    /// Get the serialized size of this block in bytes
    pub fn serialized_size(&self) -> usize {
        // Header size (fixed)
        let header_size = std::mem::size_of::<BlockHeader>();
        
        // Transaction sizes (variable)
        let txs_size: usize = self.transactions.iter()
            .map(|tx| tx.serialized_size())
            .sum();
        
        header_size + txs_size
    }
}

/// Compute the Merkle root from a list of transactions
fn compute_merkle_root(transactions: &[Transaction]) -> Result<Hash, crate::Error> {
    if transactions.is_empty() {
        return Ok([0u8; 32]); // Empty Merkle root
    }
    
    // Get transaction hashes
    let mut hashes: Vec<Hash> = transactions.iter()
        .map(|tx| tx.hash())
        .collect();
    
    // Compute Merkle tree (simplified implementation)
    while hashes.len() > 1 {
        if hashes.len() % 2 != 0 {
            hashes.push(hashes.last().unwrap().clone());
        }
        
        let mut new_hashes = Vec::with_capacity(hashes.len() / 2);
        for i in (0..hashes.len()).step_by(2) {
            new_hashes.push(hash_pair(&hashes[i], &hashes[i+1]));
        }
        
        hashes = new_hashes;
    }
    
    Ok(hashes[0])
}

/// Hash a pair of child hashes to create a parent hash
fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    // Implement efficient hashing
    // This is just a placeholder
    [0u8; 32]
}
