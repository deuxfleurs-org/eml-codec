#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
use bounded_static::{IntoBoundedStatic, ToBoundedStatic, ToStatic};
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{map, opt},
    multi::many0,
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
#[cfg(feature = "tracing")]
use tracing::warn;

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use crate::i18n::ContainsUtf8;
use crate::print::{Formatter, Print, ToStringFromPrint};
use crate::text::charset::EmailCharset;
use crate::text::misc_token::{mime_word, MIMEWord};
use crate::text::quoted::{print_quoted, QuotedString};
use crate::text::recovery::take_quoted_or_until;
use crate::text::whitespace::cfws;
use crate::text::words::{mime_atom, MIMEAtom};
#[cfg(any(feature = "tracing-recover", feature = "tracing-unsupported"))]
use crate::utils::bytes_to_trace_string;

// --------- NAIVE TYPE
#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct NaiveType<'a> {
    pub main: MIMEAtom<'a>,
    pub sub: MIMEAtom<'a>,
    pub params: Vec<Parameter<'a>>,
}
impl<'a> NaiveType<'a> {
    pub fn to_type(&self) -> AnyType<'a> {
        self.into()
    }
}
pub fn naive_type(input: &[u8]) -> IResult<&[u8], NaiveType<'_>> {
    let (input, (main, sub)) = alt((
        separated_pair(mime_atom, tag("/"), mime_atom),
        // Recognize some broken content-types found in the real world:
        recover_broken_type(b"text", b"text", b"plain"),
        recover_broken_type(b".pdf", b"application", b"pdf"),
    ))(input)?;
    let (input, params) = parameter_list(input)?;
    Ok((input, NaiveType { main, sub, params }))
}
pub fn recover_broken_type<'a>(
    broken_name: &'a [u8],
    main: &'a [u8],
    sub: &'a [u8],
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], (MIMEAtom<'a>, MIMEAtom<'a>)> {
    move |input: &[u8]| {
        map(delimited(opt(cfws), tag(broken_name), opt(cfws)), |_| {
            #[cfg(feature = "tracing-recover")]
            warn!(
                "use of broken content-type {}, interpreted as {}/{}",
                String::from_utf8_lossy(broken_name),
                String::from_utf8_lossy(main),
                String::from_utf8_lossy(sub)
            );
            (MIMEAtom(main.into()), MIMEAtom(sub.into()))
        })(input)
    }
}

// XXX we allow printing content types without further validation;
// this is not strictly allowed by the spec, which only allows
// x-token or ietf-token on top of the RFC defined content types.
impl<'a> Print for NaiveType<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.main.print(fmt);
        fmt.write_bytes(b"/");
        self.sub.print(fmt);
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct Parameter<'a> {
    pub name: MIMEAtom<'a>,
    pub value: MIMEWord<'a>,
}
impl<'a> Print for Parameter<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.name.print(fmt);
        fmt.write_bytes(b"=");
        self.value.print(fmt)
    }
}

/// Parses a parameter list that follows a content-type.
///
/// The RFC parameter-list syntax is:
/// ```abnf
///   parameter-list   =  *(";" mime-atom "=" mime-word)
/// ```
///
/// Additionally, we handle partially broken parameter lists, where some
/// segments (delimited by ";") contain invalid data. We drop invalid segments
/// and keep the rest.
///
/// We thus parse the following grammar:
/// ```abnf
///   parameter-list   =   *(";" (mime-atom "=" mime-word / any-not-semicolon)) [";"]
/// ```
/// As a consequence, this combinator always consumes all of its input.
pub fn parameter_list(input: &[u8]) -> IResult<&[u8], Vec<Parameter<'_>>> {
    // recovery parser: skips over junk until the next ';'
    let junk = |input| {
        pair(
            opt(cfws),
            map(take_quoted_or_until(|c| c == b';'), |i| {
                #[cfg(feature = "tracing-unsupported")]
                if !i.is_empty() {
                    warn!(input = %bytes_to_trace_string(i),
                          "unsupported segment in parameter list");
                }
                i
            }),
        )(input)
    };
    let (input, params) = terminated(
        many0(preceded(pair(junk, tag(";")), opt(parameter))),
        pair(opt(tag(";")), junk),
    )(input)?;

    Ok((input, params.into_iter().flatten().collect()))
}
pub fn parameter(input: &[u8]) -> IResult<&[u8], Parameter<'_>> {
    // We handle both '=' and ':' as separators. ':' is not valid but
    // occurs in some emails we want to support...
    let separator = alt((
        tag("="),
        map(tag(":"), |i| {
            #[cfg(feature = "tracing-recover")]
            warn!(input = %bytes_to_trace_string(input),
                  "non-compliant use of ':' instead of '=' in parameter");
            i
        }),
    ));

    map(
        tuple((mime_atom, separator, mime_word)),
        |(name, _, value)| Parameter { name, value },
    )(input)
}

// MIME TYPES TRANSLATED TO RUST TYPING SYSTEM

#[derive(Clone, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub enum AnyType<'a> {
    // Composite types
    Multipart(Multipart<'a>), // multipart/*
    Message(Message<'a>),     // message/{rfc822, global}

    // Discrete types
    Text(Text<'a>),     // text/*
    Binary(Binary<'a>), // everything else
}

impl<'a> AnyType<'a> {
    pub fn params(&self) -> Vec<Parameter<'a>> {
        match self {
            AnyType::Multipart(t) => t.params(),
            AnyType::Message(t) => t.params.clone(),
            AnyType::Text(t) => t.params(),
            AnyType::Binary(t) => t.ctype.params.clone(),
        }
    }
}

impl<'a> From<&NaiveType<'a>> for AnyType<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        match nt.main.0.to_ascii_lowercase().as_slice() {
            b"multipart" =>
            // fails if there is no boundary parameter
            {
                Multipart::try_from(nt)
                    .map(Self::Multipart)
                    .unwrap_or(Self::Binary(Binary::from(nt)))
            }
            b"message" =>
            // fails if this the subtype is not supported
            {
                Message::try_from(nt)
                    .map(Self::Message)
                    .unwrap_or(Self::Binary(Binary::from(nt)))
            }
            b"text" => Self::Text(Text::from(nt)),
            _ => Self::Binary(Binary::from(nt)),
        }
    }
}

impl<'a> Print for AnyType<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        match self {
            AnyType::Multipart(mp) => mp.print(fmt),
            AnyType::Message(msg) => msg.print(fmt),
            AnyType::Text(txt) => txt.print(fmt),
            AnyType::Binary(bin) => bin.print(fmt),
        }
    }
}

// REAL PARTS

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Multipart<'a> {
    pub subtype: MultipartSubtype,
    #[cfg_attr(feature = "arbitrary", fuzz_eq(ignore))]
    #[contains_utf8(ignore)] // boundary is always ascii
    // XXX: this `boundary` field is a hack.
    //
    // `boundary` is tracked in this AST node as a parsing implementation
    // detail rather than some explicit information of the final email.
    //
    // During parsing, the parser for a multipart email body needs to know the
    // boundary that was specified in the headers to be able to parse parts. The
    // `boundary` field is used to propagate that information from the parser
    // for MIME headers to the parser for a multipart body.
    //
    // After parsing, this field is ignored. In particular, during printing, a
    // new boundary is generated by the eml-codec's printer, and is used instead
    // of the original boundary. Indeed, the original boundary may not be
    // correct to reuse if the body parts have been modified (by modifying the
    // parts AST)---remember that boundaries must not appear in body parts.
    //
    // Finally, this `boundary` is an `Option<String>` rather than a `String` to
    // account for the case where this AST node is constructed directly using
    // the library API and not from the parser. In this case there is no input
    // boundary to use, so the field can be set to `None`. In the other case
    // where a `mime::type::Multipart` record is constructed by the parser, the
    // `boundary` field is guaranteed to be `Some(...)`.
    pub boundary: Option<String>,
    // Invariant: parameters with .name != "boundary"
    pub other_params: Vec<Parameter<'a>>,
}

impl<'a> Multipart<'a> {
    pub fn params(&self) -> Vec<Parameter<'a>> {
        let mut params = self.other_params.clone();
        match &self.boundary {
            Some(b) => params.push(Parameter {
                name: MIMEAtom(b"boundary".into()),
                value: MIMEWord::Quoted(QuotedString(vec![b.into()])).into_static(),
            }),
            None => {
                // XXX in this case there is no boundary parameter returned,
                // even the final email will contain one...
            }
        };
        params
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Multipart<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let other_params: Vec<Parameter> = u.arbitrary()?;
        if other_params
            .iter()
            .any(|p| p.name.0.as_ref() == b"boundary")
        {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        Ok(Self {
            subtype: u.arbitrary()?,
            boundary: None,
            other_params,
        })
    }
}

impl<'a> Print for Multipart<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.push_new_boundary();
        fmt.write_bytes(b"multipart/");
        self.subtype.print(fmt);
        fmt.write_bytes(b";");
        fmt.write_fws();
        // always quote the boundary ("never hurts" says RFC2046)
        fmt.write_bytes(b"boundary=\"");
        fmt.write_current_boundary();
        fmt.write_bytes(b"\"");
        for param in &self.other_params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> TryFrom<&NaiveType<'a>> for Multipart<'a> {
    type Error = ();

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "type::Multipart::try_from")
    )]
    fn try_from(nt: &NaiveType<'a>) -> Result<Self, Self::Error> {
        let mut other_params = vec![];
        let mut boundary = None;
        for param in &nt.params {
            if param.name.0.to_ascii_lowercase().as_slice() == b"boundary" {
                let s = param.value.chars().collect::<String>();
                if boundary.is_none() {
                    boundary = Some(s);
                } else {
                    // drop any redundant "boundary" parameter that is not the first
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(boundary = s, "dropping redundant boundary parameter")
                }
            } else {
                other_params.push(param.clone())
            }
        }
        match boundary {
            Some(boundary) => Ok(Multipart {
                subtype: MultipartSubtype::from(nt),
                boundary: Some(boundary),
                other_params,
            }),
            None => Err(()),
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MultipartSubtype {
    Alternative,
    Mixed,
    Digest,
    Parallel,
    Report,
    // neither of the above (capitalization does not matter).
    // should be treated as Mixed
    Unknown(MIMEAtom<'static>),
}
impl MultipartSubtype {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Alternative => b"alternative",
            Self::Mixed => b"mixed",
            Self::Digest => b"digest",
            Self::Parallel => b"parallel",
            Self::Report => b"report",
            Self::Unknown(v) => &v.0,
        }
    }
}
impl Print for MultipartSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> From<&NaiveType<'a>> for MultipartSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        let sub = nt.sub.0.to_ascii_lowercase();
        match sub.as_slice() {
            b"alternative" => Self::Alternative,
            b"mixed" => Self::Mixed,
            b"digest" => Self::Digest,
            b"parallel" => Self::Parallel,
            b"report" => Self::Report,
            _ => Self::Unknown(nt.sub.to_static()),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MultipartSubtype {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=5)? {
            0 => Ok(MultipartSubtype::Alternative),
            1 => Ok(MultipartSubtype::Mixed),
            2 => Ok(MultipartSubtype::Digest),
            3 => Ok(MultipartSubtype::Parallel),
            4 => Ok(MultipartSubtype::Report),
            5 => {
                let a: MIMEAtom = u.arbitrary()?;
                if matches!(
                    a.0.to_ascii_lowercase().as_slice(),
                    b"alternative" | b"mixed" | b"digest" | b"parallel" | b"report"
                ) {
                    return Err(arbitrary::Error::IncorrectFormat);
                }
                Ok(MultipartSubtype::Unknown(a))
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, Default, PartialEq, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum MessageSubtype {
    #[default]
    RFC822,
    Global, // RFC6532 subtype (message containing UTF-8 headers)
}
impl MessageSubtype {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::RFC822 => b"rfc822",
            Self::Global => b"global",
        }
    }
}
impl Print for MessageSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> TryFrom<&NaiveType<'a>> for MessageSubtype {
    type Error = ();

    fn try_from(nt: &NaiveType<'a>) -> Result<Self, ()> {
        let sub = nt.sub.0.to_ascii_lowercase();
        match sub.as_slice() {
            b"rfc822" => Ok(MessageSubtype::RFC822),
            b"global" => Ok(MessageSubtype::Global),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MessageSubtype {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=1)? {
            0 => Ok(MessageSubtype::RFC822),
            1 => Ok(MessageSubtype::Global),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, Default, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary, FuzzEq))]
pub struct Message<'a> {
    pub subtype: MessageSubtype,
    pub params: Vec<Parameter<'a>>,
}

impl<'a> Print for Message<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"message/");
        self.subtype.print(fmt);
        for param in &self.params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> TryFrom<&NaiveType<'a>> for Message<'a> {
    type Error = ();
    fn try_from(nt: &NaiveType<'a>) -> Result<Self, ()> {
        Ok(Self {
            subtype: MessageSubtype::try_from(nt)?,
            params: nt.params.clone(),
        })
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, Default, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Text<'a> {
    // NOTE: an unknown subtype combined with an unknown charset should
    // result in this type be treated as equivalent to the Binary type.
    pub subtype: TextSubtype,
    pub charset: EmailCharset,
    // Invariant: parameters with .name != "charset"
    pub other_params: Vec<Parameter<'a>>,
}

impl<'a> Text<'a> {
    pub fn params(&self) -> Vec<Parameter<'a>> {
        let mut params = self.other_params.clone();
        params.push(Parameter {
            name: MIMEAtom(b"charset".into()),
            value: MIMEWord::Quoted(QuotedString(vec![self.charset.as_str().into()])).into_static(),
        });
        params
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Text<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let other_params: Vec<Parameter> = u.arbitrary()?;
        if other_params.iter().any(|p| p.name.0.as_ref() == b"charset") {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        Ok(Self {
            subtype: u.arbitrary()?,
            charset: u.arbitrary()?,
            other_params,
        })
    }
}

impl<'a> Print for Text<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(b"text/");
        self.subtype.print(fmt);
        fmt.write_bytes(b";");
        fmt.write_fws();
        fmt.write_bytes(b"charset=");
        match &self.charset {
            EmailCharset::Unknown(s) =>
            // print it as quoted just to be safe
            {
                print_quoted(fmt, s.chars())
            }
            _ => fmt.write_bytes(self.charset.as_bytes()),
        }
        for param in &self.other_params {
            fmt.write_bytes(b";");
            fmt.write_fws();
            param.print(fmt);
        }
    }
}

impl<'a> From<&NaiveType<'a>> for Text<'a> {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn from(nt: &NaiveType<'a>) -> Self {
        let mut other_params = vec![];
        let mut charset = None;
        for param in &nt.params {
            if param.name.0.to_ascii_lowercase().as_slice() == b"charset" {
                let value: String = param.value.chars().collect();
                if charset.is_none() {
                    charset = Some(EmailCharset::from(&value));
                } else {
                    // drop any "charset" parameter that is not the first
                    #[cfg(feature = "tracing-unsupported")]
                    warn!(param = value, "dropping redundant charset parameter");
                }
            } else {
                other_params.push(param.clone())
            }
        }

        Self {
            subtype: TextSubtype::from(nt),
            charset: charset.unwrap_or_default(),
            other_params,
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, Default, ToStatic, ToStringFromPrint)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub enum TextSubtype {
    #[default]
    Plain,
    Html,
    // none of the above
    Unknown(MIMEAtom<'static>),
}
impl TextSubtype {
    pub fn as_bytes(&self) -> &[u8] {
        use TextSubtype::*;
        match self {
            Plain => b"plain",
            Html => b"html",
            Unknown(b) => &b.0,
        }
    }
}
impl Print for TextSubtype {
    fn print(&self, fmt: &mut impl Formatter) {
        fmt.write_bytes(self.as_bytes())
    }
}

impl<'a> From<&NaiveType<'a>> for TextSubtype {
    fn from(nt: &NaiveType<'a>) -> Self {
        let sub = nt.sub.0.to_ascii_lowercase();
        match sub.as_slice() {
            b"plain" => Self::Plain,
            b"html" => Self::Html,
            _ => Self::Unknown(nt.sub.to_static()),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for TextSubtype {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=2)? {
            0 => Ok(TextSubtype::Plain),
            1 => Ok(TextSubtype::Html),
            2 => {
                let a: MIMEAtom = u.arbitrary()?;
                if matches!(a.0.to_ascii_lowercase().as_slice(), b"plain" | b"html") {
                    return Err(arbitrary::Error::IncorrectFormat);
                }
                Ok(TextSubtype::Unknown(a))
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, ContainsUtf8, Debug, PartialEq, ToStatic)]
#[cfg_attr(feature = "arbitrary", derive(FuzzEq))]
pub struct Binary<'a> {
    // invariant: ctype.main is neither "multipart", "message" or "text"
    pub ctype: NaiveType<'a>,
}

impl<'a> Print for Binary<'a> {
    fn print(&self, fmt: &mut impl Formatter) {
        self.ctype.print(fmt)
    }
}
impl<'a> From<&NaiveType<'a>> for Binary<'a> {
    fn from(nt: &NaiveType<'a>) -> Self {
        Self { ctype: nt.clone() }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for Binary<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let ctype: NaiveType = u.arbitrary()?;
        if matches!(
            ctype.main.0.to_ascii_lowercase().as_slice(),
            b"multipart" | b"message" | b"text"
        ) {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        Ok(Self { ctype })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::charset::EmailCharset;
    use crate::text::quoted::QuotedString;

    #[test]
    fn test_parameter() {
        assert_eq!(
            parameter(b"charset=utf-8"),
            Ok((
                &b""[..],
                Parameter {
                    name: MIMEAtom(b"charset"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"utf-8"[..].into())),
                }
            )),
        );
        assert_eq!(
            parameter(b"charset=\"utf-8\""),
            Ok((
                &b""[..],
                Parameter {
                    name: MIMEAtom(b"charset"[..].into()),
                    value: MIMEWord::Quoted(QuotedString(vec!["utf-8"[..].into()])),
                }
            )),
        );
    }

    #[test]
    fn test_content_type_plaintext() {
        let (rest, nt) = naive_type(b"text/plain;\r\n charset=utf-8 ; hello=yolo").unwrap();
        assert_eq!(rest, &b""[..]);

        assert_eq!(
            nt.to_type(),
            AnyType::Text(Text {
                charset: EmailCharset::utf8(),
                subtype: TextSubtype::Plain,
                other_params: vec![Parameter {
                    name: MIMEAtom(b"hello"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"yolo"[..].into())),
                }],
            })
        );
    }

    // old invalid form of text/plain
    #[test]
    fn test_content_type_plaintext_old() {
        let (rest, nt) = naive_type(b"  text ").unwrap();
        assert_eq!(rest, &b""[..]);
        assert_eq!(
            nt.to_type(),
            AnyType::Text(Text {
                charset: EmailCharset::US_ASCII,
                subtype: TextSubtype::Plain,
                other_params: vec![],
            })
        );

        let (rest, nt) = naive_type(b"text;\r\n charset=utf-8 ; hello=yolo").unwrap();
        assert_eq!(rest, &b""[..]);
        assert_eq!(
            nt.to_type(),
            AnyType::Text(Text {
                charset: EmailCharset::utf8(),
                subtype: TextSubtype::Plain,
                other_params: vec![Parameter {
                    name: MIMEAtom(b"hello"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"yolo"[..].into())),
                }],
            })
        );
    }

    #[test]
    fn test_content_type_multipart() {
        let (rest, nt) = naive_type(b"multipart/mixed;\r\n\tboundary=\"--==_mimepart_64a3f2c69114f_2a13d020975fe\";\r\n\tcharset=UTF-8").unwrap();
        assert_eq!(rest, &[]);
        assert_eq!(
            nt.to_type(),
            AnyType::Multipart(Multipart {
                subtype: MultipartSubtype::Mixed,
                boundary: Some("--==_mimepart_64a3f2c69114f_2a13d020975fe".into()),
                other_params: vec![Parameter {
                    name: MIMEAtom(b"charset"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"UTF-8"[..].into())),
                }],
            })
        );
    }

    #[test]
    fn test_content_type_message() {
        let (rest, nt) = naive_type(b"message/rfc822").unwrap();
        assert_eq!(rest, &[]);
        assert_eq!(
            nt.to_type(),
            AnyType::Message(Message {
                subtype: MessageSubtype::RFC822,
                params: vec![],
            })
        );

        // unknown message subtype: treat it as "application/octet-stream"
        // (i.e. opaque Binary part)
        let (rest, nt) = naive_type(b"message/delivery-status").unwrap();
        assert_eq!(rest, &[]);
        assert_eq!(
            nt.to_type(),
            AnyType::Binary(Binary {
                ctype: NaiveType {
                    main: MIMEAtom(b"message"[..].into()),
                    sub: MIMEAtom(b"delivery-status"[..].into()),
                    params: vec![],
                }
            })
        );
    }

    #[test]
    fn test_content_type_comment() {
        let (rest, nt) = naive_type(b"text/plain; charset=\"us-ascii\" (Plain text)").unwrap();
        assert_eq!(rest, &[]);

        assert_eq!(
            nt.to_type(),
            AnyType::Text(Text {
                subtype: TextSubtype::Plain,
                charset: EmailCharset::from(b"us-ascii"),
                other_params: vec![],
            })
        );
    }

    #[test]
    fn test_broken_content_type() {
        let (rest, nt) = naive_type(b"abc/def/ghi; charset=us-ascii").unwrap();
        assert_eq!(rest, &[]);

        assert_eq!(
            nt,
            NaiveType {
                main: MIMEAtom(b"abc".into()),
                sub: MIMEAtom(b"def".into()),
                params: vec![Parameter {
                    name: MIMEAtom(b"charset"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"us-ascii"[..].into())),
                }],
            }
        );
    }

    #[test]
    fn test_parameter_ascii() {
        assert_eq!(
            parameter(b"charset = (simple) us-ascii (Plain text)"),
            Ok((
                &b""[..],
                Parameter {
                    name: MIMEAtom(b"charset"[..].into()),
                    value: MIMEWord::Atom(MIMEAtom(b"us-ascii"[..].into())),
                }
            ))
        );
    }

    #[test]
    fn test_parameter_list_semicolons() {
        // we allow final semicolons
        assert_eq!(
            parameter_list(b";boundary=\"festivus\";"),
            Ok((
                &b""[..],
                vec![Parameter {
                    name: MIMEAtom(b"boundary"[..].into()),
                    value: MIMEWord::Quoted(QuotedString(vec!["festivus"[..].into()])),
                }],
            ))
        );

        assert_eq!(
            parameter_list(b"; charset=UTF-8; format=flowed; "),
            Ok((
                &b""[..],
                vec![
                    Parameter {
                        name: MIMEAtom(b"charset"[..].into()),
                        value: MIMEWord::Atom(MIMEAtom(b"UTF-8"[..].into())),
                    },
                    Parameter {
                        name: MIMEAtom(b"format"[..].into()),
                        value: MIMEWord::Atom(MIMEAtom(b"flowed"[..].into())),
                    },
                ],
            ))
        );

        // semicolons can appear between quotes, this is part of the normal
        // quote syntax
        assert_eq!(
            parameter_list(b"; boundary=\"abc;def\"; foo=bar"),
            Ok((
                &b""[..],
                vec![
                    Parameter {
                        name: MIMEAtom(b"boundary"[..].into()),
                        value: MIMEWord::Quoted(QuotedString(vec!["abc;def"[..].into()])),
                    },
                    Parameter {
                        name: MIMEAtom(b"foo"[..].into()),
                        value: MIMEWord::Atom(MIMEAtom(b"bar"[..].into())),
                    },
                ],
            ))
        );
    }

    #[test]
    fn test_parameter_list_broken() {
        // these test cases come from real-world emails with broken parameter lists
        assert_eq!(
            parameter_list(b"; name=threadTest.ml; charset="),
            Ok((
                &b""[..],
                vec![Parameter {
                    name: MIMEAtom(b"name".into()),
                    value: MIMEWord::Atom(MIMEAtom(b"threadTest.ml".into())),
                },]
            ))
        );

        // Anytime emits emails with 'charset: UTF-8'; we add support for those...
        assert_eq!(
            parameter_list(b"; charset: UTF-8; foo=bar"),
            Ok((
                &b""[..],
                vec![
                    Parameter {
                        name: MIMEAtom(b"charset".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"UTF-8".into())),
                    },
                    Parameter {
                        name: MIMEAtom(b"foo".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"bar".into())),
                    },
                ]
            ))
        );

        assert_eq!(
            // Example emitted by inria CASA. An extra space was inserted before
            // the Content-Transfer-Encoding header name, making it a
            // continuation of the previous Content-Type header as per line
            // folding rules... This ends up being read as an extra parameter
            // "thanks" to the recovery of ':' as '='...
            parameter_list(
                b"; name=\"calendar.ics\";method=REQUEST;\n Content-Transfer-Encoding: 8bit;"
            ),
            Ok((
                &b""[..],
                vec![
                    Parameter {
                        name: MIMEAtom(b"name".into()),
                        value: MIMEWord::Quoted(QuotedString(vec!["calendar.ics".into()])),
                    },
                    Parameter {
                        name: MIMEAtom(b"method".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"REQUEST".into())),
                    },
                    Parameter {
                        name: MIMEAtom(b"Content-Transfer-Encoding".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"8bit".into())),
                    },
                ]
            ))
        );

        assert_eq!(
            parameter_list(b"; name=threadTest.ml foo=bar; baz=qux"),
            Ok((
                &b""[..],
                vec![
                    Parameter {
                        name: MIMEAtom(b"name".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"threadTest.ml".into())),
                    },
                    Parameter {
                        name: MIMEAtom(b"baz".into()),
                        value: MIMEWord::Atom(MIMEAtom(b"qux".into())),
                    },
                ]
            ))
        );
    }

    #[test]
    fn test_roundtrip_unknown() {
        let raw = b"Foo/Bar; bAr=Unknown; uU=zorrO";
        let (rest, nt) = naive_type(raw).unwrap();
        assert_eq!(rest, &[]);
        let t: AnyType = nt.to_type();
        assert!(matches!(t, AnyType::Binary(_)));
        let printed = crate::print::tests::print_to_vec(t);
        assert_eq!(
            String::from_utf8_lossy(raw),
            String::from_utf8_lossy(&printed)
        )
    }
}
