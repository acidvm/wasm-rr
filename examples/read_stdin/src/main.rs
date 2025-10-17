use std::io::{self, Read};

fn main() {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(n) => {
            println!("Read {} bytes from stdin", n);
            print!("{}", buffer);
        }
        Err(e) => {
            eprintln!("Error reading stdin: {}", e);
            std::process::exit(1);
        }
    }
}
