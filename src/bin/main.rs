use blocana::{Blockchain, BlockchainConfig}; // Quitamos Transaction ya que no se usa
use std::process;
use std::io::{self, BufRead, Write};
use clap::{App, Arg}; // Quitamos SubCommand ya que no se usa
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    // Parse command line arguments
    let matches = App::new("Blocana")
        .version(blocana::VERSION)
        .author("Encrypia Labs")
        .about("A lightweight blockchain for resource-constrained environments")
        .arg(Arg::with_name("node")
            .long("node")
            .help("Run as a full node"))
        .arg(Arg::with_name("light")
            .long("light")
            .help("Run as a light client"))
        .arg(Arg::with_name("port")
            .long("port")
            .takes_value(true)
            .default_value("8080")
            .help("Port to listen on"))
        .arg(Arg::with_name("connect")
            .long("connect")
            .takes_value(true)
            .help("Address of node to connect to"))
        .arg(Arg::with_name("interactive")
            .long("interactive")
            .short('i') // Cambiado de "i" a 'i' para corregir el error
            .help("Run in interactive mode"))
        .get_matches();

    // Configure the blockchain
    let mut config = BlockchainConfig::default();
    
    // Apply command line options to config
    if let Some(port) = matches.value_of("port") {
        if let Ok(port_num) = port.parse::<u16>() {
            config.network_config.listen_port = port_num;
        } else {
            eprintln!("Invalid port number");
            process::exit(1);
        }
    }
    
    // Store the port before moving config
    let listen_port = config.network_config.listen_port;
    
    // Create and start the blockchain
    match Blockchain::new(config) {
        Ok(blockchain) => {
            println!("Blocana node starting...");
            
            // Wrap the blockchain in an Arc<Mutex> so it can be shared between threads
            let blockchain = Arc::new(Mutex::new(blockchain));
            
            // Start the blockchain in a separate thread
            let blockchain_clone = blockchain.clone();
            thread::spawn(move || {
                let mut bc = blockchain_clone.lock().unwrap();
                if let Err(e) = bc.start() {
                    eprintln!("Failed to start blockchain: {:?}", e);
                    process::exit(1);
                }
            });
            
            println!("Blocana node running on port {}", listen_port);
            
            // If interactive mode is enabled, start the CLI
            if matches.is_present("interactive") {
                run_interactive_cli(blockchain);
            } else {
                // Keep the main thread alive
                loop {
                    thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize blockchain: {:?}", e);
            process::exit(1);
        }
    }
}

// Interactive CLI for Blocana
fn run_interactive_cli(blockchain: Arc<Mutex<Blockchain>>) {
    println!("Welcome to Blocana Interactive CLI");
    println!("Type 'help' for available commands");
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    loop {
        print!("blocana> ");
        stdout.flush().unwrap();
        
        let mut input = String::new();
        stdin.lock().read_line(&mut input).unwrap();
        
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        
        match parts[0] {
            "help" => {
                println!("Available commands:");
                println!("  block create                - Generate a new block");
                println!("  tx create <to> <amount>     - Create a new transaction");
                println!("  status                      - Show blockchain status");
                println!("  peers                       - List connected peers");
                println!("  quit                        - Exit the program");
            }
            "block" => {
                if parts.len() > 1 && parts[1] == "create" {
                    println!("Creating a new block...");
                    let mut bc = blockchain.lock().unwrap();
                    match bc.generate_block() {
                        Ok(block) => println!("Block created: height={}, transactions={}", 
                            block.header.height, block.transactions.len()),
                        Err(e) => println!("Failed to create block: {:?}", e),
                    }
                } else {
                    println!("Unknown sub-command. Use 'block create'");
                }
            }
            "tx" => {
                if parts.len() >= 4 && parts[1] == "create" {
                    let to = parts[2];
                    if let Ok(amount) = parts[3].parse::<u64>() {
                        println!("Creating transaction to {} with amount {}", to, amount);
                        
                        // Create a placeholder recipient (in a real app, we'd parse an address)
                        let mut recipient = [0u8; 32];
                        let bytes = to.as_bytes();
                        for (i, &byte) in bytes.iter().enumerate().take(32) {
                            recipient[i] = byte;
                        }
                        
                        let mut bc = blockchain.lock().unwrap();
                        match bc.create_transaction(recipient, amount) {
                            Ok(_) => println!("Transaction created successfully"),
                            Err(e) => println!("Failed to create transaction: {:?}", e),
                        }
                    } else {
                        println!("Invalid amount");
                    }
                } else {
                    println!("Usage: tx create <to> <amount>");
                }
            }
            "status" => {
                let bc = blockchain.lock().unwrap();
                bc.print_status();
            }
            "peers" => {
                let bc = blockchain.lock().unwrap();
                bc.print_peers();
            }
            "quit" => {
                println!("Exiting Blocana");
                break;
            }
            _ => {
                println!("Unknown command. Type 'help' for available commands");
            }
        }
    }
}
