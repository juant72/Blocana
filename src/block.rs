//! Block structures and functionality for the Blocana blockchain
//!
//! This module contains the core block structures and related functionality.

use crate::transaction::Transaction;
use crate::types::{Hash, PublicKeyBytes, SignatureBytes};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// Block header containing metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub validator: PublicKeyBytes,
    /// Validator signature
    pub signature: SignatureBytes,
}

impl BlockHeader {
    /// Create a new block header
    pub fn new(
        version: u8,
        prev_hash: Hash,
        merkle_root: Hash,
        height: u64,
        validator: PublicKeyBytes,
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
    pub fn sign(&mut self, private_key: &crate::types::PrivateKeyBytes) -> Result<(), crate::Error> {
        // Get bytes to sign (without the signature field)
        let bytes = self.serialize_for_signing();
        
        // Sign the data
        let signature = crate::crypto::sign_message(private_key, &bytes)?;
        self.signature = signature;
        
        Ok(())
    }
    
    /// Verify the block header signature
    pub fn verify_signature(&self) -> Result<(), crate::Error> {
        // Get the bytes that were signed (without signature)
        let bytes = self.serialize_for_signing();
        
        // Verify the signature
        crate::crypto::verify_signature(&self.validator, &self.signature, &bytes)
    }
    
    /// Serialize for hashing (excludes signature)
    pub fn serialize_for_hashing(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            1 + // version
            32 + // prev_hash
            32 + // merkle_root
            8 + // timestamp
            8 + // height
            32   // validator
        );
        
        // Append fields in canonical order
        bytes.push(self.version);
        bytes.extend_from_slice(&self.prev_hash);
        bytes.extend_from_slice(&self.merkle_root);
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.validator);
        
        bytes
    }
    
    /// Serialize for signing (same as hashing in this implementation)
    pub fn serialize_for_signing(&self) -> Vec<u8> {
        self.serialize_for_hashing()
    }
    
    /// Compute the hash of this block header
    pub fn hash(&self) -> Hash {
        crate::crypto::hash_data(&self.serialize_for_hashing())
    }
}

/// A full block in the Blocana blockchain
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        validator: PublicKeyBytes,
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
    
    /// Create a genesis block
    pub fn genesis(validator: PublicKeyBytes, initial_transactions: Vec<Transaction>) -> Result<Self, crate::Error> {
        // Genesis block has a zero prev_hash
        let prev_hash = [0u8; 32];
        
        // Create the block with height 0
        Self::new(prev_hash, 0, initial_transactions, validator)
    }
    
    /// Validate the block structure and signatures
    pub fn validate(&self) -> Result<(), crate::Error> {
        // Verify merkle root matches transactions
        let computed_root = compute_merkle_root(&self.transactions)?;
        if computed_root != self.header.merkle_root {
            return Err(crate::Error::Validation("Invalid merkle root".into()));
        }
        
        // Verify validator signature
        self.header.verify_signature()?;
        
        // Validate all transactions
        for tx in &self.transactions {
            tx.verify()?;
        }
        
        Ok(())
    }
    
    /// Get the serialized size of this block in bytes
    pub fn serialized_size(&self) -> usize {
        // Use bincode to estimate the serialized size
        bincode::serialized_size(&self)
            .unwrap_or(0) as usize
    }
}

/// Compute the Merkle root from a list of transactions
pub fn compute_merkle_root(transactions: &[Transaction]) -> Result<Hash, crate::Error> {
    if transactions.is_empty() {
        return Ok([0u8; 32]); // Empty Merkle root
    }
    
    // Get transaction hashes
    let hashes: Vec<Hash> = transactions.iter()
        .map(|tx| tx.hash())
        .collect();
    
    // Compute the Merkle root using the crypto module
    Ok(crate::crypto::compute_merkle_root(&hashes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::Transaction;
    
    fn create_test_transaction() -> Transaction {
        let mut tx = Transaction::new(
            [1u8; 32], // sender
            [2u8; 32], // recipient
            100,       // amount
            5,         // fee
            0,         // nonce
            vec![],    // data
        );
        
        // We don't need to properly sign for tests
        tx.signature = [3u8; 64];
        tx
    }
    
    #[test]
    fn test_block_creation() {
        let transactions = vec![create_test_transaction()];
        let validator = [5u8; 32];
        
        let block = Block::new(
            [0u8; 32],  // prev_hash
            1,          // height
            transactions,
            validator,
        );
        
        assert!(block.is_ok());
        let block = block.unwrap();
        
        assert_eq!(block.header.version, 1);
        assert_eq!(block.header.height, 1);
        assert_eq!(block.header.validator, validator);
        assert_eq!(block.transactions.len(), 1);
    }
    
    #[test]
    fn test_genesis_block() {
        let transactions = vec![create_test_transaction()];
        let validator = [5u8; 32];
        
        let genesis = Block::genesis(validator, transactions);
        
        assert!(genesis.is_ok());
        let genesis = genesis.unwrap();
        
        assert_eq!(genesis.header.version, 1);
        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.prev_hash, [0u8; 32]);
        assert_eq!(genesis.transactions.len(), 1);
    }
    
    #[test]
    fn test_block_hash() {
        let block = Block::new(
            [0u8; 32],
            1,
            vec![create_test_transaction()],
            [5u8; 32],
        ).unwrap();
        
        let hash = block.header.hash();
        
        // Hash should not be all zeros
        assert_ne!(hash, [0u8; 32]);
    }
}
