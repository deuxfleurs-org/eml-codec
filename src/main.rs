use nom::{
    IResult, 
    branch::alt,
    combinator::opt,
    character::complete::alphanumeric1,
    character::complete::crlf,
    character::complete::space0,
    bytes::complete::tag,
    bytes::complete::take_while1,
    multi::many0,
    sequence::terminated,
    sequence::tuple,
};

//-------------- ABNF rfc5234

/// Permissive CRLF
///
/// Theoretically, all lines must end with \r\n
/// but mail servers support malformated emails,
/// for example with only \n eol. It works because
/// \r\n is allowed nowhere else, so we also add this support.
fn perm_crlf(input: &str) -> IResult<&str, &str> {
    alt((crlf, tag("\r"), tag("\n")))(input)
}

// Note: WSP = SP / HTAB = %x20 / %x09
// nom::*::space0 = *WSP
// nom::*::space1 = 1*WSP

/// Parse a folding white space
///
/// Folding white space are used for long headers splitted on multiple lines
///
/// ```abnf
/// FWS             =   ([*WSP CRLF] 1*WSP) /  obs-FWS
/// obs-FWS         =   1*WSP *(CRLF 1*WSP)
/// ```
fn fws(input: &str) -> IResult<&str, &str> {
    let (input, _) = opt(terminated(space0, perm_crlf))(input)?;
    // @FIXME: not implemented obs-FWS
    space0(input)
}

/// Sequence of visible chars with the UTF-8 extension
///
/// ```abnf
/// VCHAR   =  %x21-7E
///            ; visible (printing) characters
/// VCHAR   =/  UTF8-non-ascii
/// SEQ     = 1*VCHAR
///```
fn vchar_seq(input: &str) -> IResult<&str, &str> {
   take_while1(|c: char| (c >= '\x21' && c <= '\x7E') || !c.is_ascii())(input)
}

//--------------- HEADERS
/// Optional Fields
///
/// Fields may appear in messages that are otherwise unspecified in this
/// document.  They MUST conform to the syntax of an optional-field.
/// This is a field name, made up of the printable US-ASCII characters
/// except SP and colon, followed by a colon, followed by any text that
/// conforms to the unstructured syntax.
///
/// https://www.rfc-editor.org/rfc/rfc5322.html#section-3.6.8
///
/// ```abnf
/// optional-field  =   field-name ":" unstructured CRLF
/// field-name      =   1*ftext
/// ftext           =   %d33-57 /          ; Printable US-ASCII
///                     %d59-126           ;  characters not including
///                                        ;  ":".
/// ```
#[derive(Debug, PartialEq)]
pub struct OptionalField<'a> {
    pub name: &'a str,
    pub body: String,
}

fn optional_field(input: &str) -> IResult<&str, OptionalField> {
    let (input, name) = take_while1(|c| c >= '\x21' && c <= '\x7E' && c != '\x3A')(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, body) = unstructured(input)?;
    
    Ok((input, OptionalField { name, body }))
}

/// Unstructured header field body
///
/// ```abnf
/// unstructured    =   (*([FWS] VCHAR_SEQ) *WSP) / obs-unstruct
/// ```
fn unstructured(input: &str) -> IResult<&str, String> {
    let (input, _) = many0(tuple((opt(fws), vchar_seq)))(input)?;
    let (input, _) = space0(input)?;
    Ok((input, "FIX ME".to_string()))
}

fn main() {
    let header_fields = "Subject: Hello\r\n World\r\n";
    println!("{:?}", optional_field(header_fields));
}
