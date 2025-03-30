//! Transaction management for the Blocana blockchain
//!
//! This module provides the core transaction implementation including
//! creation, validation, and processing of transactions.

use crate::crypto;
use crate::types::{Hash, PrivateKeyBytes, PublicKeyBytes, SignatureBytes};
use crate::Error;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Transaction structure representing a transfer of value
#[derive(Debug, Clone, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Transaction {
    /// Transaction format version
    pub version: u8,
    /// Sender public key
    pub sender: PublicKeyBytes,
    /// Recipient public key
    pub recipient: PublicKeyBytes,
    /// Amount to transfer
    pub amount: u64,
    /// Fee for the transaction
    pub fee: u64,
    /// Sender's transaction counter (for replay protection)
    pub nonce: u64,
    /// Optional transaction data
    pub data: Vec<u8>,
    /// Transaction signature
    #[serde(with = "BigArray")]
    pub signature: SignatureBytes,
}

pub mod pool;

impl Transaction {
    /// Create a new unsigned transaction
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

    /// Sign a transaction with the sender's private key
    pub fn sign(&mut self, private_key: &PrivateKeyBytes) -> Result<(), Error> {
        // Create a message to sign (hash of transaction data without signature)
        let message = self.serialized_for_signing();

        // Sign the message
        self.signature = crypto::sign_message(private_key, &message)?;

        Ok(())
    }

    /// Verify the transaction signature and validate transaction structure
    ///
    /// This performs a comprehensive verification of the transaction including:
    /// - Signature verification against the sender's public key
    /// - Structural validation (amounts, version, etc.)
    /// - Size limits
    ///
    /// # Returns
    /// `Ok(())` if the transaction is valid, otherwise an `Error`
    pub fn verify(&self) -> Result<(), Error> {
        // Check transaction version
        if self.version != 1 {
            return Err(Error::Validation(format!(
                "Invalid transaction version: {}",
                self.version
            )));
        }

        // Check for zero amount
        if self.amount == 0 {
            return Err(Error::Validation(
                "Transaction amount cannot be zero".into(),
            ));
        }

        // Check for zero fee
        if self.fee == 0 {
            return Err(Error::Validation("Transaction fee cannot be zero".into()));
        }

        // Check data size limits
        if self.data.len() > 1024 * 10 {
            // 10KB limit
            return Err(Error::Validation(format!(
                "Transaction data too large: {} bytes exceeds limit",
                self.data.len()
            )));
        }

        // Check for self-transfer
        if self.sender == self.recipient {
            return Err(Error::Validation(
                "Sender cannot be the same as recipient".into(),
            ));
        }

        // Check for potential overflow in transaction total
        if self.amount.checked_add(self.fee).is_none() {
            return Err(Error::Validation(
                "Transaction amount and fee overflow".into(),
            ));
        }

        // Create the message that was signed (hash of transaction data without signature)
        let message = self.serialized_for_signing();

        // Verify the signature
        crypto::verify_signature(&self.sender, &self.signature, &message)
    }

    /// Calculate the transaction hash
    ///
    /// This hash uniquely identifies the transaction and is used for:
    /// - Transaction indexing in the pool
    /// - Merkle tree construction
    /// - Transaction lookups
    ///
    /// # Returns
    /// A 32-byte hash value
    pub fn hash(&self) -> Hash {
        // Use the serialized transaction data for signing (without signature)
        let message = self.serialized_for_signing();

        // Hash the serialized data
        crypto::hash_data(&message)
    }

    /// Serialize the transaction data for signing
    ///
    /// This produces a byte array containing all transaction fields
    /// except the signature itself.
    ///
    /// # Returns
    /// A vector of bytes representing the transaction data
    fn serialized_for_signing(&self) -> Vec<u8> {
        // Create serialized representation of the transaction without signature
        let mut data = Vec::with_capacity(128);

        // Add version
        data.push(self.version);

        // Add sender
        data.extend_from_slice(&self.sender);

        // Add recipient
        data.extend_from_slice(&self.recipient);

        // Add amount (8 bytes, little-endian)
        data.extend_from_slice(&self.amount.to_le_bytes());

        // Add fee (8 bytes, little-endian)
        data.extend_from_slice(&self.fee.to_le_bytes());

        // Add nonce (8 bytes, little-endian)
        data.extend_from_slice(&self.nonce.to_le_bytes());

        // Add data length (4 bytes, little-endian)
        let data_len = self.data.len() as u32;
        data.extend_from_slice(&data_len.to_le_bytes());

        // Add data
        data.extend_from_slice(&self.data);

        data
    }

    /// Estimate the size of the transaction in bytes
    ///
    /// This provides an approximation of how much space the transaction
    /// will occupy when serialized, which is useful for fee calculations
    /// and memory management.
    ///
    /// # Returns
    /// Estimated size in bytes
    pub fn estimate_size(&self) -> usize {
        // Base size (fixed overhead)
        let base_size = 1 +                 // version (u8)
                   32 +                 // sender (32 bytes)
                   32 +                 // recipient (32 bytes)
                   8 +                  // amount (u64)
                   8 +                  // fee (u64)
                   8 +                  // nonce (u64)
                   64; // signature (64 bytes)

        // Suma el tamaño base más el tamaño real del vector de datos
        base_size + self.data.len()
    }

    /// Get the fee-per-byte for this transaction
    ///
    /// Fee-per-byte is commonly used for transaction prioritization
    /// in the transaction pool.
    ///
    /// # Returns
    /// Fee per byte as a u64
    pub fn fee_per_byte(&self) -> f64 {
        let size = self.estimate_size() as f64;
        self.fee as f64 / size
    }

    /// Check if this transaction can pay the minimum required fee
    ///
    /// # Parameters
    /// * `min_fee_per_byte` - The minimum fee per byte required
    ///
    /// # Returns
    /// `true` if the transaction meets the minimum fee requirement
    pub fn meets_fee_requirement(&self, min_fee_per_byte: f64) -> bool {
        self.fee_per_byte() >= min_fee_per_byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyPair;

    #[test]
    fn test_transaction_creation() {
        let sender = [1u8; 32];
        let recipient = [2u8; 32];

        let tx = Transaction::new(sender, recipient, 100, 10, 0, vec![1, 2, 3]);

        assert_eq!(tx.version, 1);
        assert_eq!(tx.sender, sender);
        assert_eq!(tx.recipient, recipient);
        assert_eq!(tx.amount, 100);
        assert_eq!(tx.fee, 10);
        assert_eq!(tx.nonce, 0);
        assert_eq!(tx.data, vec![1, 2, 3]);
        assert_eq!(tx.signature, [0u8; 64]);
    }

    #[test]
    fn test_transaction_hash() {
        // Create a test transaction
        let sender = [1u8; 32];
        let recipient = [2u8; 32];

        let tx1 = Transaction::new(sender, recipient, 100, 10, 0, vec![]);

        // Get the hash
        let hash1 = tx1.hash();

        // Hash should not be all zeros
        assert_ne!(hash1, [0u8; 32]);

        // Same transaction data should produce the same hash
        let tx2 = Transaction::new(sender, recipient, 100, 10, 0, vec![]);

        let hash2 = tx2.hash();
        assert_eq!(hash1, hash2);

        // Different transaction data should produce different hash
        let tx3 = Transaction::new(
            sender,
            recipient,
            200, // Different amount
            10,
            0,
            vec![],
        );

        let hash3 = tx3.hash();
        assert_ne!(hash1, hash3);

        // Test that signature doesn't affect the hash
        let mut tx4 = tx1.clone();
        tx4.signature = [1u8; 64]; // Different signature

        let hash4 = tx4.hash();
        assert_eq!(hash1, hash4); // Hash should still match tx1
    }

    #[test]
    fn test_sign_and_verify() {
        // Generate a key pair
        let keypair = KeyPair::generate().unwrap();

        // Create a transaction
        let mut tx = Transaction::new(keypair.public_key, [2u8; 32], 100, 10, 0, vec![1, 2, 3]);

        // Initially, verify should fail (no signature)
        assert!(tx.verify().is_err());

        // Sign the transaction
        tx.sign(&keypair.private_key).unwrap();

        // Now verify should succeed
        assert!(tx.verify().is_ok());

        // Modify transaction data
        let mut tx2 = tx.clone();
        tx2.amount = 200;

        // Verify should now fail with the modified data
        assert!(tx2.verify().is_err());
    }

    #[test]
    fn test_transaction_verify_structural_rules() {
        let keypair = KeyPair::generate().unwrap();
        let recipient = [2u8; 32];

        // Test invalid version
        let mut tx = Transaction::new(keypair.public_key, recipient, 100, 10, 0, vec![]);
        tx.version = 2; // Currently only version 1 is supported
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_err());

        // Test zero amount
        let mut tx = Transaction::new(
            keypair.public_key,
            recipient,
            0, // Zero amount
            10,
            0,
            vec![],
        );
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_err());

        // Test zero fee
        let mut tx = Transaction::new(
            keypair.public_key,
            recipient,
            100,
            0, // Zero fee
            0,
            vec![],
        );
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_err());

        // Test self-transfer (sender = recipient)
        let mut tx = Transaction::new(
            keypair.public_key,
            keypair.public_key, // Same as sender
            100,
            10,
            0,
            vec![],
        );
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_err());

        // Test overlarge data
        let mut tx = Transaction::new(
            keypair.public_key,
            recipient,
            100,
            10,
            0,
            vec![0; 1024 * 10 + 1], // Just over 10KB
        );
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_err());

        // Test valid transaction
        let mut tx = Transaction::new(
            keypair.public_key,
            recipient,
            100,
            10,
            0,
            vec![1, 2, 3], // Small data
        );
        tx.sign(&keypair.private_key).unwrap();
        assert!(tx.verify().is_ok());
    }

    #[test]
    fn test_fee_per_byte() {
        let sender = [1u8; 32];
        let recipient = [2u8; 32];

        // Create transaction with 200 byte size and 100 fee
        let tx = Transaction::new(
            sender,
            recipient,
            100,
            100, // fee
            0,
            vec![0; 47], // Add data to reach roughly 200 bytes
        );

        let size = tx.estimate_size();
        println!("Transaction size: {} bytes", size);

        // Fee per byte should be approximately 0.5 (100/200)
        let fee_per_byte = tx.fee_per_byte();
        assert!(fee_per_byte >= 0.4 && fee_per_byte <= 0.6);

        // Test meets_fee_requirement
        assert!(tx.meets_fee_requirement(0.4));
        assert!(!tx.meets_fee_requirement(1.0));
    }

    #[test]
    fn test_estimate_size() {
        // Create a transaction with no data
        let sender = [1u8; 32];
        let recipient = [2u8; 32];
        let tx_no_data = Transaction::new(sender, recipient, 100, 10, 0, vec![]);

        // Expected size: 1 (version) + 32 (sender) + 32 (recipient) +
        // 8 (amount) + 8 (fee) + 8 (nonce) + 64 (signature) + 8 (data length) = 161 bytes
        assert_eq!(tx_no_data.estimate_size(), 153);

        // Create a transaction with data
        let tx_with_data = Transaction::new(
            sender,
            recipient,
            100,
            10,
            0,
            vec![0u8; 50], // 50 bytes of data
        );

        // Expected size: 153 (base size) + 50 (data) = 203 bytes
        assert_eq!(tx_with_data.estimate_size(), 203);
    }
}
