#![doc = include_str!("../README.md")]

/// Parse and represent full emails as "parts" as defined by MIME (RFC 2046)
pub mod part;

/// Parse and represent IMF (Internet Message Format) headers (RFC 822, RFC 5322)
pub mod imf;

/// Parse and represent MIME headers (RFC 2045, RFC 2047)
pub mod mime;

/// MIME and IMF represent headers the same way: module contains their commong logic
pub mod header;

/// Low-level email-specific text-based representation for data
pub mod text;

/// Error management
pub mod error;

/// Parse a whole email including its (MIME) body
///
/// Returns the parsed content, but also the remaining bytes
/// if the parser stopped before arriving to the end (for example
/// due to a multipart delimiter).
///
/// # Arguments
///
/// * `input` - A buffer of bytes containing your full email
///
/// # Examples
/// 
/// ```
/// let input = br#"Date: 7 Mar 2023 08:00:00 +0200
/// From: deuxfleurs@example.com
/// To: someone_else@example.com
/// Subject: An RFC 822 formatted message
/// MIME-Version: 1.0
/// Content-Type: text/plain; charset=us-ascii
///
/// This is the plain text body of the message. Note the blank line
/// between the header information and the body of the message."#;
///
/// let email = eml_codec::email(input).unwrap();
/// println!(
///     "{} raw message is:\n{}",
///     email.imf.from[0].to_string(),
///     String::from_utf8_lossy(email.child.as_text().unwrap().body),
/// );
/// ```
pub fn email(input: &[u8]) -> Result<part::composite::Message, error::EMLError> {
    part::composite::message(mime::Message::default())(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}

/// Only extract the headers of the email that are part of the Internet Message Format spec
///
/// Emails headers contain MIME and IMF (Internet Message Format) headers.
/// Sometimes you only need to know the recipient or the sender of an email,
/// and are not interested in its content. In this case, you only need to parse the IMF
/// fields and can ignore the MIME headers + the body. This is what this function does.
///
/// # Arguments
///
/// * `input` - A buffer of bytes containing either only the headers of your email or your full
/// email (in both cases, the body will be ignored)
///
/// # Examples
///
/// ```
/// let input = br#"Date: 7 Mar 2023 08:00:00 +0200
/// From: deuxfleurs@example.com
/// To: someone_else@example.com
/// Subject: An RFC 822 formatted message
/// MIME-Version: 1.0
/// Content-Type: text/plain; charset=us-ascii
///
/// This is the plain text body of the message. Note the blank line
/// between the header information and the body of the message."#;
///
/// let header = eml_codec::imf(input).unwrap();
/// println!(
///     "{} just sent you an email with subject \"{}\"",
///     header.from[0].to_string(),
///     header.subject.unwrap().to_string(),
/// );
/// ```
pub fn imf(input: &[u8]) -> Result<imf::Imf, error::EMLError> {
    imf::field::imf(input)
        .map(|(_, v)| v)
        .map_err(error::EMLError::ParseError)
}
