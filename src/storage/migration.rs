//! Database migration utilities for Blocana
//!
//! This module provides tools for managing database schema changes
//! and migrations between different versions of the database.

use super::{BlockchainStorage, Error};
use rocksdb::DB ;

/// Current database schema version
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Migration descriptor for a database schema change
pub struct Migration {
    /// Version this migration upgrades from
    pub from_version: u32,
    /// Version this migration upgrades to
    pub to_version: u32,
    /// Human-readable description of the changes
    pub description: &'static str,
    /// Function that performs the actual migration
    pub migrate_fn: fn(&BlockchainStorage) -> Result<(), Error>,
}

/// Configuration for a database migration
pub struct MigrationConfig {
    /// Whether to perform a backup before migration
    pub backup_before_migration: bool,
    /// Whether to allow skipping versions
    pub allow_version_skipping: bool,
    /// Backup directory (if backing up)
    pub backup_dir: Option<String>,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            backup_before_migration: true,
            allow_version_skipping: false,
            backup_dir: None,
        }
    }
}

/// Gets the schema version from a database
pub fn get_schema_version(db: &DB) -> Result<u32, Error> {
    // Schema version is stored in the metadata column family
    match db.get(b"schema_version")? {
        Some(bytes) => {
            if bytes.len() < 4 {
                return Err(Error::Database("Invalid schema version format".into()));
            }
            
            let mut version_bytes = [0u8; 4];
            version_bytes.copy_from_slice(&bytes[..4]);
            Ok(u32::from_le_bytes(version_bytes))
        },
        None => {
            // If no version is stored, assume it's version 0
            Ok(0)
        }
    }
}

/// Updates the schema version in the database
pub fn set_schema_version(db: &DB, version: u32) -> Result<(), Error> {
    let version_bytes = version.to_le_bytes();
    db.put(b"schema_version", &version_bytes)?;
    Ok(())
}

/// Available migrations between schema versions
pub fn available_migrations() -> Vec<Migration> {
    vec![
        Migration {
            from_version: 0,
            to_version: 1,
            description: "Initialize schema to version 1",
            migrate_fn: |_storage| {
                // Nothing to do for initial version
                Ok(())
            },
        },
        // Example future migration:
        // Migration {
        //     from_version: 1,
        //     to_version: 2,
        //     description: "Add timestamp index",
        //     migrate_fn: |storage| {
        //         // Migration logic would go here
        //         Ok(())
        //     },
        // },
    ]
}

/// Check if a database needs migration and performs any required migrations
pub fn check_and_migrate(
    storage: &BlockchainStorage, 
    config: MigrationConfig
) -> Result<bool, Error> {
    let db = storage.raw_db();
    let current_version = get_schema_version(db)?;
    
    if current_version == CURRENT_SCHEMA_VERSION {
        // No migration needed
        return Ok(false);
    }
    
    if current_version > CURRENT_SCHEMA_VERSION {
        return Err(Error::Database(format!(
            "Database schema version {} is newer than supported version {}",
            current_version, CURRENT_SCHEMA_VERSION
        )));
    }
    
    // Create backup if requested
    if config.backup_before_migration {
        let backup_dir = config.backup_dir
            .unwrap_or_else(|| format!("{}_backup_v{}", 
                std::env::var("BLOCANA_DATA_DIR").unwrap_or_else(|_| "data".to_string()), 
                current_version
            ));
        
        storage.create_backup(&backup_dir)?;
    }
    
    // Get available migrations
    let migrations = available_migrations();
    
    // Build migration path
    let mut path = Vec::new();
    let mut version = current_version;
    
    while version < CURRENT_SCHEMA_VERSION {
        // Find the next migration
        let next = migrations.iter().find(|m| m.from_version == version);
        
        match next {
            Some(migration) => {
                path.push(migration);
                version = migration.to_version;
            },
            None => {
                if config.allow_version_skipping {
                    // Try to find a migration that can skip versions
                    let skip_migration = migrations.iter()
                        .find(|m| m.from_version < version && m.to_version > version);
                    
                    match skip_migration {
                        Some(migration) => {
                            path.push(migration);
                            version = migration.to_version;
                        },
                        None => {
                            return Err(Error::Database(format!(
                                "No migration path from version {} to {}",
                                current_version, CURRENT_SCHEMA_VERSION
                            )));
                        }
                    }
                } else {
                    return Err(Error::Database(format!(
                        "No migration available for version {} (and skipping not allowed)",
                        version
                    )));
                }
            }
        }
    }
    
    // Execute migrations
    for migration in path {
        log::info!("Migrating database from v{} to v{}: {}",
            migration.from_version,
            migration.to_version,
            migration.description
        );
        
        // Execute the migration
        (migration.migrate_fn)(storage)?;
        
        // Update schema version
        set_schema_version(db, migration.to_version)?;
        
        log::info!("Migration to v{} completed successfully",
            migration.to_version
        );
    }
    
    Ok(true)
}

/// Verify database compatibility and migrate if needed
pub fn ensure_compatible_schema(storage: &BlockchainStorage) -> Result<(), Error> {
    let migrated = check_and_migrate(storage, MigrationConfig::default())?;
    
    if migrated {
        log::info!("Database successfully migrated to schema version {}", 
            CURRENT_SCHEMA_VERSION);
    } else {
        log::debug!("Database schema is already at version {}, no migration needed", 
            CURRENT_SCHEMA_VERSION);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use rocksdb::Options; 
    use super::super::StorageConfig;
    
    #[test]
    fn test_schema_version() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();
        
        // Create a new database
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, &db_path).unwrap();
        
        // Initial schema version should be 0
        assert_eq!(get_schema_version(&db).unwrap(), 0);
        
        // Set and verify schema version
        set_schema_version(&db, 42).unwrap();
        assert_eq!(get_schema_version(&db).unwrap(), 42);
        
        // Clean up
        drop(db);
        temp_dir.close().unwrap();
    }
    
    #[test]
    fn test_migration_path() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap().to_string();
        
        // Configure and open storage
        let config = StorageConfig {
            db_path,
            ..Default::default()
        };
        
        let storage = BlockchainStorage::open(&config).unwrap();
        
        // Set initial schema version to 0
        set_schema_version(storage.raw_db(), 0).unwrap();
        
        // Run migration
        let migrated = check_and_migrate(&storage, MigrationConfig::default()).unwrap();
        assert!(migrated);
        
        // Verify new schema version
        assert_eq!(get_schema_version(storage.raw_db()).unwrap(), CURRENT_SCHEMA_VERSION);
        
        // Subsequent migration should do nothing
        let migrated_again = check_and_migrate(&storage, MigrationConfig::default()).unwrap();
        assert!(!migrated_again);
        
        // Clean up
        drop(storage);
        temp_dir.close().unwrap();
    }
}
