//! Programa de prueba para verificar el funcionamiento completo de Blocana
//! 
//! Ejecutar con: cargo run --example blockchain_test

use std::collections::HashMap;
use blocana::{
    block::{Block, BlockHeader},
    crypto::KeyPair,
    state::BlockchainState,
    storage::{BlockchainStorage, StorageConfig},
    transaction::Transaction,
    types::{Hash, PublicKeyBytes},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configura logging
    env_logger::init();
    println!("Iniciando prueba integral de Blocana...");
    
    // 1. Configurar almacenamiento temporal para la prueba
    let test_db_path = "test_blockchain_db";
    if Path::new(test_db_path).exists() {
        std::fs::remove_dir_all(test_db_path)?;
    }
    
    let config = StorageConfig {
        db_path: test_db_path.into(),
        ..Default::default()
    };
    
    // 2. Inicializar almacenamiento
    let storage = BlockchainStorage::open(&config)?;
    println!("✅ Almacenamiento inicializado correctamente");
    
    // 3. Generar keypairs para la prueba
    println!("Generando cuentas de prueba...");
    let validator_keypair = KeyPair::generate()?;
    let user1_keypair = KeyPair::generate()?;
    let user2_keypair = KeyPair::generate()?;
    
    // 4. Crear estado inicial con saldos
    let mut initial_balances = HashMap::new();
    initial_balances.insert(user1_keypair.public_key, 1000);
    initial_balances.insert(user2_keypair.public_key, 500);
    
    let mut state = BlockchainState::genesis_state(initial_balances);
    println!("✅ Estado inicial creado con saldos para las cuentas");
    
    // 5. Crear bloque génesis
    let genesis_block = Block::genesis(validator_keypair.public_key, Vec::new())?;
    let genesis_hash = genesis_block.header.hash();
    
    // 6. Almacenar bloque génesis
    storage.store_block(&genesis_block)?;
    println!("✅ Bloque génesis creado y almacenado con hash: {}", hex::encode(genesis_hash));
    
    // 7. Crear una transacción de user1 a user2
    println!("Creando transacción de prueba...");
    let mut tx = Transaction::new(
        user1_keypair.public_key, // sender
        user2_keypair.public_key, // recipient
        100,                      // amount
        10,                       // fee
        0,                        // nonce inicial
        vec![],                   // data
    );
    
    // 8. Firmar la transacción
    tx.sign(&user1_keypair.private_key)?;
    println!("✅ Transacción creada y firmada");
    
    // 9. Verificar la transacción
    tx.verify()?;
    println!("✅ Firma de transacción verificada");
    
    // 10. Crear bloque con la transacción
    let block1 = Block::new(
        genesis_hash,
        1,
        vec![tx],
        validator_keypair.public_key,
    )?;
    
    // 11. Firmar el bloque
    let mut header = block1.header.clone();
    header.sign(&validator_keypair.private_key)?;
    
    let block1 = Block {
        header,
        transactions: block1.transactions,
    };
    
    // 12. Verificar el bloque
    block1.validate()?;
    println!("✅ Bloque 1 creado y validado");
    
    // 13. Almacenar el bloque
    storage.store_block(&block1)?;
    println!("✅ Bloque 1 almacenado con éxito");
    
    // 14. Aplicar el bloque al estado
    state.apply_block(&block1)?;
    println!("✅ Estado actualizado con transacción del bloque 1");
    
    // 15. Verificar el estado resultante
    {
        // Process one account state at a time
        {
            let user1_state = state.get_account_state(&user1_keypair.public_key);
            println!("Estado final de User1: {} (nonce: {})", user1_state.balance, user1_state.nonce);
            assert_eq!(user1_state.balance, 890); // 1000 - 100 - 10 fee
            assert_eq!(user1_state.nonce, 1);     // incrementado a 1
        } // First mutable borrow ends here
    
        {
            let user2_state = state.get_account_state(&user2_keypair.public_key);
            println!("Estado final de User2: {}", user2_state.balance);
            assert_eq!(user2_state.balance, 600); // 500 + 100
        } // Second mutable borrow ends here
    
        println!("✅ Estado final verificado correctamente");
    }
    
    // 16. Recuperar bloques de la base de datos
    let block_from_db = storage.get_block(&block1.header.hash())?.expect("Bloque debe existir");
    assert_eq!(block_from_db.header.hash(), block1.header.hash());
    println!("✅ Bloque recuperado de la base de datos correctamente");
    
    // 17. Verificar la continuidad de la cadena
    let height_1_hash = storage.get_block_hash_by_height(1)?;
    assert_eq!(height_1_hash, block1.header.hash());
    println!("✅ Continuidad de la cadena verificada correctamente");
    
    // 18. Verificar integridad de la base de datos
    assert!(storage.verify_integrity()?);
    println!("✅ Integridad de la base de datos verificada");
    
    // Limpiar recursos
    std::fs::remove_dir_all(test_db_path)?;
    println!("\n✅ Prueba integral completada con éxito! La blockchain funciona correctamente.");

    Ok(())
}
