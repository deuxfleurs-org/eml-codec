pub fn main() {
    let input = br#"Date: 7 Mar 2023 08:00:00 +0200
From: deuxfleurs@example.com
To: someone_else@example.com
Subject: An RFC 822 formatted message
MIME-Version: 1.0
Content-Type: text/plain; charset=us-ascii

This is the plain text body of the message. Note the blank line
between the header information and the body of the message."#;

    let email = eml_codec::email(input).unwrap();
    println!(
        "{} just sent you an email with subject \"{}\"",
        email.imf.from[0].to_string(),
        email.imf.subject.unwrap().to_string(),
    );
}
