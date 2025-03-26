# Cryptography Implementation Guide

This document provides detailed implementation instructions for Blocana's cryptographic components.

## SHA-256 Hash Implementation

### Requirements
- Must handle arbitrary length inputs
- Must produce consistent 32-byte outputs
- Must be compatible with standard SHA-256 implementations
- Must be efficient for frequent small inputs

### Implementation Steps

1. **Add the sha2 crate to your dependencies**

   In your `Cargo.toml`:
   ```toml
   [dependencies]
   sha2 = "0.10.6"
   ```

2. **Create core hash function**

   Create a module to encapsulate all hashing functionality:

   ```rust
   use sha2::{Sha256, Digest};
   
   /// Type alias for hash results
   pub type Hash = [u8; 32];
   
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
   ```

3. **Create specialized hash functions**

   ```rust
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
   
   /// Hash the contents of a block header
   pub fn hash_block_header(header: &BlockHeader) -> Hash {
       // Serialize header to bytes in canonical form
       let bytes = header.serialize_for_hashing();
       
       // Hash the serialized data
       hash_data(&bytes)
   }
   ```

4. **Implement testing with standard test vectors**

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_empty_hash() {
           // SHA-256 of empty input: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
           let expected = [
               0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 
               0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24, 
               0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 
               0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55
           ];
           
           let result = hash_data(&[]);
           assert_eq!(result, expected);
       }
       
       #[test]
       fn test_standard_vector() {
           // SHA-256 of "abc": ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
           let input = b"abc";
           let expected = [
               0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea,
               0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
               0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
               0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad
           ];
           
           let result = hash_data(input);
           assert_eq!(result, expected);
       }
   }
   ```

5. **Performance optimization considerations**

   - Use thread-local hashers for repeated hashing operations
   - For small inputs, avoid unnecessary allocations
   - Consider batch processing when hashing multiple items

## Ed25519 Signature Implementation

### Requirements
- Must support key generation, signing, and verification
- Must conform to RFC 8032 Ed25519 standard
- Must be secure against timing attacks
- Must handle 32-byte public keys and 64-byte signatures

### Implementation Steps

1. **Add the ed25519-dalek crate to your dependencies**

   In your `Cargo.toml`:
   ```toml
   [dependencies]
   ed25519_dalek = "1.0.1"
   rand = "0.8.5"
   ```

2. **Create key pair generation function**

   ```rust
   use ed25519_dalek::{Keypair, Signer, Verifier, PublicKey, SecretKey};
   use rand::{rngs::OsRng, RngCore};
   
   pub type PublicKeyBytes = [u8; 32];
   pub type PrivateKeyBytes = [u8; 32];
   pub type SignatureBytes = [u8; 64];
   
   /// Generate a new Ed25519 key pair
   pub fn generate_keypair() -> Result<(PublicKeyBytes, PrivateKeyBytes), Error> {
       let mut csprng = OsRng {};
       let keypair = Keypair::generate(&mut csprng);
       
       let mut public_key = [0u8; 32];
       let mut private_key = [0u8; 32];
       
       public_key.copy_from_slice(keypair.public.as_bytes());
       private_key.copy_from_slice(keypair.secret.as_bytes());
       
       Ok((public_key, private_key))
   }
   ```

3. **Implement the signing function**

   ```rust
   /// Sign a message with a private key
   pub fn sign_message(
       private_key: &PrivateKeyBytes, 
       message: &[u8]
   ) -> Result<SignatureBytes, Error> {
       // Convert private key bytes to dalek SecretKey
       let secret = match SecretKey::from_bytes(private_key) {
           Ok(key) => key,
           Err(_) => return Err(Error::InvalidKey),
       };
       
       // Create public key from secret key
       let public = PublicKey::from(&secret);
       
       // Create keypair from components
       let keypair = Keypair {
           public,
           secret,
       };
       
       // Sign the message
       let signature = keypair.sign(message);
       
       // Convert to fixed-size array
       let mut sig_bytes = [0u8; 64];
       sig_bytes.copy_from_slice(signature.as_ref());
       
       Ok(sig_bytes)
   }
   ```

4. **Implement the verification function**

   ```rust
   /// Verify a signature against a message and public key
   pub fn verify_signature(
       public_key: &PublicKeyBytes,
       signature: &SignatureBytes,
       message: &[u8]
   ) -> Result<(), Error> {
       // Convert public key bytes to dalek PublicKey
       let public = match PublicKey::from_bytes(public_key) {
           Ok(key) => key,
           Err(_) => return Err(Error::InvalidKey),
       };
       
       // Convert signature bytes to dalek Signature
       let sig = match ed25519_dalek::Signature::from_bytes(signature) {
           Ok(s) => s,
           Err(_) => return Err(Error::InvalidSignature),
       };
       
       // Verify the signature
       match public.verify(message, &sig) {
           Ok(_) => Ok(()),
           Err(_) => Err(Error::InvalidSignature),
       }
   }
   ```

5. **Implement test functions**

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_keypair_generation() {
           let result = generate_keypair();
           assert!(result.is_ok());
           
           let (public_key, private_key) = result.unwrap();
           
           // Public key should not be all zeros
           assert_ne!(public_key, [0u8; 32]);
           // Private key should not be all zeros
           assert_ne!(private_key, [0u8; 32]);
       }
       
       #[test]
       fn test_sign_and_verify() {
           let (public_key, private_key) = generate_keypair().unwrap();
           let message = b"This is a test message";
           
           // Sign the message
           let signature = sign_message(&private_key, message).unwrap();
           
           // Verify the signature
           let result = verify_signature(&public_key, &signature, message);
           assert!(result.is_ok());
           
           // Modify the message
           let modified_message = b"This is a MODIFIED message";
           
           // Try to verify with modified message
           let result = verify_signature(&public_key, &signature, modified_message);
           assert!(result.is_err());
       }
   }
   ```

6. **Security considerations**

   - Store private keys securely, preferably encrypted
   - Use constant-time equality comparisons for cryptographic values
   - Ensure memory containing private keys is wiped after use
   - Avoid implementing custom cryptographic algorithms

## Merkle Tree Implementation

### Requirements
- Must efficiently compute a Merkle root from a set of transactions
- Must handle the case of an odd number of leaf nodes
- Must enable efficient inclusion proofs
- Must be compatible with standard Merkle tree implementations

### Implementation Steps

1. **Create Merkle tree structure and types**

   ```rust
   /// Represents a Merkle proof path
   pub struct MerkleProof {
       /// The leaf index being proven
       pub index: usize,
       /// Hash values needed to compute the root
       pub proof_hashes: Vec<Hash>,
       /// The expected root hash
       pub root_hash: Hash,
   }
   
   /// Build a Merkle tree from a list of leaf hashes
   pub fn build_merkle_tree(leaf_hashes: &[Hash]) -> Vec<Hash> {
       if leaf_hashes.is_empty() {
           // Empty tree case
           return vec![[0u8; 32]];
       }
       
       // Start with leaf nodes
       let mut tree = leaf_hashes.to_vec();
       
       // Calculate the next level up until we reach the root
       let mut level_size = leaf_hashes.len();
       while level_size > 1 {
           // Compute the size of the next level up
           let next_level_size = (level_size + 1) / 2;
           
           // For each pair of nodes in the current level
           for i in 0..next_level_size {
               let left_idx = i * 2;
               let right_idx = left_idx + 1;
               
               // If right_idx is valid, hash the pair
               if right_idx < level_size {
                   let parent = hash_pair(&tree[left_idx], &tree[right_idx]);
                   tree.push(parent);
               } else {
                   // No right sibling, duplicate the left node
                   let parent = hash_pair(&tree[left_idx], &tree[left_idx]);
                   tree.push(parent);
               }
           }
           
           level_size = next_level_size;
       }
       
       tree
   }
   ```

2. **Create Merkle root computation function**

   ```rust
   /// Compute the Merkle root from a list of leaf hashes
   pub fn compute_merkle_root(leaf_hashes: &[Hash]) -> Hash {
       if leaf_hashes.is_empty() {
           // Empty tree case
           return [0u8; 32];
       }
       
       let tree = build_merkle_tree(leaf_hashes);
       
       // The last element is the root
       tree[tree.len() - 1]
   }
   ```

3. **Implement Merkle proof generation**

   ```rust
   /// Generate a Merkle proof for a specific leaf index
   pub fn generate_merkle_proof(leaf_hashes: &[Hash], leaf_index: usize) -> Result<MerkleProof, Error> {
       if leaf_index >= leaf_hashes.len() {
           return Err(Error::InvalidLeafIndex);
       }
       
       // Build the complete tree
       let tree = build_merkle_tree(leaf_hashes);
       
       // Get the root hash
       let root_hash = tree[tree.len() - 1];
       
       // Collect proof hashes
       let mut proof_hashes = Vec::new();
       let mut index = leaf_index;
       let mut level_size = leaf_hashes.len();
       let mut level_offset = 0;
       
       while level_size > 1 {
           let sibling_index = if index % 2 == 0 {
               // Left node, sibling is on the right
               index + 1
           } else {
               // Right node, sibling is on the left
               index - 1
           };
           
           // Ensure sibling is within bounds
           if sibling_index < level_size {
               proof_hashes.push(tree[level_offset + sibling_index]);
           }
           
           // Move to parent level
           level_offset += level_size;
           index /= 2;
           level_size = (level_size + 1) / 2;
       }
       
       Ok(MerkleProof {
           index: leaf_index,
           proof_hashes,
           root_hash,
       })
   }
   ```

4. **Implement Merkle proof verification**

   ```rust
   /// Verify a Merkle proof for a specific leaf
   pub fn verify_merkle_proof(
       leaf_hash: &Hash, 
       proof: &MerkleProof
   ) -> bool {
       let mut index = proof.index;
       let mut current_hash = *leaf_hash;
       
       for sibling_hash in &proof.proof_hashes {
           // Determine which node is left and which is right based on the index
           let (left, right) = if index % 2 == 0 {
               // Current node is left
               (&current_hash, sibling_hash)
           } else {
               // Current node is right
               (sibling_hash, &current_hash)
           };
           
           // Compute parent hash
           current_hash = hash_pair(left, right);
           
           // Move to parent index
           index /= 2;
       }
       
       // Final hash should match the expected root
       current_hash == proof.root_hash
   }
   ```

5. **Implement tests**

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_merkle_root_empty() {
           let hashes: Vec<Hash> = vec![];
           let root = compute_merkle_root(&hashes);
           assert_eq!(root, [0u8; 32]);
       }
       
       #[test]
       fn test_merkle_root_single_leaf() {
           let leaf = hash_data(b"single leaf");
           let hashes = vec![leaf];
           
           let root = compute_merkle_root(&hashes);
           // For a single leaf, the root is the hash of the leaf with itself
           let expected = hash_pair(&leaf, &leaf);
           
           assert_eq!(root, expected);
       }
       
       #[test]
       fn test_merkle_root_even_leaves() {
           let leaf1 = hash_data(b"leaf1");
           let leaf2 = hash_data(b"leaf2");
           let leaf3 = hash_data(b"leaf3");
           let leaf4 = hash_data(b"leaf4");
           
           let hashes = vec![leaf1, leaf2, leaf3, leaf4];
           
           // Manually compute the expected root
           let node1 = hash_pair(&leaf1, &leaf2);
           let node2 = hash_pair(&leaf3, &leaf4);
           let expected_root = hash_pair(&node1, &node2);
           
           let root = compute_merkle_root(&hashes);
           
           assert_eq!(root, expected_root);
       }
       
       #[test]
       fn test_merkle_root_odd_leaves() {
           let leaf1 = hash_data(b"leaf1");
           let leaf2 = hash_data(b"leaf2");
           let leaf3 = hash_data(b"leaf3");
           
           let hashes = vec![leaf1, leaf2, leaf3];
           
           // With odd leaf count, the last leaf is duplicated
           let node1 = hash_pair(&leaf1, &leaf2);
           let node2 = hash_pair(&leaf3, &leaf3);
           let expected_root = hash_pair(&node1, &node2);
           
           let root = compute_merkle_root(&hashes);
           
           assert_eq!(root, expected_root);
       }
   }
   ```

## Key Management

### Requirements
- Must securely generate and store cryptographic keys
- Must support key derivation for HD wallets
- Must handle serialization/deserialization for storage

### Implementation Steps

1. **Add dependencies for secure key handling**

   ```toml
   [dependencies]
   rand = "0.8.5"
   zeroize = "1.6.0"
   ```

2. **Create secure key storage types**

   ```rust
   use zeroize::Zeroize;
   
   /// Securely zeroes memory when dropped
   pub struct SecurePrivateKey {
       key_data: PrivateKeyBytes,
   }
   
   impl SecurePrivateKey {
       /// Create a new secure private key
       pub fn new(key_data: PrivateKeyBytes) -> Self {
           Self { key_data }
       }
       
       /// Access the key data
       pub fn as_bytes(&self) -> &PrivateKeyBytes {
           &self.key_data
       }
   }
   
   impl Drop for SecurePrivateKey {
       fn drop(&mut self) {
           // Securely zero the key data when dropped
           self.key_data.zeroize();
       }
   }
   ```

3. **Implement secure key generation**

   ```rust
   /// Generate a new secure private key
   pub fn generate_secure_key() -> Result<SecurePrivateKey, Error> {
       let (_, private_key) = generate_keypair()?;
       Ok(SecurePrivateKey::new(private_key))
   }
   ```

4. **Implement key storage and retrieval**

   ```rust
   /// Encrypt a private key for storage
   pub fn encrypt_private_key(key: &SecurePrivateKey, password: &str) -> Result<Vec<u8>, Error> {
       // This is a placeholder - in a real implementation, use a proper
       // key derivation function and authenticated encryption
       
       let mut key_bytes = key.as_bytes().clone();
       
       // Derive encryption key from password
       let mut encryption_key = [0u8; 32];
       // In a real implementation, use PBKDF2, Argon2, or similar
       
       // Encrypt the private key
       // In a real implementation, use AES-GCM, ChaCha20-Poly1305, or similar
       
       // Return the encrypted key with metadata
       Ok(Vec::new()) // Placeholder
   }
   
   /// Decrypt a private key from storage
   pub fn decrypt_private_key(encrypted_key: &[u8], password: &str) -> Result<SecurePrivateKey, Error> {
       // This is a placeholder - implement proper decryption
       
       // Derive decryption key from password
       
       // Decrypt and verify the private key
       
       // Return the secure private key
       Ok(SecurePrivateKey::new([0u8; 32])) // Placeholder
   }
   ```

## Conclusion

This cryptography implementation guide provides detailed instructions for implementing the core cryptographic components of Blocana. Following these guidelines ensures that the cryptographic foundations of the blockchain are secure, efficient, and maintainable.

Remember that cryptography is a specialized field, and small mistakes can have serious security implications. Always prefer well-tested libraries and follow established best practices when implementing cryptographic functionality. When in doubt, consult with security experts or conduct thorough security reviews.

--- End of Document ---