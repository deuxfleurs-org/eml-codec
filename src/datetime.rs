use chrono::DateTime;
use nom::IResult;
use crate::{model,misc_token};

pub fn section(input: &str) -> IResult<&str, model::HeaderDate> {
    // @FIXME want to extract datetime our way in the future
    // to better handle obsolete/bad cases instead of returning raw text.
    let (input, raw_date) = misc_token::unstructured(input)?;
    match DateTime::parse_from_rfc2822(&raw_date) {
        Ok(chronodt) => Ok((input, model::HeaderDate::Parsed(chronodt))),
        Err(e) => Ok((input, model::HeaderDate::Unknown(raw_date, e))),
    }
}
