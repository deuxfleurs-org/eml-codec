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

use nom::IResult;

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
/// # Returns
///
/// * `rest` - The rest of the buffer, the part that is not parsed as the email ended before the
/// end of the data
/// * `msg` - The parsed message
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
/// let (_, email) = eml_codec::email(input).unwrap();
/// println!(
///     "{} raw message is:\n{}",
///     email.imf.from[0].to_string(),
///     String::from_utf8_lossy(email.child.as_text().unwrap().body),
/// );
/// ```
pub fn email(input: &[u8]) -> IResult<&[u8], part::composite::Message> {
    part::composite::message(mime::MIME::<mime::r#type::Message>::default())(input)
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
/// # Returns
///
/// * `rest` - The rest of the buffer, ie. the body of your email as raw bytes
/// * `imf` - The parsed IMF headers of your email
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
/// let (_, imf) = eml_codec::imf(input).unwrap();
/// println!(
///     "{} just sent you an email with subject \"{}\"",
///     imf.from[0].to_string(),
///     imf.subject.unwrap().to_string(),
/// );
/// ```
pub fn imf(input: &[u8]) -> IResult<&[u8], imf::Imf> {
    imf::field::imf(input)
}
