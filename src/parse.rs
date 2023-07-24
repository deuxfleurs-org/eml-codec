//use imf_codec::fragments::section::Section;
//use imf_codec::multipass::segment;
use std::io;
use std::io::Read;

fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    let eml = eml_codec::email(&rawmail).unwrap();
    println!("{:#?}", eml);
    assert!(eml.1.date.is_some());
    assert!(!eml.1.from.is_empty());
}
