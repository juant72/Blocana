use blocana::{Blockchain, BlockchainConfig};
use std::process;
use clap::{App, Arg};

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
    
    // Create and start the blockchain
    match Blockchain::new(config) {
        Ok(mut blockchain) => {
            println!("Blocana node starting...");
            
            if let Err(e) = blockchain.start() {
                eprintln!("Failed to start blockchain: {:?}", e);
                process::exit(1);
            }
            
            println!("Blocana node running on port {}", config.network_config.listen_port);
            
            // Keep the main thread alive
            // In a real implementation, we would handle signals properly
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize blockchain: {:?}", e);
            process::exit(1);
        }
    }
}
