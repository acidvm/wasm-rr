use std::io::{self, Read};

/// Read all data from stdin upfront for recording
pub fn read_all_stdin() -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(buffer)
}
