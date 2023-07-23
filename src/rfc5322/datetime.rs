use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag, tag_no_case, take_while_m_n},
    character,
    character::complete::{alphanumeric1, digit0},
    combinator::{map, opt, value},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

use crate::text::whitespace::{cfws, fws};
//use crate::error::IMFError;

const MIN: i32 = 60;
const HOUR: i32 = 60 * MIN;

/*
impl<'a> TryFrom<&'a lazy::DateTime<'a>> for DateTime<FixedOffset> {
    type Error = IMFError<'a>;

    fn try_from(value: &'a lazy::DateTime<'a>) -> Result<Self, Self::Error> {
        match section(value.0) {
            Ok((_, Some(dt))) => Ok(dt),
            Err(e) => Err(IMFError::DateTimeParse(e)),
            _ => Err(IMFError::DateTimeLogic),
        }
    }
}*/

/// Read datetime
///
/// ```abnf
/// date-time       =   [ day-of-week "," ] date time [CFWS]
/// time            =   time-of-day zone
/// ```
///
/// ## @FIXME - known bugs
///  
///   - if chrono fails, Option::None is silently returned instead of failing the parser
///   - `-0000` means NaiveDateTime, a date without a timezone
/// while this library interprets it as +0000 aka UTC.
///   - Obsolete military zones should be considered as NaiveTime
/// due to an error in RFC0822 but are interpreted as their respective
/// timezone according to the RFC5322 definition
pub fn section(input: &[u8]) -> IResult<&[u8], Option<DateTime<FixedOffset>>> {
    map(
        terminated(
            alt((
                tuple((
                    opt(terminated(strict_day_of_week, tag(","))),
                    strict_date,
                    strict_time_of_day,
                    strict_zone,
                )),
                tuple((
                    opt(terminated(obs_day_of_week, tag(","))),
                    obs_date,
                    obs_time_of_day,
                    alt((strict_zone, obs_zone)),
                )),
            )),
            opt(cfws),
        ),
        |res| match res {
            (_, Some(date), Some(time), Some(tz)) => {
                date.and_time(time).and_local_timezone(tz).earliest()
            }
            _ => None,
        },
    )(input)
}

///    day-of-week     =   ([FWS] day-name) / obs-day-of-week
fn strict_day_of_week(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(opt(fws), day_name)(input)
}

///    obs-day-of-week =   [CFWS] day-name [CFWS]
fn obs_day_of_week(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(opt(cfws), day_name, opt(cfws))(input)
}

///   day-name        =   "Mon" / "Tue" / "Wed" / "Thu" /
///                       "Fri" / "Sat" / "Sun"
fn day_name(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((
        tag_no_case(b"Mon"),
        tag_no_case(b"Tue"),
        tag_no_case(b"Wed"),
        tag_no_case(b"Thu"),
        tag_no_case(b"Fri"),
        tag_no_case(b"Sat"),
        tag_no_case(b"Sun"),
    ))(input)
}

///    date            =   day month year
fn strict_date(input: &[u8]) -> IResult<&[u8], Option<NaiveDate>> {
    map(tuple((strict_day, month, strict_year)), |(d, m, y)| {
        NaiveDate::from_ymd_opt(y, m, d)
    })(input)
}

///    date            =   day month year
fn obs_date(input: &[u8]) -> IResult<&[u8], Option<NaiveDate>> {
    map(tuple((obs_day, month, obs_year)), |(d, m, y)| {
        NaiveDate::from_ymd_opt(y, m, d)
    })(input)
}

///    day             =   ([FWS] 1*2DIGIT FWS) / obs-day
fn strict_day(input: &[u8]) -> IResult<&[u8], u32> {
    delimited(opt(fws), character::complete::u32, fws)(input)
}

///    obs-day         =   [CFWS] 1*2DIGIT [CFWS]
fn obs_day(input: &[u8]) -> IResult<&[u8], u32> {
    delimited(opt(cfws), character::complete::u32, opt(cfws))(input)
}

///  month           =   "Jan" / "Feb" / "Mar" / "Apr" /
///                      "May" / "Jun" / "Jul" / "Aug" /
///                      "Sep" / "Oct" / "Nov" / "Dec"
fn month(input: &[u8]) -> IResult<&[u8], u32> {
    alt((
        value(1, tag_no_case(b"Jan")),
        value(2, tag_no_case(b"Feb")),
        value(3, tag_no_case(b"Mar")),
        value(4, tag_no_case(b"Apr")),
        value(5, tag_no_case(b"May")),
        value(6, tag_no_case(b"Jun")),
        value(7, tag_no_case(b"Jul")),
        value(8, tag_no_case(b"Aug")),
        value(9, tag_no_case(b"Sep")),
        value(10, tag_no_case(b"Oct")),
        value(11, tag_no_case(b"Nov")),
        value(12, tag_no_case(b"Dec")),
    ))(input)
}

///   year            =   (FWS 4*DIGIT FWS) / obs-year
fn strict_year(input: &[u8]) -> IResult<&[u8], i32> {
    delimited(
        fws,
        map(
            terminated(take_while_m_n(4, 9, |c| c >= 0x30 && c <= 0x39), digit0),
            |d: &[u8]| {
                encoding_rs::UTF_8
                    .decode_without_bom_handling(d)
                    .0
                    .parse::<i32>()
                    .unwrap_or(0)
            },
        ),
        fws,
    )(input)
}

///   obs-year        =   [CFWS] 2*DIGIT [CFWS]
fn obs_year(input: &[u8]) -> IResult<&[u8], i32> {
    map(
        delimited(
            opt(cfws),
            terminated(take_while_m_n(2, 7, |c| c >= 0x30 && c <= 0x39), digit0),
            opt(cfws),
        ),
        |cap: &[u8]| {
            let year_txt = encoding_rs::UTF_8.decode_without_bom_handling(cap).0;
            let d = year_txt.parse::<i32>().unwrap_or(0);
            if d >= 0 && d <= 49 {
                2000 + d
            } else if d >= 50 && d <= 999 {
                1900 + d
            } else {
                d
            }
        },
    )(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
fn strict_time_of_day(input: &[u8]) -> IResult<&[u8], Option<NaiveTime>> {
    map(
        tuple((
            strict_time_digit,
            tag(":"),
            strict_time_digit,
            opt(preceded(tag(":"), strict_time_digit)),
        )),
        |(hour, _, minute, maybe_sec)| {
            NaiveTime::from_hms_opt(hour, minute, maybe_sec.unwrap_or(0))
        },
    )(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
fn obs_time_of_day(input: &[u8]) -> IResult<&[u8], Option<NaiveTime>> {
    map(
        tuple((
            obs_time_digit,
            tag(":"),
            obs_time_digit,
            opt(preceded(tag(":"), obs_time_digit)),
        )),
        |(hour, _, minute, maybe_sec)| {
            NaiveTime::from_hms_opt(hour, minute, maybe_sec.unwrap_or(0))
        },
    )(input)
}

fn strict_time_digit(input: &[u8]) -> IResult<&[u8], u32> {
    character::complete::u32(input)
}

fn obs_time_digit(input: &[u8]) -> IResult<&[u8], u32> {
    delimited(opt(cfws), character::complete::u32, opt(cfws))(input)
}

/// Obsolete zones
///
/// ```abnf
///   zone            =   (FWS ( "+" / "-" ) 4DIGIT) / (FWS obs-zone)
/// ```
fn strict_zone(input: &[u8]) -> IResult<&[u8], Option<FixedOffset>> {
    map(
        tuple((
            opt(fws),
            is_a("+-"),
            take_while_m_n(2, 2, |c| c >= 0x30 && c <= 0x39),
            take_while_m_n(2, 2, |c| c >= 0x30 && c <= 0x39),
        )),
        |(_, op, dig_zone_hour, dig_zone_min)| {
            let zone_hour: i32 =
                ((dig_zone_hour[0] - 0x30) * 10 + (dig_zone_hour[1] - 0x30)) as i32 * HOUR;
            let zone_min: i32 =
                ((dig_zone_min[0] - 0x30) * 10 + (dig_zone_min[1] - 0x30)) as i32 * MIN;
            match op {
                b"+" => FixedOffset::east_opt(zone_hour + zone_min),
                b"-" => FixedOffset::west_opt(zone_hour + zone_min),
                _ => unreachable!(),
            }
        },
    )(input)
}

/// obsole zone
///
///   obs-zone        =   "UT" / "GMT" /     ; Universal Time
///                                          ; North American UT
///                                          ; offsets
///                       "EST" / "EDT" /    ; Eastern:  - 5/ - 4
///                       "CST" / "CDT" /    ; Central:  - 6/ - 5
///                       "MST" / "MDT" /    ; Mountain: - 7/ - 6
///                       "PST" / "PDT" /    ; Pacific:  - 8/ - 7
///                                          ;
///                       %d65-73 /          ; Military zones - "A"
///                       %d75-90 /          ; through "I" and "K"
///                       %d97-105 /         ; through "Z", both
///                       %d107-122 /        ; upper and lower case
///                                          ;
///                       1*(ALPHA / DIGIT)  ; Unknown legacy timezones
fn obs_zone(input: &[u8]) -> IResult<&[u8], Option<FixedOffset>> {
    // The writing of this function is volontarily verbose
    // to keep it straightforward to understand.
    // @FIXME: Could return a TimeZone and not an Option<TimeZone>
    // as it could be determined at compile time if values are correct
    // and panic at this time if not. But not sure how to do it without unwrap.
    preceded(
        opt(fws),
        alt((
            // Legacy UTC/GMT
            value(
                FixedOffset::west_opt(0 * HOUR),
                alt((tag_no_case(b"UTC"), tag_no_case(b"UT"), tag_no_case(b"GMT"))),
            ),
            // USA Timezones
            value(FixedOffset::west_opt(4 * HOUR), tag_no_case(b"EDT")),
            value(
                FixedOffset::west_opt(5 * HOUR),
                alt((tag_no_case(b"EST"), tag_no_case(b"CDT"))),
            ),
            value(
                FixedOffset::west_opt(6 * HOUR),
                alt((tag_no_case(b"CST"), tag_no_case(b"MDT"))),
            ),
            value(
                FixedOffset::west_opt(7 * HOUR),
                alt((tag_no_case(b"MST"), tag_no_case(b"PDT"))),
            ),
            value(FixedOffset::west_opt(8 * HOUR), tag_no_case(b"PST")),
            // Military Timezone UTC
            value(FixedOffset::west_opt(0 * HOUR), tag_no_case(b"Z")),
            // Military Timezones East
            alt((
                value(FixedOffset::east_opt(1 * HOUR), tag_no_case(b"A")),
                value(FixedOffset::east_opt(2 * HOUR), tag_no_case(b"B")),
                value(FixedOffset::east_opt(3 * HOUR), tag_no_case(b"C")),
                value(FixedOffset::east_opt(4 * HOUR), tag_no_case(b"D")),
                value(FixedOffset::east_opt(5 * HOUR), tag_no_case(b"E")),
                value(FixedOffset::east_opt(6 * HOUR), tag_no_case(b"F")),
                value(FixedOffset::east_opt(7 * HOUR), tag_no_case(b"G")),
                value(FixedOffset::east_opt(8 * HOUR), tag_no_case(b"H")),
                value(FixedOffset::east_opt(9 * HOUR), tag_no_case(b"I")),
                value(FixedOffset::east_opt(10 * HOUR), tag_no_case(b"K")),
                value(FixedOffset::east_opt(11 * HOUR), tag_no_case(b"L")),
                value(FixedOffset::east_opt(12 * HOUR), tag_no_case(b"M")),
            )),
            // Military Timezones West
            alt((
                value(FixedOffset::west_opt(1 * HOUR), tag_no_case(b"N")),
                value(FixedOffset::west_opt(2 * HOUR), tag_no_case(b"O")),
                value(FixedOffset::west_opt(3 * HOUR), tag_no_case(b"P")),
                value(FixedOffset::west_opt(4 * HOUR), tag_no_case(b"Q")),
                value(FixedOffset::west_opt(5 * HOUR), tag_no_case(b"R")),
                value(FixedOffset::west_opt(6 * HOUR), tag_no_case(b"S")),
                value(FixedOffset::west_opt(7 * HOUR), tag_no_case(b"T")),
                value(FixedOffset::west_opt(8 * HOUR), tag_no_case(b"U")),
                value(FixedOffset::west_opt(9 * HOUR), tag_no_case(b"V")),
                value(FixedOffset::west_opt(10 * HOUR), tag_no_case(b"W")),
                value(FixedOffset::west_opt(11 * HOUR), tag_no_case(b"X")),
                value(FixedOffset::west_opt(12 * HOUR), tag_no_case(b"Y")),
            )),
            // Unknown timezone
            value(FixedOffset::west_opt(0 * HOUR), alphanumeric1),
        )),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_section_rfc_strict() {
        assert_eq!(
            section(b"Fri, 21 Nov 1997 09:55:06 -0600"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::west_opt(6 * HOUR)
                        .unwrap()
                        .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_received() {
        assert_eq!(
            section(b"Sun, 18 Jun 2023 15:39:08 +0200 (CEST)"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(2 * HOUR)
                        .unwrap()
                        .with_ymd_and_hms(2023, 6, 18, 15, 39, 8)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_rfc_ws() {
        assert_eq!(
            section(
                r#"Thu,
         13
           Feb
             1969
         23:32
                  -0330 (Newfoundland Time)"#
                    .as_bytes()
            ),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::west_opt(3 * HOUR + 30 * MIN)
                        .unwrap()
                        .with_ymd_and_hms(1969, 2, 13, 23, 32, 00)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_rfc_obs() {
        assert_eq!(
            section(b"21 Nov 97 09:55:06 GMT"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_3digit_year() {
        assert_eq!(
            section(b"21 Nov 103 09:55:06 UT"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2003, 11, 21, 9, 55, 6)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_rfc_obs_ws() {
        assert_eq!(
            section(b"Fri, 21 Nov 1997 09(comment):   55  :  06 -0600"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::west_opt(6 * HOUR)
                        .unwrap()
                        .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_2digit_year() {
        assert_eq!(
            section(b"21 Nov 23 09:55:06Z"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 9, 55, 6)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_military_zone_east() {
        ["a", "B", "c", "D", "e", "F", "g", "H", "i", "K", "l", "M"]
            .iter()
            .enumerate()
            .for_each(|(i, x)| {
                assert_eq!(
                    section(format!("1 Jan 22 08:00:00 {}", x).as_bytes()),
                    Ok((
                        &b""[..],
                        Some(
                            FixedOffset::east_opt((i as i32 + 1) * HOUR)
                                .unwrap()
                                .with_ymd_and_hms(2022, 01, 01, 8, 0, 0)
                                .unwrap()
                        )
                    ))
                );
            });
    }

    #[test]
    fn test_section_military_zone_west() {
        ["N", "O", "P", "q", "r", "s", "T", "U", "V", "w", "x", "y"]
            .iter()
            .enumerate()
            .for_each(|(i, x)| {
                assert_eq!(
                    section(format!("1 Jan 22 08:00:00 {}", x).as_bytes()),
                    Ok((
                        &b""[..],
                        Some(
                            FixedOffset::west_opt((i as i32 + 1) * HOUR)
                                .unwrap()
                                .with_ymd_and_hms(2022, 01, 01, 8, 0, 0)
                                .unwrap()
                        )
                    ))
                );
            });
    }

    #[test]
    fn test_section_gmt() {
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 +0000"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 -0000"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 Z"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 GMT"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 UT"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
        assert_eq!(
            section(b"21 Nov 2023 07:07:07 UTC"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::east_opt(0)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                        .unwrap()
                )
            )),
        );
    }

    #[test]
    fn test_section_usa() {
        assert_eq!(
            section(b"21 Nov 2023 4:4:4 CST"),
            Ok((
                &b""[..],
                Some(
                    FixedOffset::west_opt(6 * HOUR)
                        .unwrap()
                        .with_ymd_and_hms(2023, 11, 21, 4, 4, 4)
                        .unwrap()
                )
            )),
        );
    }
}
