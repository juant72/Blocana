//! Account state storage implementation for the Blocana blockchain
//!
//! This module provides a specialized interface for working with account state storage,
//! offering methods tailored to account state operations while abstracting the underlying
//! storage details.
//!
//! # Examples
//!
//! ```
//! use blocana::storage::{BlockchainStorage, StateStore, StorageConfig};
//! use blocana::state::AccountState;
//!
//! // Open the database
//! let config = StorageConfig::default();
//! let storage = BlockchainStorage::open(&config).unwrap();
//!
//! // Create a state store
//! let state_store = StateStore::new(&storage);
//!
//! // Get account state
//! let address = [0u8; 32];
//! let account = state_store.get_account_state(&address).unwrap();
//!
//! // Update account state
//! state_store.update_account_state(&address, |state| {
//!     state.balance += 100;
//!     state.nonce += 1;
//! }).unwrap();
//! ```

use super::{BlockchainStorage, Error};
use crate::state::AccountState;
use crate::types::PublicKeyBytes;
use std::collections::HashMap;

/// A specialized store for account state operations
///
/// Provides a higher-level interface for working with account states in storage,
/// abstracting the underlying database operations.
pub struct StateStore<'a> {
    /// Reference to the underlying storage
    storage: &'a BlockchainStorage,
}

impl<'a> StateStore<'a> {
    /// Creates a new state store.
    ///
    /// # Parameters
    /// * `storage` - Reference to the blockchain storage
    ///
    /// # Returns
    /// A new `StateStore` instance
    pub fn new(storage: &'a BlockchainStorage) -> Self {
        Self { storage }
    }

    /// Gets account state for an address.
    ///
    /// If the account doesn't exist, returns a new default account state.
    ///
    /// # Parameters
    /// * `address` - The account address
    ///
    /// # Returns
    /// The account state (existing or newly created)
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn get_account_state(&self, address: &PublicKeyBytes) -> Result<AccountState, Error> {
        match self.storage.get_account_state(address)? {
            Some(state) => Ok(state),
            None => Ok(AccountState::new()), // Return default state if not found
        }
    }

    /// Stores account state for an address.
    ///
    /// # Parameters
    /// * `address` - The account address
    /// * `state` - The account state to store
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn store_account_state(
        &self,
        address: &PublicKeyBytes,
        state: &AccountState,
    ) -> Result<(), Error> {
        self.storage.store_account_state(address, state)
    }

    /// Updates account state using a transformation function.
    ///
    /// Retrieves the current state, applies the transformation function,
    /// and stores the updated state in a single operation.
    ///
    /// # Parameters
    /// * `address` - The account address
    /// * `f` - Function that transforms the account state
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn update_account_state<F>(&self, address: &PublicKeyBytes, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut AccountState),
    {
        // Get current state
        let mut state = self.get_account_state(address)?;

        // Apply the transformation
        f(&mut state);

        // Store updated state
        self.store_account_state(address, &state)
    }

    /// Stores multiple account states in a batch.
    ///
    /// This is more efficient than storing states individually.
    ///
    /// # Parameters
    /// * `states` - Map of account addresses to their states
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn store_account_states(
        &self,
        states: HashMap<PublicKeyBytes, AccountState>,
    ) -> Result<(), Error> {
        let cfs = self.storage.get_column_families()?;

        // Create write batch
        let mut batch = rocksdb::WriteBatch::default();

        for (address, state) in states {
            // Change from serialize to encode_to_vec with configuration
            let state_bytes = bincode::encode_to_vec(&state, bincode::config::standard())?;
            batch.put_cf(cfs.account_state, address, state_bytes);
        }

        // Write all states atomically
        self.storage.raw_db().write(batch)?;

        Ok(())
    }

    /// Checks if an account exists in storage.
    ///
    /// # Parameters
    /// * `address` - The account address
    ///
    /// # Returns
    /// `true` if the account exists in storage, `false` otherwise
    ///
    /// # Errors
    /// Returns an error if the storage operation fails
    pub fn account_exists(&self, address: &PublicKeyBytes) -> Result<bool, Error> {
        let exists = self.storage.get_account_state(address)?.is_some();
        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::super::StorageConfig;
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_state_store_operations() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();

        // Config with test path
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };
        {
            // Open the storage and create state store
            let storage = BlockchainStorage::open(&config).unwrap();
            let state_store = StateStore::new(&storage);

            // Test non-existent account returns default state
            let address = [1u8; 32];
            let state = state_store.get_account_state(&address).unwrap();
            assert_eq!(state.balance, 0);
            assert_eq!(state.nonce, 0);

            // Test storing and retrieving state
            let mut new_state = AccountState::new();
            new_state.balance = 1000;
            new_state.nonce = 5;

            state_store
                .store_account_state(&address, &new_state)
                .unwrap();

            let retrieved_state = state_store.get_account_state(&address).unwrap();
            assert_eq!(retrieved_state.balance, 1000);
            assert_eq!(retrieved_state.nonce, 5);

            // Test update_account_state
            state_store
                .update_account_state(&address, |state| {
                    state.balance += 500;
                    state.nonce += 1;
                })
                .unwrap();

            let updated_state = state_store.get_account_state(&address).unwrap();
            assert_eq!(updated_state.balance, 1500);
            assert_eq!(updated_state.nonce, 6);

            // Test account_exists
            assert!(state_store.account_exists(&address).unwrap());
            assert!(!state_store.account_exists(&[2u8; 32]).unwrap());

            // Test batch store
            let mut batch = HashMap::new();

            let mut state1 = AccountState::new();
            state1.balance = 2000;

            let mut state2 = AccountState::new();
            state2.balance = 3000;

            batch.insert([10u8; 32], state1);
            batch.insert([11u8; 32], state2);

            state_store.store_account_states(batch).unwrap();

            let check1 = state_store.get_account_state(&[10u8; 32]).unwrap();
            let check2 = state_store.get_account_state(&[11u8; 32]).unwrap();

            assert_eq!(check1.balance, 2000);
            assert_eq!(check2.balance, 3000);
        }
        // Clean up
        temp_dir.close().unwrap();
    }
}
