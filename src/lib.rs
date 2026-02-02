#![doc = include_str!("../README.md")]

/// Parse and represent full "top-level" emails (RFC 822, RFC 2045, RFC 2046)
pub mod message;

/// Parse and represent emails "parts" as defined by MIME (RFC 2046)
pub mod part;

/// Parse and represent IMF (Internet Message Format) headers (RFC 822, RFC 5322)
pub mod imf;

/// Parse and represent MIME headers (RFC 2045, RFC 2047)
pub mod mime;

/// MIME and IMF represent headers the same way: module contains their common logic
pub mod header;

/// Low-level email-specific text-based representation for data
pub mod text;

/// Printing with email-specific line folding
pub mod print;

mod utils;

use crate::print::Print;
use nom::IResult;

// FIXME: in case of success there is no remaining input; update the comment
// and return type?
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
/// let (_, email) = eml_codec::parse_message(input).unwrap();
/// println!(
///     "{} message structure is:\n{:#?}",
///     email.imf.from_or_sender().to_string(),
///     email,
/// );
/// ```
pub fn parse_message(input: &[u8]) -> IResult<&[u8], message::Message<'_>> {
    message::message(input)
}

/// Print a whole email.
///
/// The `seed` parameter controls the RNG used to generate multipart boundaries.
/// Passing `None` will use randomness from the operating system.
pub fn print_message(msg: message::Message<'_>, seed: Option<u64>) -> Vec<u8> {
    print::with_formatter(seed, |fmt| msg.print(fmt))
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
/// let (_, imf) = eml_codec::parse_imf(input).unwrap();
/// println!(
///     "{} just sent you an email with subject \"{}\"",
///     imf.from_or_sender().to_string(),
///     imf.subject.unwrap().to_string(),
/// );
/// ```
pub fn parse_imf(input: &[u8]) -> IResult<&[u8], imf::Imf<'_>> {
    message::imf(input)
}
