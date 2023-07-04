use nom;

#[derive(Debug, PartialEq)]
pub enum IMFError<'a> {
    Segment(nom::Err<nom::error::Error<&'a [u8]>>),
    ExtractFields(nom::Err<nom::error::Error<&'a str>>),
    DateTimeParse(nom::Err<nom::error::Error<&'a str>>),
    DateTimeLogic,
    Mailbox(nom::Err<nom::error::Error<&'a str>>),
    MailboxList(nom::Err<nom::error::Error<&'a str>>),
    AddressList(nom::Err<nom::error::Error<&'a str>>),
    NullableAddressList(nom::Err<nom::error::Error<&'a str>>),
    MessageID(nom::Err<nom::error::Error<&'a str>>),
    MessageIDList(nom::Err<nom::error::Error<&'a str>>),
    Unstructured(nom::Err<nom::error::Error<&'a str>>),
    PhraseList(nom::Err<nom::error::Error<&'a str>>),
    ReceivedLog(nom::Err<nom::error::Error<&'a str>>),
    Version(nom::Err<nom::error::Error<&'a str>>),
    ContentType(nom::Err<nom::error::Error<&'a str>>),
}
