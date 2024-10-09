use std::io::{self, Read, Write}; // Importamos `Read` y `Write`
use std::net::TcpStream;

fn main() {
    loop {
        println!("1. GET blockchain");
        println!("2. POST transaction");
        println!("3. Exit");
        print!("Enter choice: ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        let choice = choice.trim();

        match choice {
            "1" => {
                let response = get_blockchain();
                println!("Response: {}", response);
            }
            "2" => {
                let transaction = create_transaction();
                let response = post_transaction(transaction);
                println!("Response: {}", response);
            }
            "3" => {
                println!("Exiting...");
                break;
            }
            _ => {
                println!("Invalid choice, try again.");
            }
        }
    }
}

fn get_blockchain() -> String {
    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    let request = "GET / HTTP/1.1\r\nHost: 127.0.0.1:9090\r\n\r\n"; // Añadir Host
    stream.write_all(request.as_bytes()).unwrap();

    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();

    let response = String::from_utf8_lossy(&buffer);
    response.to_string()
}

fn create_transaction() -> (String, String, f64) {
    let mut sender = String::new();
    let mut recipient = String::new();
    let mut amount = String::new();

    print!("Enter sender: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut sender).unwrap();

    print!("Enter recipient: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut recipient).unwrap();

    print!("Enter amount: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut amount).unwrap();

    let amount: f64 = amount.trim().parse().expect("Invalid amount"); // Manejo de errores

    (
        sender.trim().to_string(),
        recipient.trim().to_string(),
        amount,
    )
}

fn post_transaction(transaction: (String, String, f64)) -> String {
    let mut stream = TcpStream::connect("127.0.0.1:9090").unwrap();
    let (sender, recipient, amount) = transaction;

    // Crear el cuerpo de la solicitud en formato JSON
    let json_body = format!(
        "{{\"sender\": \"{}\", \"recipient\": \"{}\", \"amount\": {}}}",
        sender, recipient, amount
    );

    // Crear la solicitud POST correctamente
    let request = format!(
        "POST / HTTP/1.1\r\n\
         Host: 127.0.0.1:9090\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        json_body.len(),
        json_body
    );

    // Enviar la solicitud
    stream.write_all(request.as_bytes()).unwrap();

    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();

    let response = String::from_utf8_lossy(&buffer);
    response.to_string()
}
