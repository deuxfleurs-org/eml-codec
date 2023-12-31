pub fn main() {
    let input = br#"Date: 7 Mar 2023 08:00:00 +0200
From: deuxfleurs@example.com
To: someone_else@example.com
Subject: An RFC 822 formatted message
MIME-Version: 1.0
Content-Type: text/plain; charset=us-ascii

This is the plain text body of the message. Note the blank line
between the header information and the body of the message."#;

    // if you are only interested in email metadata/headers
    let (_, imf) = eml_codec::parse_imf(input).unwrap();
    println!(
        "{} just sent you an email with subject \"{}\"",
        imf.from[0].to_string(),
        imf.subject.unwrap().to_string(),
    );

    // if you like to also parse the body/content
    let (_, email) = eml_codec::parse_message(input).unwrap();
    println!(
        "{} raw message is:\n{}",
        email.imf.from[0].to_string(),
        String::from_utf8_lossy(email.child.as_text().unwrap().body),
    );
}
