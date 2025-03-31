//! Specialized error types for transaction processing
//!
//! This module provides detailed error types specific to transaction
//! validation and pool operations, offering better context and
//! categorization than generic errors.

use thiserror::Error;
use crate::types::{Hash, PublicKeyBytes};

/// Transaction pool specific errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum TransactionError {
    /// Transaction validation error
    #[error("Transaction validation failed: {0}")]
    Validation(String),
    
    /// Transaction already in the pool
    #[error("Transaction {tx_hash:?} already exists in pool")]
    AlreadyExists {
        /// Hash of the duplicate transaction
        tx_hash: Hash,
    },
    
    /// Transaction has an invalid signature
    #[error("Invalid transaction signature")]
    InvalidSignature,
    
    /// Transaction nonce is invalid
    #[error("Invalid nonce for {sender:?}: expected {expected}, got {actual}")]
    InvalidNonce {
        /// Transaction sender
        sender: PublicKeyBytes,
        /// Expected nonce value
        expected: u64,
        /// Actual nonce in transaction
        actual: u64,
    },
    
    /// Transaction sender has insufficient balance
    #[error("Insufficient balance for {sender:?}: has {balance}, needs {required}")]
    InsufficientBalance {
        /// Transaction sender
        sender: PublicKeyBytes,
        /// Current balance
        balance: u64,
        /// Required balance
        required: u64,
    },
    
    /// Transaction fee is too low
    #[error("Transaction fee too low: {fee_per_byte} per byte, minimum is {min_required}")]
    FeeTooLow {
        /// Transaction fee per byte
        fee_per_byte: u64,
        /// Minimum required fee per byte
        min_required: u64,
    },
    
    /// Fee too low for replacement
    #[error("Replacement fee too low: got {actual}, need at least {required}")]
    ReplacementFeeTooLow {
        /// Actual fee offered
        actual: u64,
        /// Minimum required fee
        required: u64,
    },
    
    /// Transaction data is too large
    #[error("Transaction data too large: {size} bytes, maximum is {max_size}")]
    DataTooLarge {
        /// Transaction size
        size: usize,
        /// Maximum allowed size
        max_size: usize,
    },
    
    /// Transaction pool is full
    #[error("Transaction pool is full: {current_size}/{max_size} transactions")]
    PoolFull {
        /// Current number of transactions
        current_size: usize,
        /// Maximum number of transactions
        max_size: usize,
    },
    
    /// Transaction pool is out of memory
    #[error("Transaction pool memory limit reached: {current_bytes}/{max_bytes} bytes")]
    MemoryLimitReached {
        /// Current memory usage
        current_bytes: usize,
        /// Maximum allowed memory
        max_bytes: usize,
    },
    
    /// Transaction is expired
    #[error("Transaction {tx_hash:?} is expired: created at {creation_time}, expired at {expiry_time}")]
    Expired {
        /// Hash of the expired transaction
        tx_hash: Hash,
        /// Transaction creation time
        creation_time: u64,
        /// Transaction expiry time
        expiry_time: u64,
    },

    /// Transaction rate limiting
    #[error("Rate limited: too many transactions from sender {sender:?}")]
    RateLimited {
        /// Address being rate limited
        sender: PublicKeyBytes,
    },
    
    /// Temporary failure that might succeed if tried again later
    #[error("Temporary failure: {0}")]
    Temporary(String),
    
    /// Internal pool error
    #[error("Transaction pool internal error: {0}")]
    Internal(String),
    
    /// Cryptographic error
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    /// Database error
    #[error("Database error: {0}")]
    Database(String),
}

impl TransactionError {
    /// Checks if the error indicates a duplicate transaction
    pub fn is_duplicate(&self) -> bool {
        matches!(self, TransactionError::AlreadyExists { .. })
    }
    
    /// Checks if the error is related to the transaction's nonce
    pub fn is_nonce_error(&self) -> bool {
        matches!(self, TransactionError::InvalidNonce { .. })
    }
    
    /// Checks if the error is related to the transaction's fee
    pub fn is_fee_error(&self) -> bool {
        matches!(
            self,
            TransactionError::FeeTooLow { .. } | 
            TransactionError::ReplacementFeeTooLow { .. }
        )
    }
    
    /// Checks if the error is related to the sender's balance
    pub fn is_balance_error(&self) -> bool {
        matches!(self, TransactionError::InsufficientBalance { .. })
    }
    
    /// Checks if the error is related to pool resource constraints
    pub fn is_resource_error(&self) -> bool {
        matches!(
            self,
            TransactionError::PoolFull { .. } | 
            TransactionError::MemoryLimitReached { .. }
        )
    }
    
    /// Checks if the error is temporary and might succeed if tried again later
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            TransactionError::Temporary(_) | 
            TransactionError::RateLimited { .. } |
            TransactionError::PoolFull { .. } |
            TransactionError::MemoryLimitReached { .. }
        )
    }

    /// Gets the expected nonce if this is a nonce error
    pub fn expected_nonce(&self) -> Option<u64> {
        if let TransactionError::InvalidNonce { expected, .. } = self {
            Some(*expected)
        } else {
            None
        }
    }
    
    /// Gets the minimum required fee if this is a fee error
    pub fn minimum_required_fee(&self) -> Option<u64> {
        match self {
            TransactionError::FeeTooLow { min_required, .. } => Some(*min_required),
            TransactionError::ReplacementFeeTooLow { required, .. } => Some(*required),
            _ => None,
        }
    }
    
    /// Provides additional context for log messages
    pub fn log_context(&self) -> String {
        match self {
            TransactionError::AlreadyExists { tx_hash } => 
                format!("Duplicate transaction: {}", hex::encode(&tx_hash[0..4])),
                
            TransactionError::InvalidNonce { sender, expected, actual } => 
                format!("Nonce error for {}: expected {}, got {}", 
                        hex::encode(&sender[0..4]), expected, actual),
                        
            TransactionError::InsufficientBalance { sender, balance, required } =>
                format!("Balance error for {}: has {}, needs {}", 
                        hex::encode(&sender[0..4]), balance, required),
            
            TransactionError::PoolFull { current_size, max_size } =>
                format!("Pool capacity at {}/{} ({:.1}%)", 
                        current_size, max_size, 
                        (*current_size as f64 / *max_size as f64) * 100.0),
                        
            TransactionError::MemoryLimitReached { current_bytes, max_bytes } =>
                format!("Memory at {}/{} bytes ({:.1}%)", 
                        current_bytes, max_bytes,
                        (*current_bytes as f64 / *max_bytes as f64) * 100.0),
                        
            _ => format!("{}", self),
        }
    }

    /// Convert from general Error to TransactionError
    pub fn from_error(err: &crate::Error) -> Self {
        match err {
            crate::Error::Validation(msg) => TransactionError::Validation(msg.clone()),
            crate::Error::Crypto(msg) => TransactionError::Crypto(msg.clone()),
            crate::Error::DB(msg) => TransactionError::Database(msg.clone()),
            _ => TransactionError::Internal(format!("Unknown error: {:?}", err)),
        }
    }
}

/// Result type for transaction pool operations
pub type Result<T> = std::result::Result<T, TransactionError>;

// Avoid implicit conversions that could cause errors
// Instead, use explicit conversion when needed through the from_error method
// Only implement From<TransactionError> for crate::Error to maintain compatibility
impl From<TransactionError> for crate::Error {
    fn from(err: TransactionError) -> Self {
        match err {
            TransactionError::Validation(msg) => crate::Error::Validation(msg),
            TransactionError::Crypto(msg) => crate::Error::Crypto(msg),
            TransactionError::Database(msg) => crate::Error::DB(format!("Database error: {}", msg)),
            // For all other specific transaction errors, convert to validation errors with descriptive messages
            _ => crate::Error::Validation(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_categorization() {
        let nonce_error = TransactionError::InvalidNonce {
            sender: [0u8; 32],
            expected: 5,
            actual: 3,
        };
        
        assert!(nonce_error.is_nonce_error());
        assert!(!nonce_error.is_fee_error());
        assert!(!nonce_error.is_balance_error());
        assert_eq!(nonce_error.expected_nonce(), Some(5));
        
        let fee_error = TransactionError::FeeTooLow {
            fee_per_byte: 1,
            min_required: 2,
        };
        
        assert!(fee_error.is_fee_error());
        assert!(!fee_error.is_nonce_error());
        assert_eq!(fee_error.minimum_required_fee(), Some(2));
        
        let balance_error = TransactionError::InsufficientBalance {
            sender: [0u8; 32],
            balance: 100,
            required: 150,
        };
        
        assert!(balance_error.is_balance_error());
        assert!(!balance_error.is_fee_error());
        
        let resource_error = TransactionError::PoolFull {
            current_size: 5000,
            max_size: 5000,
        };
        
        assert!(resource_error.is_resource_error());
        assert!(resource_error.is_temporary());
    }
    
    #[test]
    fn test_error_messages() {
        let nonce_error = TransactionError::InvalidNonce {
            sender: [1u8; 32],
            expected: 5,
            actual: 3,
        };
        
        let error_msg = nonce_error.to_string();
        assert!(error_msg.contains("Invalid nonce"));
        assert!(error_msg.contains("expected 5"));
        assert!(error_msg.contains("got 3"));
        
        let fee_error = TransactionError::FeeTooLow {
            fee_per_byte: 1,
            min_required: 2,
        };
        
        let error_msg = fee_error.to_string();
        assert!(error_msg.contains("Transaction fee too low"));
        assert!(error_msg.contains("minimum is 2"));
        
        let log_context = fee_error.log_context();
        assert!(log_context.contains("Transaction fee too low"));
    }
    
    #[test]
    fn test_error_conversion() {
        // Test conversion from TransactionError to Error
        let tx_error = TransactionError::InvalidSignature;
        let error: crate::Error = tx_error.into();
        match error {
            crate::Error::Validation(msg) => {
                assert!(msg.contains("Invalid transaction signature"));
            },
            _ => panic!("Wrong error type after conversion"),
        }
        
        // Test conversion from Error to TransactionError
        let error = crate::Error::Validation("Test validation error".to_string());
        let tx_error = TransactionError::from_error(&error);
        match tx_error {
            TransactionError::Validation(msg) => {
                assert_eq!(msg, "Test validation error");
            },
            _ => panic!("Wrong error type after conversion"),
        }
    }
}
