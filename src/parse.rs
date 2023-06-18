use imf_codec::header;
use std::io;
use std::io::Read;

fn main() {
    let mut email = String::new();
    io::stdin().lock().read_to_string(&mut email).unwrap();

    let (_, hdrs) = header::section(&email).unwrap();
    assert!(hdrs.date.is_some());
    assert!(hdrs.from.len() > 0);

    println!("{:?}", hdrs);
}
