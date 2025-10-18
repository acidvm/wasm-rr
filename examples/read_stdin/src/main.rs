use std::io::{self, Read};

fn main() {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(_) => {
            let content = buffer.escape_default().to_string();
            let byte_len = buffer.len();
            println!("read {byte_len} bytes from stdin");
            if byte_len > 0 {
                println!("stdin content: {content}");
            } else {
                println!("stdin content: <empty>");
            }
        }
        Err(err) => {
            eprintln!("failed to read from stdin: {err}");
        }
    }
}
