//! Common types used throughout Blocana
//!
//! This module provides type definitions used across different modules
//! to avoid circular dependencies.

// use serde_big_array::BigArray;

/// Hash type used throughout the blockchain (32 bytes)
pub type Hash = [u8; 32];

/// Type alias for public key bytes (32 bytes)
pub type PublicKeyBytes = [u8; 32];

/// Type alias for private key bytes (32 bytes)
pub type PrivateKeyBytes = [u8; 32];

/// Type alias for signature bytes (64 bytes)
pub type SignatureBytes = [u8; 64];
