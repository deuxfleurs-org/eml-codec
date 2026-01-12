//use imf_codec::fragments::section::Section;
//use imf_codec::multipass::segment;
use std::io;
use std::io::Read;

fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    let (_, eml) = eml_codec::parse_message(&rawmail).unwrap();
    println!("{:#?}", eml)
}
