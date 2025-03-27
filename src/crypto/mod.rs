//! Cryptographic utilities for Blocana
//!
//! This module provides the core cryptographic functions used in the blockchain.

use sha2::{Sha256, Digest};
use ed25519_dalek::{Keypair, Signer, Verifier, PublicKey, SecretKey};
use rand::{rngs::OsRng, RngCore};
use crate::types::{Hash, PublicKeyBytes, PrivateKeyBytes, SignatureBytes};

/// Key pair structure
pub struct KeyPair {
    pub public_key: PublicKeyBytes,
    pub private_key: PrivateKeyBytes,
    keypair: Keypair,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Result<Self, crate::Error> {
        let mut csprng = OsRng{};
        let keypair = Keypair::generate(&mut csprng);
        
        let mut public_key = [0u8; 32];
        let mut private_key = [0u8; 32];
        
        public_key.copy_from_slice(keypair.public.as_bytes());
        private_key.copy_from_slice(keypair.secret.as_bytes());
        
        Ok(Self {
            public_key,
            private_key,
            keypair,
        })
    }
    
    /// Create a key pair from an existing private key
    pub fn from_private_key(private_key: &PrivateKeyBytes) -> Result<Self, crate::Error> {
        // Convert to ed25519-dalek SecretKey
        let secret = SecretKey::from_bytes(private_key)
            .map_err(|_| crate::Error::Crypto("Invalid private key".into()))?;
        
        // Derive public key
        let public = PublicKey::from(&secret);
        
        // Create keypair
        let keypair = Keypair {
            public,
            secret,
        };
        
        let mut public_key = [0u8; 32];
        let mut private_key_copy = [0u8; 32];
        
        public_key.copy_from_slice(public.as_bytes());
        private_key_copy.copy_from_slice(private_key);
        
        Ok(Self {
            public_key,
            private_key: private_key_copy,
            keypair,
        })
    }
    
    /// Sign a message with this key pair
    pub fn sign(&self, message: &[u8]) -> SignatureBytes {
        let signature = self.keypair.sign(message);
        
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(signature.as_ref());
        
        sig_bytes
    }
}

/// Hash arbitrary data using SHA-256
pub fn hash_data(data: &[u8]) -> Hash {
    // Create a SHA-256 hasher instance
    let mut hasher = Sha256::new();
    
    // Update the hasher with input data
    hasher.update(data);
    
    // Finalize the hash computation
    let result = hasher.finalize();
    
    // Convert to fixed-size array
    let mut hash = [0u8; 32];
    hash.copy_from_slice(result.as_slice());
    
    hash
}

/// Hash two hashes together (useful for Merkle trees)
pub fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    
    let result = hasher.finalize();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(result.as_slice());
    
    hash
}

/// Sign a message with a private key
pub fn sign_message(
    private_key: &PrivateKeyBytes, 
    message: &[u8]
) -> Result<SignatureBytes, crate::Error> {
    // Create keypair from private key
    let keypair = KeyPair::from_private_key(private_key)?;
    
    // Sign the message
    let signature = keypair.sign(message);
    
    Ok(signature)
}

/// Verify a signature against a message and public key
pub fn verify_signature(
    public_key: &PublicKeyBytes,
    signature: &SignatureBytes,
    message: &[u8]
) -> Result<(), crate::Error> {
    // Convert public key bytes to dalek PublicKey
    let public = PublicKey::from_bytes(public_key)
        .map_err(|_| crate::Error::Crypto("Invalid public key".into()))?;
    
    // Convert signature bytes to dalek Signature
    let sig = ed25519_dalek::Signature::from_bytes(signature)
        .map_err(|_| crate::Error::Crypto("Invalid signature".into()))?;
    
    // Verify the signature
    public.verify(message, &sig)
        .map_err(|_| crate::Error::Crypto("Signature verification failed".into()))
}

/// Compute the Merkle root from a list of leaf hashes
pub fn compute_merkle_root(leaf_hashes: &[Hash]) -> Hash {
    if leaf_hashes.is_empty() {
        // Empty tree case
        return [0u8; 32];
    }
    
    // Start with leaf nodes
    let mut hashes = leaf_hashes.to_vec();
    
    // Calculate the next level up until we reach the root
    while hashes.len() > 1 {
        // If we have an odd number of hashes, duplicate the last one
        if hashes.len() % 2 != 0 {
            hashes.push(hashes[hashes.len() - 1]);
        }
        
        let mut next_level = Vec::with_capacity(hashes.len() / 2);
        
        for i in (0..hashes.len()).step_by(2) {
            next_level.push(hash_pair(&hashes[i], &hashes[i + 1]));
        }
        
        hashes = next_level;
    }
    
    // Return the root hash
    hashes[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_data() {
        let data = b"test data";
        let hash = hash_data(data);
        
        // Hash should not be all zeros
        assert_ne!(hash, [0u8; 32]);
        
        // Same data should produce same hash
        let hash2 = hash_data(data);
        assert_eq!(hash, hash2);
        
        // Different data should produce different hash
        let hash3 = hash_data(b"different data");
        assert_ne!(hash, hash3);
    }
    
    #[test]
    fn test_keypair_generation() {
        let result = KeyPair::generate();
        assert!(result.is_ok());
        
        let keypair = result.unwrap();
        
        // Public key should not be all zeros
        assert_ne!(keypair.public_key, [0u8; 32]);
        // Private key should not be all zeros
        assert_ne!(keypair.private_key, [0u8; 32]);
    }
    
    #[test]
    fn test_sign_and_verify() {
        let keypair = KeyPair::generate().unwrap();
        let message = b"This is a test message";
        
        // Sign the message
        let signature = keypair.sign(message);
        
        // Verify the signature
        let result = verify_signature(&keypair.public_key, &signature, message);
        assert!(result.is_ok());
        
        // Modify the message
        let modified_message = b"This is a MODIFIED message";
        
        // Try to verify with modified message
        let result = verify_signature(&keypair.public_key, &signature, modified_message);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_merkle_root() {
        // Test with empty hashes
        let empty_root = compute_merkle_root(&[]);
        assert_eq!(empty_root, [0u8; 32]);
        
        // Test with single hash
        let hash1 = hash_data(b"leaf1");
        let single_root = compute_merkle_root(&[hash1]);
        assert_eq!(single_root, hash1);
        
        // Test with multiple hashes
        let hash2 = hash_data(b"leaf2");
        let hash3 = hash_data(b"leaf3");
        let hash4 = hash_data(b"leaf4");
        
        let hashes = vec![hash1, hash2, hash3, hash4];
        
        // Manually compute the expected root
        let node1 = hash_pair(&hash1, &hash2);
        let node2 = hash_pair(&hash3, &hash4);
        let expected_root = hash_pair(&node1, &node2);
        
        let root = compute_merkle_root(&hashes);
        assert_eq!(root, expected_root);
    }
}
