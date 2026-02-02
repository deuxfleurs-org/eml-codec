use std::io;
use std::io::Read;

fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    let eml = eml_codec::parse_message(&rawmail);
    eprintln!("--- message structure ---\n{:#?}\n--- message structure end ---", eml);
    let bytes = eml_codec::print_message(eml, None);
    print!("{}", String::from_utf8_lossy(&bytes));
}
