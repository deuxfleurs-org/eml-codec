use imf_codec::multipass;
use std::io;
use std::io::Read;


fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    // Parse it
    let segment = multipass::segment::Segment::try_from(&rawmail[..]).unwrap();
    let charng = multipass::guess_charset::GuessCharset::from(segment);
    let extr = multipass::extract_fields::ExtractFields::try_from(&charng).unwrap();
    let lazy = multipass::field_lazy::Parsed::from(extr);
    let eager = multipass::field_eager::Parsed::from(lazy);
    let section = multipass::header_section::Parsed::from(eager);
    //let section: multipass::header_section::Parsed = rawmail.as_ref().into();
    //let (email, encoding, malformed) = header::from_bytes(&rawmail);
    //println!("Encoding: {:?}, Malformed: {:?}", encoding, malformed);

    //let (input, hdrs) = header::section(&email).unwrap();

    // Checks/debug
    println!("{:?}", section);
    //assert!(hdrs.date.is_some());
    //assert!(hdrs.from.len() > 0);
    //assert!(hdrs.bad_fields.len() == 0);
}
