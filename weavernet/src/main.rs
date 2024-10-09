use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Transaction {
    sender: String,
    recipient: String,
    amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Block {
    index: u32,
    previous_hash: String,
    transactions: Vec<Transaction>,
}

struct Blockchain {
    chain: Vec<Block>,
    current_transactions: Vec<Transaction>,
}

impl Blockchain {
    fn new() -> Blockchain {
        let mut blockchain = Blockchain {
            chain: vec![],
            current_transactions: vec![],
        };
        // Crear el bloque génesis
        blockchain.new_block("0"); // Suponiendo que el hash del bloque anterior es "0"
        blockchain
    }

    fn new_block(&mut self, previous_hash: &str) -> &Block {
        let block = Block {
            index: (self.chain.len() + 1) as u32,
            previous_hash: previous_hash.to_string(),
            transactions: self.current_transactions.clone(),
        };

        self.current_transactions.clear();
        self.chain.push(block);
        self.chain.last().unwrap()
    }

    fn new_transaction(&mut self, sender: &str, recipient: &str, amount: f64) {
        let transaction = Transaction {
            sender: sender.to_string(),
            recipient: recipient.to_string(),
            amount,
        };
        self.current_transactions.push(transaction);
    }

    fn to_json(&self) -> String {
        serde_json::to_string(&self.chain).unwrap()
    }
}

#[tokio::main]
async fn main() {
    let blockchain = Arc::new(Mutex::new(Blockchain::new()));

    let listener = TcpListener::bind("127.0.0.1:9090").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let blockchain = Arc::clone(&blockchain);

        tokio::spawn(async move {
            let mut buffer = [0; 1024];
            let mut stream = socket;

            match stream.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let request = String::from_utf8_lossy(&buffer[..n]);
                    let parts: Vec<&str> = request.trim().split_whitespace().collect();

                    println!("Received request: {}", request);

                    if parts[0] == "GET" {
                        let blockchain_data = blockchain.lock().unwrap().to_json();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
                            blockchain_data
                        );
                        stream.write_all(response.as_bytes()).await.unwrap();
                    } else if parts[0] == "POST" {
                        // Leer el cuerpo de la solicitud
                        let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
                        let transaction: Result<Transaction, _> = serde_json::from_str(body);

                        match transaction {
                            Ok(tx) => {
                                {
                                    let mut bchain = blockchain.lock().unwrap();
                                    bchain.new_transaction(&tx.sender, &tx.recipient, tx.amount);
                                    bchain.new_block("0"); // Puedes cambiar esto si deseas acumular más transacciones antes de crear un bloque

                                    println!(
                                        "Transaction created: sender = {}, recipient = {}, amount = {}",
                                        tx.sender, tx.recipient, tx.amount
                                    );
                                }

                                let response = format!("HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n{{\"message\": \"Transaction created\"}}");
                                stream.write_all(response.as_bytes()).await.unwrap();
                            }
                            Err(_) => {
                                let response = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n\r\n{{\"error\": \"Invalid request\"}}");
                                stream.write_all(response.as_bytes()).await.unwrap();
                            }
                        }
                    } else {
                        let response = format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n\r\n{{\"error\": \"Invalid request\"}}");
                        stream.write_all(response.as_bytes()).await.unwrap();
                    }
                }
                _ => {}
            }
        });
    }
}
