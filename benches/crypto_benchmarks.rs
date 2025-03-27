//! Benchmarks for cryptographic operations
//! 
//! Run with: cargo bench

#![feature(test)]
extern crate test;

use test::Bencher;
use blocana::crypto;
use blocana::types::{Hash, SignatureBytes};

#[bench]
fn bench_hash_data_small(b: &mut Bencher) {
    let data = [1u8; 32]; // Small input (32 bytes)
    b.iter(|| {
        crypto::hash_data(&data)
    });
}

#[bench]
fn bench_hash_data_medium(b: &mut Bencher) {
    let data = [1u8; 1024]; // Medium input (1 KB)
    b.iter(|| {
        crypto::hash_data(&data)
    });
}

#[bench]
fn bench_hash_data_large(b: &mut Bencher) {
    let data = [1u8; 1024 * 10]; // Large input (10 KB)
    b.iter(|| {
        crypto::hash_data(&data)
    });
}

#[bench]
fn bench_keypair_generation(b: &mut Bencher) {
    b.iter(|| {
        let _ = crypto::KeyPair::generate().unwrap();
    });
}

#[bench]
fn bench_sign_message(b: &mut Bencher) {
    let keypair = crypto::KeyPair::generate().unwrap();
    let message = [1u8; 256]; // 256 byte message
    
    b.iter(|| {
        keypair.sign(&message)
    });
}

#[bench]
fn bench_verify_signature(b: &mut Bencher) {
    let keypair = crypto::KeyPair::generate().unwrap();
    let message = [1u8; 256]; // 256 byte message
    let signature = keypair.sign(&message);
    
    b.iter(|| {
        crypto::verify_signature(&keypair.public_key, &signature, &message).unwrap()
    });
}

#[bench]
fn bench_merkle_root_small(b: &mut Bencher) {
    // 8 hashes
    let hashes: Vec<Hash> = (0..8)
        .map(|i| crypto::hash_data(&[i as u8; 32]))
        .collect();
    
    b.iter(|| {
        crypto::compute_merkle_root(&hashes)
    });
}

#[bench]
fn bench_merkle_root_medium(b: &mut Bencher) {
    // 128 hashes
    let hashes: Vec<Hash> = (0..128)
        .map(|i| crypto::hash_data(&[i as u8; 32]))
        .collect();
    
    b.iter(|| {
        crypto::compute_merkle_root(&hashes)
    });
}

#[bench]
fn bench_merkle_root_large(b: &mut Bencher) {
    // 1024 hashes (simulating a large block)
    let hashes: Vec<Hash> = (0..1024)
        .map(|i| crypto::hash_data(&[(i % 256) as u8; 32]))
        .collect();
    
    b.iter(|| {
        crypto::compute_merkle_root(&hashes)
    });
}

#[bench]
fn bench_batch_verify_small(b: &mut Bencher) {
    // Generate test data: 10 signatures
    const SIZE: usize = 10;
    let keypairs: Vec<_> = (0..SIZE).map(|_| crypto::KeyPair::generate().unwrap()).collect();
    let messages: Vec<Vec<u8>> = (0..SIZE).map(|i| vec![(i % 256) as u8; 32]).collect();
    let signatures: Vec<SignatureBytes> = keypairs.iter().zip(messages.iter())
        .map(|(kp, msg)| kp.sign(msg)).collect();
    
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&SignatureBytes> = signatures.iter().collect();
    let pk_refs: Vec<&blocana::types::PublicKeyBytes> = keypairs.iter().map(|kp| &kp.public_key).collect();
    
    b.iter(|| {
        crypto::batch_verify_signatures(&msg_refs, &sig_refs, &pk_refs).unwrap()
    });
}

#[bench]
fn bench_batch_verify_large(b: &mut Bencher) {
    // Generate test data: 100 signatures
    const SIZE: usize = 100;
    let keypairs: Vec<_> = (0..SIZE).map(|_| crypto::KeyPair::generate().unwrap()).collect();
    let messages: Vec<Vec<u8>> = (0..SIZE).map(|i| vec![(i % 256) as u8; 32]).collect();
    let signatures: Vec<SignatureBytes> = keypairs.iter().zip(messages.iter())
        .map(|(kp, msg)| kp.sign(msg)).collect();
    
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&SignatureBytes> = signatures.iter().collect();
    let pk_refs: Vec<&blocana::types::PublicKeyBytes> = keypairs.iter().map(|kp| &kp.public_key).collect();
    
    b.iter(|| {
        crypto::batch_verify_signatures(&msg_refs, &sig_refs, &pk_refs).unwrap()
    });
}

#[bench]
fn bench_compare_individual_vs_batch_verify(b: &mut Bencher) {
    // Generate test data: 20 signatures
    const SIZE: usize = 20;
    let keypairs: Vec<_> = (0..SIZE).map(|_| crypto::KeyPair::generate().unwrap()).collect();
    let messages: Vec<Vec<u8>> = (0..SIZE).map(|i| vec![(i % 256) as u8; 32]).collect();
    let signatures: Vec<SignatureBytes> = keypairs.iter().zip(messages.iter())
        .map(|(kp, msg)| kp.sign(msg)).collect();
    
    // Individual verification
    b.iter(|| {
        for i in 0..SIZE {
            crypto::verify_signature(&keypairs[i].public_key, &signatures[i], &messages[i]).unwrap();
        }
    });
    
    // Benchmark batch verification in a separate test
}
