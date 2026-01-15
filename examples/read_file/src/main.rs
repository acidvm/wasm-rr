use std::fs;

fn main() {
    // Read the file passed as first argument, or "input.txt" by default
    let args: Vec<String> = std::env::args().collect();
    let filename = args.get(1).map(|s| s.as_str()).unwrap_or("input.txt");
    
    match fs::read_to_string(filename) {
        Ok(contents) => {
            println!("File contents ({} bytes):", contents.len());
            println!("{}", contents);
        }
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            std::process::exit(1);
        }
    }
}
