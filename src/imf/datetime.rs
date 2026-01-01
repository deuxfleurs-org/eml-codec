use bounded_static::{IntoBoundedStatic, ToBoundedStatic};
use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, Timelike};
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag, tag_no_case, take_while_m_n},
    character,
    character::complete::{alphanumeric1, digit0},
    combinator::{map, map_opt, opt, value},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use std::fmt::{Debug, Formatter};

use crate::display_bytes::{Print, Formatter as PFmt};
use crate::text::whitespace::{cfws, fws};
//use crate::error::IMFError;

const MIN: i32 = 60;
const HOUR: i32 = 60 * MIN;

const MONTHS: &[&[u8]] = &[
    b"Jan",
    b"Feb",
    b"Mar",
    b"Apr",
    b"May",
    b"Jun",
    b"Jul",
    b"Aug",
    b"Sep",
    b"Oct",
    b"Nov",
    b"Dec",
];

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

// NOTE: must satisfy the following properties:
// - timezone offset: must be a round hours+minutes (no seconds)
// - year must be after 1900 or later
#[derive(Clone, PartialEq)]
pub struct DateTime(pub chrono::DateTime<FixedOffset>);

impl Debug for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl AsRef<chrono::DateTime<FixedOffset>> for DateTime {
    fn as_ref(&self) -> &chrono::DateTime<FixedOffset> {
        &self.0
    }
}

impl IntoBoundedStatic for DateTime {
    type Static = Self;
    fn into_static(self) -> Self::Static {
        self
    }
}

impl ToBoundedStatic for DateTime {
    type Static = Self;
    fn to_static(&self) -> Self::Static {
        self.clone()
    }
}

impl Print for DateTime {
    fn print(&self, fmt: &mut impl PFmt) -> std::io::Result<()> {
        // date
        fmt.write_bytes(format!("{:02}", self.0.day()).as_bytes())?;
        fmt.write_fws()?;
        fmt.write_bytes(MONTHS[self.0.month0() as usize])?;
        fmt.write_fws()?;
        fmt.write_bytes(format!("{}", self.0.year()).as_bytes())?;
        fmt.write_fws()?;
        // time-of-day
        fmt.write_bytes(format!("{:02}", self.0.hour()).as_bytes())?;
        fmt.write_bytes(b":")?;
        fmt.write_bytes(format!("{:02}", self.0.minute()).as_bytes())?;
        fmt.write_bytes(b":")?;
        fmt.write_bytes(format!("{:02}", self.0.second()).as_bytes())?;
        fmt.write_fws()?;
        // zone
        let offset_secs = self.0.offset().local_minus_utc();
        let sign = if offset_secs >= 0 { b"+" } else { b"-" };
        let offset_mins = offset_secs.abs().rem_euclid(HOUR).div_euclid(MIN);
        let offset_hours = offset_secs.abs().div_euclid(HOUR);
        fmt.write_bytes(sign)?;
        fmt.write_bytes(
            format!("{:02}{:02}", offset_hours, offset_mins).as_bytes()
        )?;
        Ok(())
    }
}

/// Read datetime
///
/// ```abnf
/// date-time       =   [ day-of-week "," ] date time [CFWS]
/// time            =   time-of-day zone
/// ```
///
/// ## @FIXME - known bugs
///  
///   - `-0000` means NaiveDateTime, a date without a timezone
/// while this library interprets it as +0000 aka UTC.
///   - Obsolete military zones should be considered as NaiveTime
/// due to an error in RFC0822 but are interpreted as their respective
/// timezone according to the RFC5322 definition
pub fn date_time(input: &[u8]) -> IResult<&[u8], DateTime> {
    map_opt(
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
        |(_, date, time, tz)| {
            date.and_time(time).and_local_timezone(tz).earliest().map(DateTime)
        }
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
fn strict_date(input: &[u8]) -> IResult<&[u8], NaiveDate> {
    map_opt(tuple((strict_day, month, strict_year)), |(d, m, y)| {
        NaiveDate::from_ymd_opt(y, m, d)
    })(input)
}

///    date            =   day month year
fn obs_date(input: &[u8]) -> IResult<&[u8], NaiveDate> {
    map_opt(tuple((obs_day, month, obs_year)), |(d, m, y)| {
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
            terminated(take_while_m_n(4, 9, |c| (0x30..=0x39).contains(&c)), digit0),
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
// NOTE: RFC5322 defines obs-year as above, but also defines the interpretation
// of three digit years (which are not covered by this grammar).
// The implementation below thus also supports three digit years.
fn obs_year(input: &[u8]) -> IResult<&[u8], i32> {
    map(
        delimited(
            opt(cfws),
            terminated(take_while_m_n(2, 7, |c| (0x30..=0x39).contains(&c)), digit0),
            opt(cfws),
        ),
        |cap: &[u8]| {
            let year_txt = encoding_rs::UTF_8.decode_without_bom_handling(cap).0;
            let d = year_txt.parse::<i32>().unwrap_or(0);
            if (0..=49).contains(&d) {
                2000 + d
            } else if (50..=999).contains(&d) {
                1900 + d
            } else {
                d
            }
        },
    )(input)
}

///   time-of-day     =   hour ":" minute [ ":" second ]
fn strict_time_of_day(input: &[u8]) -> IResult<&[u8], NaiveTime> {
    map_opt(
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
fn obs_time_of_day(input: &[u8]) -> IResult<&[u8], NaiveTime> {
    map_opt(
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
fn strict_zone(input: &[u8]) -> IResult<&[u8], FixedOffset> {
    map_opt(
        tuple((
            opt(fws),
            is_a("+-"),
            take_while_m_n(2, 2, |c| (0x30..=0x39).contains(&c)),
            take_while_m_n(2, 2, |c| (0x30..=0x39).contains(&c)),
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
fn obs_zone(input: &[u8]) -> IResult<&[u8], FixedOffset> {
    // The writing of this function is volontarily verbose
    // to keep it straightforward to understand.
    map_opt(
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
                    value(FixedOffset::east_opt(HOUR), tag_no_case(b"A")),
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
                    value(FixedOffset::west_opt(HOUR), tag_no_case(b"N")),
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
        ),
        |tz| tz)(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn date_parsed_printed(date: &[u8], printed: &[u8], parsed: DateTime) {
        assert_eq!(date_time(date).unwrap(), (&b""[..], parsed.clone()));
        let mut v = Vec::new();
        parsed.print(&mut v).unwrap();
        assert_eq!(String::from_utf8_lossy(&v), String::from_utf8_lossy(printed));
    }


    #[test]
    fn test_date_time_rfc_strict() {
        date_parsed_printed(
            b"Fri, 21 Nov 1997 09:55:06 -0600",
            b"21 Nov 1997 09:55:06 -0600",
            DateTime(
                FixedOffset::west_opt(6 * HOUR)
                    .unwrap()
                    .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_received() {
        date_parsed_printed(
            b"Sun, 18 Jun 2023 15:39:08 +0200 (CEST)",
            b"18 Jun 2023 15:39:08 +0200",
            DateTime(
                FixedOffset::east_opt(2 * HOUR)
                    .unwrap()
                    .with_ymd_and_hms(2023, 6, 18, 15, 39, 8)
                    .unwrap()
            ),
        );
    }

    #[test]
    fn test_date_time_rfc_ws() {
        date_parsed_printed(
                r#"Thu,
         13
           Feb
             1969
         23:32
                  -0330 (Newfoundland Time)"#
                    .as_bytes(),
            b"13 Feb 1969 23:32:00 -0330",
            DateTime(
                FixedOffset::west_opt(3 * HOUR + 30 * MIN)
                    .unwrap()
                    .with_ymd_and_hms(1969, 2, 13, 23, 32, 00)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_rfc_obs() {
        date_parsed_printed(
            b"21 Nov 97 09:55:06 GMT",
            b"21 Nov 1997 09:55:06 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_3digit_year() {
        date_parsed_printed(
            b"21 Nov 103 09:55:06 UT",
            b"21 Nov 2003 09:55:06 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2003, 11, 21, 9, 55, 6)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_rfc_obs_ws() {
        date_parsed_printed(
            b"Fri, 21 Nov 1997 09(comment):   55  :  06 -0600",
            b"21 Nov 1997 09:55:06 -0600",
            DateTime(
                FixedOffset::west_opt(6 * HOUR)
                    .unwrap()
                    .with_ymd_and_hms(1997, 11, 21, 9, 55, 6)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_2digit_year() {
        date_parsed_printed(
            b"21 Nov 23 09:55:06Z",
            b"21 Nov 2023 09:55:06 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 9, 55, 6)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_military_zone_east() {
        ["a", "B", "c", "D", "e", "F", "g", "H", "i", "K", "l", "M"]
            .iter()
            .enumerate()
            .for_each(|(i, x)| {
                assert_eq!(
                    date_time(format!("1 Jan 22 08:00:00 {}", x).as_bytes()),
                    Ok((
                        &b""[..],
                        DateTime(
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
    fn test_date_time_military_zone_west() {
        ["N", "O", "P", "q", "r", "s", "T", "U", "V", "w", "x", "y"]
            .iter()
            .enumerate()
            .for_each(|(i, x)| {
                assert_eq!(
                    date_time(format!("1 Jan 22 08:00:00 {}", x).as_bytes()),
                    Ok((
                        &b""[..],
                        DateTime(
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
    fn test_date_time_gmt() {
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 +0000",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 -0000",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 Z",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 GMT",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 UT",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 UTC",
            b"21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_usa() {
        date_parsed_printed(
            b"21 Nov 2023 4:4:4 CST",
            b"21 Nov 2023 04:04:04 -0600",
            DateTime(
                FixedOffset::west_opt(6 * HOUR)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 4, 4, 4)
                    .unwrap()
            )
        );
    }
}
