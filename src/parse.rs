use imf_codec::header;
use std::io;
use std::io::Read;

use chardetng::EncodingDetector;
use encoding_rs::Encoding;

fn main() {
    // Read full mail in memory
    let mut rawmail = Vec::new();
    io::stdin().lock().read_to_end(&mut rawmail).unwrap();

    // Create detector
    let mut detector = EncodingDetector::new();
    detector.feed(&rawmail, true);
    
    // Get encoding
    let enc: &Encoding = detector.guess(None, true);
    let (email, encoding, malformed) = enc.decode(&rawmail);
    println!("Encoding: {:?}, Malformed: {:?}", encoding, malformed);

    let (_, hdrs) = header::section(&email).unwrap();
    assert!(hdrs.date.is_some());
    assert!(hdrs.from.len() > 0);

    println!("{:?}", hdrs);
}
