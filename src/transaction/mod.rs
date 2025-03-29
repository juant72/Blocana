//! Transaction functionality for the Blocana blockchain
//!
//! This module contains transaction structures and related functionality.

pub mod pool;

use serde_big_array::BigArray;
pub use pool::{TransactionPool, TransactionPoolConfig};
use crate::types::{Hash, PublicKeyBytes, SignatureBytes};
use serde::{Serialize, Deserialize};

/// Transaction structure
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Transaction {
    /// Transaction version
    pub version: u8,
    /// Transaction sender
    pub sender: PublicKeyBytes,
    /// Transaction recipient
    pub recipient: PublicKeyBytes,
    /// Transaction amount
    pub amount: u64,
    /// Transaction fee
    pub fee: u64,
    /// Transaction nonce (to prevent replay attacks)
    pub nonce: u64,
    /// Transaction data (optional)
    pub data: Vec<u8>,
    /// Transaction signature
    #[serde(with = "BigArray")]
    pub signature: SignatureBytes,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(
        sender: PublicKeyBytes,
        recipient: PublicKeyBytes,
        amount: u64,
        fee: u64,
        nonce: u64,
        data: Vec<u8>,
    ) -> Self {
        Self {
            version: 1,
            sender,
            recipient,
            amount,
            fee,
            nonce,
            data,
            signature: [0u8; 64],
        }
    }
    
    /// Sign the transaction with the given private key
    pub fn sign(&mut self, private_key: &crate::types::PrivateKeyBytes) -> Result<(), crate::Error> {
        // Serialize the transaction data for signing (excluding signature)
        let bytes = self.serialize_for_signing();
        
        // Sign the transaction data
        let signature = crate::crypto::sign_message(private_key, &bytes)?;
        self.signature = signature;
        
        Ok(())
    }
    
    /// Verify the transaction signature
    pub fn verify(&self) -> Result<(), crate::Error> {
        // Get bytes that were signed
        let bytes = self.serialize_for_signing();
        
        // Verify the signature
        crate::crypto::verify_signature(&self.sender, &self.signature, &bytes)
    }
    
    /// Serialize transaction data for signing (excludes signature)
    pub fn serialize_for_signing(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            1 + // version
            32 + // sender
            32 + // recipient
            8 + // amount
            8 + // fee
            8 + // nonce
            self.data.len() // data
        );
        
        // Append fields in canonical order
        bytes.push(self.version);
        bytes.extend_from_slice(&self.sender);
        bytes.extend_from_slice(&self.recipient);
        bytes.extend_from_slice(&self.amount.to_le_bytes());
        bytes.extend_from_slice(&self.fee.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        
        bytes
    }
    
    /// Compute the hash of this transaction
    pub fn hash(&self) -> Hash {
        let bytes = self.serialize_for_signing();
        
        // Hash the transaction data
        crate::crypto::hash_data(&bytes)
    }
    
    /// Get the serialized size of this transaction in bytes
    pub fn serialized_size(&self) -> usize {
    // Encode to a vector and get its length
    match bincode::encode_to_vec(self, bincode::config::standard()) {
        Ok(vec) => vec.len(),
        Err(_) => 0
    }
    }
    
    /// Estimate the size of the transaction in bytes
    ///
    /// # Returns
    /// Estimated size in bytes
    pub fn estimate_size(&self) -> usize {
        // Basic size of the struct fields
        let fixed_size = 1 +                 // version (u8)
                         32 +                // sender (32 bytes)
                         32 +                // recipient (32 bytes)
                         8 +                 // amount (u64)
                         8 +                 // fee (u64)
                         8 +                 // nonce (u64)
                         64;                 // signature (64 bytes)
        
        // Add the size of the data field (plus the length prefix for Vec)
        fixed_size + 8 + self.data.len()
    }
}

/// Transaction verifier
pub struct TransactionVerifier;

impl TransactionVerifier {
    /// Verify a transaction
    pub fn verify(&self, transaction: &Transaction) -> Result<(), crate::Error> {
        transaction.verify()
    }
    
    /// Verify the transaction is valid against the current state
    pub fn verify_against_state(&self, 
                               transaction: &Transaction, 
                               account_state: &crate::state::AccountState) -> Result<(), crate::Error> {
        // Check that the nonce is correct
        if transaction.nonce != account_state.nonce {
            return Err(crate::Error::Validation(format!(
                "Invalid nonce: expected {}, got {}", 
                account_state.nonce, 
                transaction.nonce
            )));
        }
        
        // Check that the sender has enough balance
        let total_needed = transaction.amount.saturating_add(transaction.fee);
        if account_state.balance < total_needed {
            return Err(crate::Error::Validation(format!(
                "Insufficient balance: has {}, needs {}", 
                account_state.balance, 
                total_needed
            )));
        }
        
        // Signature is already verified by the standard verify method
        transaction.verify()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new(
            [1u8; 32], // sender
            [2u8; 32], // recipient
            100,       // amount
            10,        // fee
            5,         // nonce
            vec![1, 2, 3], // data
        );
        
        assert_eq!(tx.version, 1);
        assert_eq!(tx.sender, [1u8; 32]);
        assert_eq!(tx.recipient, [2u8; 32]);
        assert_eq!(tx.amount, 100);
        assert_eq!(tx.fee, 10);
        assert_eq!(tx.nonce, 5);
        assert_eq!(tx.data, vec![1, 2, 3]);
        assert_eq!(tx.signature, [0u8; 64]); // Default zero signature
    }
    
    #[test]
    fn test_transaction_hash() {
        let tx1 = Transaction::new(
            [1u8; 32],
            [2u8; 32],
            100,
            10,
            5,
            vec![1, 2, 3],
        );
        
        let tx2 = Transaction::new(
            [1u8; 32],
            [2u8; 32],
            100,
            10,
            6, // Different nonce
            vec![1, 2, 3],
        );
        
        let hash1 = tx1.hash();
        let hash2 = tx2.hash();
        
        // Hashes should not be all zeros
        assert_ne!(hash1, [0u8; 32]);
        // Different transactions should have different hashes
        assert_ne!(hash1, hash2);
    }
}
