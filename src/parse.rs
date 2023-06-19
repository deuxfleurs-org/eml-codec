use imf_codec::fragments::header;
use std::io;
use std::io::Read;


fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    // Parse it
    let (email, encoding, malformed) = header::from_bytes(&rawmail);
    println!("Encoding: {:?}, Malformed: {:?}", encoding, malformed);

    let (input, hdrs) = header::section(&email).unwrap();

    // Checks/debug
    println!("{:?}", hdrs);
    assert!(hdrs.date.is_some());
    assert!(hdrs.from.len() > 0);
    assert!(hdrs.bad_fields.len() == 0);
}
