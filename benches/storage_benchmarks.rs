//! Benchmarks for storage operations in Blocana
//! 
//! Run with: cargo bench --bench storage_benchmarks
//! 
//! These benchmarks measure the performance of core storage operations
//! under various workloads and configurations.

#![feature(test)]
extern crate test;

use test::Bencher;
use blocana::{
    block::Block,
    storage::{BlockchainStorage, StorageConfig},
    transaction::Transaction,
    types::{Hash, PublicKeyBytes},
};

use tempfile::TempDir;

// Helper to create a test block
fn create_test_block(height: u64, prev_hash: Hash, tx_count: usize) -> Block {
    let validator = [0u8; 32];
    
    // Create test transactions
    let mut transactions = Vec::with_capacity(tx_count);
    for i in 0..tx_count {
        let mut sender = [0u8; 32];
        let mut recipient = [0u8; 32];
        sender[0] = (i % 256) as u8;
        recipient[0] = ((i + 1) % 256) as u8;
        
        let mut tx = Transaction::new(
            sender,
            recipient,
            100,
            10,
            i as u64, // use index as nonce
            vec![],
        );
        
        // Set a dummy signature
        tx.signature = [1u8; 64];
        
        transactions.push(tx);
    }
    
    let mut block = Block::new(
        prev_hash,
        height,
        transactions,
        validator,
    ).unwrap();
    
    // Set a dummy signature for the block
    block.header.signature = [1u8; 64];
    
    block
}

// Setup benchmark environment
fn setup_benchmark_db() -> (TempDir, BlockchainStorage) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().to_str().unwrap().to_string();
    
    let config = StorageConfig {
        db_path,
        max_open_files: 1000,
        write_buffer_size: 64 * 1024 * 1024, // 64MB
        max_write_buffer_number: 3,
        target_file_size_base: 64 * 1024 * 1024, // 64MB
        cache_size: 128 * 1024 * 1024, // 128MB
    };
    
    let storage = BlockchainStorage::open(&config).unwrap();
    
    (temp_dir, storage)
}

#[bench]
fn bench_store_block_small(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    let block = create_test_block(1, [0u8; 32], 10); // Small block with 10 transactions
    
    b.iter(|| {
        storage.store_block(&block).unwrap();
    });
}

#[bench]
fn bench_store_block_medium(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    let block = create_test_block(1, [0u8; 32], 100); // Medium block with 100 transactions
    
    b.iter(|| {
        storage.store_block(&block).unwrap();
    });
}

#[bench]
fn bench_store_block_large(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    let block = create_test_block(1, [0u8; 32], 1000); // Large block with 1000 transactions
    
    b.iter(|| {
        storage.store_block(&block).unwrap();
    });
}

#[bench]
fn bench_retrieve_block_by_hash(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create and store a block
    let block = create_test_block(1, [0u8; 32], 100);
    let block_hash = block.header.hash();
    storage.store_block(&block).unwrap();
    
    b.iter(|| {
        let _retrieved_block = storage.get_block(&block_hash).unwrap();
    });
}

#[bench]
fn bench_retrieve_block_by_height(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create and store a block
    let block = create_test_block(42, [0u8; 32], 100);
    storage.store_block(&block).unwrap();
    
    b.iter(|| {
        let _retrieved_block = storage.get_block_by_height(42).unwrap();
    });
}

#[bench]
fn bench_retrieve_transaction(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create and store a block with transactions
    let block = create_test_block(1, [0u8; 32], 100);
    
    // Get a specific transaction hash
    let tx_hash = block.transactions[50].hash();
    
    // Store the block
    storage.store_block(&block).unwrap();
    
    b.iter(|| {
        let _tx = storage.get_transaction(&tx_hash).unwrap();
    });
}

#[bench]
fn bench_store_account_state(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create an account state
    let mut state = blocana::state::AccountState::new();
    state.balance = 1000;
    state.nonce = 5;
    
    let address = [1u8; 32];
    
    b.iter(|| {
        storage.store_account_state(&address, &state).unwrap();
    });
}

#[bench]
fn bench_retrieve_account_state(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create and store an account state
    let mut state = blocana::state::AccountState::new();
    state.balance = 1000;
    state.nonce = 5;
    
    let address = [1u8; 32];
    storage.store_account_state(&address, &state).unwrap();
    
    b.iter(|| {
        let _retrieved_state = storage.get_account_state(&address).unwrap();
    });
}

#[bench]
fn bench_block_chain_iteration(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create a chain of 100 blocks
    let mut prev_hash = [0u8; 32];
    for i in 1..=100 {
        let block = create_test_block(i, prev_hash, 10);
        prev_hash = block.header.hash();
        storage.store_block(&block).unwrap();
    }
    
    // Benchmark iterating through the chain
    b.iter(|| {
        let mut current_height = 100;
        let mut current_hash = storage.get_block_hash_by_height(current_height).unwrap();
        
        while current_height > 0 {
            let block = storage.get_block(&current_hash).unwrap().unwrap();
            current_hash = block.header.prev_hash;
            current_height -= 1;
        }
    });
}

#[bench]
fn bench_chain_integrity_check(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Crear el bloque genesis con prev_hash de ceros
    let genesis_block = create_test_block(0, [0u8; 32], 5);
    let mut prev_hash = genesis_block.header.hash();
    
    // Almacenar el bloque genesis primero
    storage.store_block(&genesis_block).unwrap();
    
    // Crear una cadena de 49 bloques adicionales (total 50)
    for i in 1..50 {
        let block = create_test_block(i, prev_hash, 5);
        prev_hash = block.header.hash();
        storage.store_block(&block).unwrap();
    }
    
    // Verificar que la integridad funciona antes de comenzar el benchmark
    assert!(storage.verify_integrity().unwrap(), "Chain integrity check failed before benchmarking");
    
    b.iter(|| {
        let integrity_result = storage.verify_integrity().unwrap();
        test::black_box(integrity_result);
    });
}

#[bench]
fn bench_batch_account_updates(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Create 100 account states to update in a batch
    let addresses: Vec<PublicKeyBytes> = (0..100)
        .map(|i| {
            let mut addr = [0u8; 32];
            addr[0] = (i % 256) as u8;
            addr[1] = ((i / 256) % 256) as u8;
            addr
        })
        .collect();
    
    b.iter(|| {
        let cfs = storage.get_column_families().unwrap();
        let mut batch = rocksdb::WriteBatch::default();
        
        for (i, addr) in addresses.iter().enumerate() {
            let mut state = blocana::state::AccountState::new();
            state.balance = i as u64 * 1000;
            state.nonce = i as u64;
            
            let state_bytes = bincode::encode_to_vec(&state, bincode::config::standard()).unwrap();
            // Pass both key and value as byte slices using .as_ref()
            batch.put_cf(cfs.account_state, addr.as_ref(), state_bytes.as_slice());
        }
        
        storage.raw_db().write(batch).unwrap();
    });
}

// Añadir feature gate para las funciones dependientes del timestamp_index
#[bench]
#[cfg(feature = "timestamp_index")]
fn bench_query_by_timestamp(b: &mut Bencher) {
    let (_temp_dir, storage) = setup_benchmark_db();
    
    // Crear bloques con timestamps para probar
    // ...
    
    b.iter(|| {
        let _blocks = storage.get_blocks_by_time_range(1617235200000, 1617235500000, 10).unwrap();
    });
}
