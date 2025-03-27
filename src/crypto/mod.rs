//! Cryptographic utilities for Blocana
//!
//! This module provides the core cryptographic functions used in the blockchain.
//!
//! # Security Considerations
//!
//! - All cryptographic operations utilize standard, well-tested libraries
//! - Private keys should be securely managed and never stored unencrypted
//! - Signature verification is performed in constant time to prevent timing attacks
//! - Hash functions are collision-resistant but not resistant to length extension attacks
//!
//! # Examples
//!
//! ```
//! # use blocana::crypto;
//! # use blocana::types::{Hash, PublicKeyBytes, PrivateKeyBytes, SignatureBytes};
//! // Generate a new key pair
//! let key_pair = crypto::KeyPair::generate().unwrap();
//!
//! // Sign a message
//! let message = b"Important blockchain data";
//! let signature = key_pair.sign(message);
//!
//! // Verify the signature
//! let result = crypto::verify_signature(&key_pair.public_key, &signature, message);
//! assert!(result.is_ok());
//! ```

use sha2::{Sha256, Digest};
use ed25519_dalek::{Signer, Verifier, Signature};
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use rand::{rngs::OsRng, RngCore};
use crate::types::{Hash, PublicKeyBytes, PrivateKeyBytes, SignatureBytes};

/// Key pair structure
pub struct KeyPair {
    pub public_key: PublicKeyBytes,
    pub private_key: PrivateKeyBytes,
    signing_key: SigningKey,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Result<Self, crate::Error> {
        // Generate random bytes for the private key
        let mut rng = OsRng{};
        let mut private_key_bytes = [0u8; 32];
        rng.fill_bytes(&mut private_key_bytes);
        
        // Create signing key from random bytes
        let signing_key = SigningKey::from_bytes(&private_key_bytes);
        
        // Derive public key
        let verifying_key = VerifyingKey::from(&signing_key);
        
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(verifying_key.as_bytes());
        
        Ok(Self {
            public_key,
            private_key: private_key_bytes,
            signing_key,
        })
    }
    
    /// Create a key pair from an existing private key
    pub fn from_private_key(private_key: &PrivateKeyBytes) -> Result<Self, crate::Error> {
        // Convert to ed25519-dalek SecretKey - Fix for v2.x API
        let secret = match SigningKey::try_from(private_key.as_slice()) {
            Ok(sk) => sk,
            Err(_) => return Err(crate::Error::Crypto("Invalid private key".into())),
        };
        
        // Derive public key
        let public = VerifyingKey::from(&secret);
        
        let mut public_key = [0u8; 32];
        let mut private_key_copy = [0u8; 32];
        
        public_key.copy_from_slice(public.as_bytes());
        private_key_copy.copy_from_slice(private_key);
        
        Ok(Self {
            public_key,
            private_key: private_key_copy,
            signing_key: secret,
        })
    }
    
    /// Sign a message with this key pair
    pub fn sign(&self, message: &[u8]) -> SignatureBytes {
        let signature = self.signing_key.sign(message);
        
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(signature.to_bytes().as_ref());
        
        sig_bytes
    }

    /// Derive a child key from this key pair using a simple derivation path
    ///
    /// This is a basic implementation suitable for creating multiple keys from a master key.
    /// For production HD wallet functionality, a more comprehensive BIP32 implementation
    /// should be used.
    ///
    /// # Parameters
    /// * `path` - A simple numeric index used for derivation
    ///
    /// # Returns
    /// A new KeyPair derived from this one
    ///
    /// # Security
    /// This derivation is deterministic - the same path always yields the same child key
    pub fn derive_child_key(&self, path: u32) -> Result<Self, crate::Error> {
        // Create derivation data by combining private key and path
        let mut derivation_data = Vec::with_capacity(36); // 32 bytes for key + 4 for path
        derivation_data.extend_from_slice(&self.private_key);
        derivation_data.extend_from_slice(&path.to_le_bytes());
        
        // Hash the data to create a new deterministic private key
        let derived_private_key = hash_data(&derivation_data);
        
        // Create a new keypair from this derived key
        Self::from_private_key(&derived_private_key)
    }
    
    /// Securely zeroize the private key material when the KeyPair is dropped
    ///
    /// This helps prevent private key data from remaining in memory after it's no longer needed
    pub fn zeroize(&mut self) {
        use zeroize::Zeroize;
        self.private_key.zeroize();
        // Note: Complete zeroization of ed25519_dalek::SigningKey would require changes to that library
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
    let public = match VerifyingKey::try_from(public_key.as_slice()) {
        Ok(pk) => pk,
        Err(_) => return Err(crate::Error::Crypto("Invalid public key".into())),
    };
    
    // Convert signature bytes to dalek Signature - Fix for v2.x API
    let sig = match ed25519_dalek::Signature::try_from(signature.as_slice()) {
        Ok(s) => s,
        Err(_) => return Err(crate::Error::Crypto("Invalid signature".into())),
    };
    
    // Verify the signature
    public.verify_strict(message, &sig)
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

/// Compute a keyed hash using HMAC-SHA256
///
/// This is useful for creating authentication codes or deriving keys
///
/// # Parameters
/// * `key` - The secret key for the HMAC
/// * `message` - The message to authenticate
///
/// # Returns
/// A 32-byte HMAC value
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> Hash {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    
    // Create HMAC instance
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take keys of any size");
    
    // Add message data
    mac.update(message);
    
    // Finalize and return
    let result = mac.finalize().into_bytes();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Generate a secure random value
///
/// Useful for nonces and other cryptographically secure random data needs
///
/// # Returns
/// A random 32-byte value from the OS secure random number generator
pub fn generate_secure_random() -> Hash {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

/// Verify multiple signatures in batch for improved performance
pub fn batch_verify_signatures(
    messages: &[&[u8]],
    signatures: &[&SignatureBytes],
    public_keys: &[&PublicKeyBytes]
) -> Result<(), crate::Error> {
    // Check that all arrays have the same length
    if messages.len() != signatures.len() || messages.len() != public_keys.len() {
        return Err(crate::Error::Crypto("Mismatched array lengths for batch verification".into()));
    }
    
    // In ed25519-dalek v2.x, we need to use a Verifier instance
    // use ed25519_dalek::Verifier;
    
    // Process each signature individually
    // Note: This doesn't have the performance benefits of true batch verification
    // but maintains API compatibility
    for i in 0..messages.len() {
        // Convert public key
        let public = match VerifyingKey::try_from(public_keys[i].as_slice()) {
            Ok(pk) => pk,
            Err(_) => return Err(crate::Error::Crypto(format!("Invalid public key at index {}", i))),
        };
        
        // Convert signature
        let sig = match ed25519_dalek::Signature::try_from(signatures[i].as_slice()) {
            Ok(s) => s,
            Err(_) => return Err(crate::Error::Crypto(format!("Invalid signature at index {}", i))),
        };
        
        // Verify this signature
        if let Err(_) = public.verify_strict(messages[i], &sig) {
            return Err(crate::Error::Crypto(format!("Signature verification failed at index {}", i)));
        }
    }
    
    // All verifications passed
    Ok(())
}

/// Get a human-readable hex representation of a hash
///
/// # Parameters
/// * `hash` - The hash to convert to hex
///
/// # Returns
/// A lowercase hex string representing the hash
pub fn hash_to_hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Parse a hex string into a hash
///
/// # Parameters
/// * `hex_str` - The hex string to parse
///
/// # Returns
/// A Hash if the string is valid, Error otherwise
pub fn hex_to_hash(hex_str: &str) -> Result<Hash, crate::Error> {
    if hex_str.len() != 64 {
        return Err(crate::Error::Crypto("Invalid hex string length".into()));
    }
    
    let mut hash = [0u8; 32];
    for i in 0..32 {
        let pos = i * 2;
        let byte_str = &hex_str[pos..pos+2];
        hash[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| crate::Error::Crypto("Invalid hex string".into()))?;
    }
    
    Ok(hash)
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

    #[test]
    fn test_derive_child_key() {
        let master = KeyPair::generate().unwrap();
        
        // Derive two children with different paths
        let child1 = master.derive_child_key(1).unwrap();
        let child2 = master.derive_child_key(2).unwrap();
        
        // Derive child1 again - should get the same key
        let child1_again = master.derive_child_key(1).unwrap();
        
        // Children should be different from parent
        assert_ne!(master.public_key, child1.public_key);
        assert_ne!(master.private_key, child1.private_key);
        
        // Different children should be different from each other
        assert_ne!(child1.public_key, child2.public_key);
        
        // Same derivation path should produce identical keys
        assert_eq!(child1.public_key, child1_again.public_key);
        assert_eq!(child1.private_key, child1_again.private_key);
    }
    
    #[test]
    fn test_hmac_sha256() {
        let key = b"secret key";
        let message = b"test message";
        
        let hmac1 = hmac_sha256(key, message);
        
        // Same inputs should produce same HMAC
        let hmac2 = hmac_sha256(key, message);
        assert_eq!(hmac1, hmac2);
        
        // Different messages should produce different HMACs
        let hmac3 = hmac_sha256(key, b"different message");
        assert_ne!(hmac1, hmac3);
        
        // Different keys should produce different HMACs
        let hmac4 = hmac_sha256(b"different key", message);
        assert_ne!(hmac1, hmac4);
    }
    
    #[test]
    fn test_batch_verify_signatures() {
        // Generate three keypairs
        let keypair1 = KeyPair::generate().unwrap();
        let keypair2 = KeyPair::generate().unwrap();
        let keypair3 = KeyPair::generate().unwrap();
        
        // Create messages
        let message1 = b"message 1";
        let message2 = b"message 2";
        let message3 = b"message 3";
        
        // Sign the messages
        let sig1 = keypair1.sign(message1);
        let sig2 = keypair2.sign(message2);
        let sig3 = keypair3.sign(message3);
        
        // Batch verify - should succeed
        let result = batch_verify_signatures(
            &[message1, message2, message3],
            &[&sig1, &sig2, &sig3],
            &[&keypair1.public_key, &keypair2.public_key, &keypair3.public_key]
        );
        assert!(result.is_ok());
        
        // Batch verify with mismatched signature - should fail
        let result = batch_verify_signatures(
            &[message1, message2, message3],
            &[&sig1, &sig3, &sig2], // Signatures in wrong order
            &[&keypair1.public_key, &keypair2.public_key, &keypair3.public_key]
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_hash_to_hex() {
        let hash = hash_data(b"test");
        let hex = hash_to_hex(&hash);
        
        // Should be 64 characters (32 bytes * 2)
        assert_eq!(hex.len(), 64);
        
        // Convert back to hash
        let hash2 = hex_to_hash(&hex).unwrap();
        
        // Roundtrip should match
        assert_eq!(hash, hash2);
    }
    
    #[test]
    fn test_generate_secure_random() {
        let random1 = generate_secure_random();
        let random2 = generate_secure_random();
        
        // Two random values should be different
        assert_ne!(random1, random2);
    }
}
