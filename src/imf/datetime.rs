#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "tracing")]
use tracing::warn;
use bounded_static::{IntoBoundedStatic, ToBoundedStatic};
use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, Timelike};
use nom::{
    branch::alt,
    bytes::complete::{is_a, tag, tag_no_case, take_while_m_n},
    character,
    character::complete::{alphanumeric1, digit0},
    combinator::{eof, map, map_opt, opt, value},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::fmt::{Debug, Formatter};

#[cfg(feature = "arbitrary")]
use crate::fuzz_eq::FuzzEq;
use eml_codec_derives::instrument_input;
use crate::print::{Print, Formatter as PFmt};
use crate::text::whitespace::{cfws, fws};

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

// NOTE: must satisfy the following properties:
// - timezone offset: must be a round hours+minutes (no seconds)
// - year must be after 1900 or later
#[derive(Clone, PartialEq)]
pub struct DateTime(pub chrono::DateTime<FixedOffset>);

impl DateTime {
    // Used as placeholder value for a missing or invalid date
    pub fn placeholder() -> Self {
        Self(chrono::DateTime::UNIX_EPOCH.into())
    }
}

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

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for DateTime {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let timestamp: i64 = u.arbitrary()?;
        let d = chrono::DateTime::from_timestamp_secs(timestamp).ok_or(arbitrary::Error::IncorrectFormat)?;
        let tz_mins = u.int_in_range(-24 * 60 + 1 ..= 24 * 60 - 1)?;
        let tz = FixedOffset::east_opt(tz_mins * 60).unwrap();
        let d: chrono::DateTime<FixedOffset> = d.with_timezone(&tz);
        if d.year() < 1900 {
            Ok(DateTime(chrono::DateTime::UNIX_EPOCH.into()))
        } else {
            Ok(DateTime(d))
        }
    }
}
#[cfg(feature = "arbitrary")]
impl FuzzEq for DateTime {
    fn fuzz_eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Print for DateTime {
    fn print(&self, fmt: &mut impl PFmt) {
        // date
        fmt.write_bytes(format!("{},", self.0.weekday()).as_bytes());
        fmt.write_fws();
        fmt.write_bytes(format!("{}", self.0.day()).as_bytes());
        fmt.write_fws();
        fmt.write_bytes(MONTHS[self.0.month0() as usize]);
        fmt.write_fws();
        fmt.write_bytes(format!("{}", self.0.year()).as_bytes());
        fmt.write_fws();
        // time-of-day
        fmt.write_bytes(format!("{:02}", self.0.hour()).as_bytes());
        fmt.write_bytes(b":");
        fmt.write_bytes(format!("{:02}", self.0.minute()).as_bytes());
        fmt.write_bytes(b":");
        fmt.write_bytes(format!("{:02}", self.0.second()).as_bytes());
        fmt.write_fws();
        // zone
        let offset_secs = self.0.offset().local_minus_utc();
        let sign = if offset_secs >= 0 { b"+" } else { b"-" };
        let offset_mins = offset_secs.abs().rem_euclid(HOUR).div_euclid(MIN);
        let offset_hours = offset_secs.abs().div_euclid(HOUR);
        fmt.write_bytes(sign);
        fmt.write_bytes(
            format!("{:02}{:02}", offset_hours, offset_mins).as_bytes()
        );
    }
}

/// Read datetime
///
/// RFC grammar:
/// ```abnf
/// date-time       =   [ day-of-week "," ] date time [CFWS]
/// time            =   time-of-day zone
/// ```
///
/// We additionally allow dates with a missing zone (followed by end of input),
/// which appear in some real world emails.
///
/// ## @FIXME - known bugs
///  
///   - `-0000` means NaiveDateTime, a date without a timezone
/// while this library interprets it as +0000 aka UTC.
///   - Obsolete military zones should be considered as NaiveTime
/// due to an error in RFC0822 but are interpreted as their respective
/// timezone according to the RFC5322 definition
#[instrument_input("tracing")]
pub fn date_time(input: &[u8]) -> IResult<&[u8], DateTime> {
    map_opt(
        terminated(
            tuple((
                opt(terminated(alt((strict_day_of_week, obs_day_of_week)), tag(","))),
                alt((strict_date, obs_date)),
                alt((strict_time_of_day, obs_time_of_day)),
                alt((strict_zone, obs_zone, no_zone_eof)),
            )),
            opt(cfws),
        ),
        |(_, date, time, tz)| {
            date.and_time(time).and_local_timezone(tz).earliest().map(DateTime)
        }
    )(input)
}

///    day-of-week     =   ([FWS] day-name) / obs-day-of-week
#[instrument_input("tracing")]
fn strict_day_of_week(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(opt(fws), day_name)(input)
}

///    obs-day-of-week =   [CFWS] day-name [CFWS]
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
fn strict_date(input: &[u8]) -> IResult<&[u8], NaiveDate> {
    map_opt(tuple((strict_day, month, strict_year)), |(d, m, y)| {
        NaiveDate::from_ymd_opt(y, m, d)
    })(input)
}

///    date            =   day month year
#[instrument_input("tracing")]
fn obs_date(input: &[u8]) -> IResult<&[u8], NaiveDate> {
    map_opt(tuple((obs_day, month, obs_year)), |(d, m, y)| {
        NaiveDate::from_ymd_opt(y, m, d)
    })(input)
}

///    day             =   ([FWS] 1*2DIGIT FWS) / obs-day
#[instrument_input("tracing")]
fn strict_day(input: &[u8]) -> IResult<&[u8], u32> {
    delimited(opt(fws), character::complete::u32, fws)(input)
}

///    obs-day         =   [CFWS] 1*2DIGIT [CFWS]
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
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
#[instrument_input("tracing")]
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

#[instrument_input("tracing")]
fn obs_time_digit(input: &[u8]) -> IResult<&[u8], u32> {
    delimited(opt(cfws), character::complete::u32, opt(cfws))(input)
}

/// Obsolete zones
///
/// ```abnf
///   zone            =   (FWS ( "+" / "-" ) 4DIGIT) / (FWS obs-zone)
/// ```
#[instrument_input("tracing")]
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
                ((dig_zone_hour[0] - 0x30) * 10 + (dig_zone_hour[1] - 0x30)) as i32;
            let zone_min: i32 =
                ((dig_zone_min[0] - 0x30) * 10 + (dig_zone_min[1] - 0x30)) as i32;
            // consider zone_hour is to be taken modulo 24h...
            let zone_hour: i32 = zone_hour.rem_euclid(24);
            // RFC5322 mandates that zone_min is between 00 and 59; reject the
            // input if not
            if zone_min >= 60 { return None }
            match op {
                b"+" => FixedOffset::east_opt(zone_hour * HOUR + zone_min * MIN),
                b"-" => FixedOffset::west_opt(zone_hour * HOUR + zone_min * MIN),
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
#[instrument_input("tracing")]
fn obs_zone(input: &[u8]) -> IResult<&[u8], FixedOffset> {
    // The writing of this function is volontarily verbose
    // to keep it straightforward to understand.
    preceded(
        opt(fws),
        map_opt(alphanumeric1, |zname: &[u8]| {
            let zname = zname.to_ascii_lowercase();
            match zname.as_slice() {
                // Legacy UTC/GMT
                b"utc" | b"ut" | b"gmt" => FixedOffset::west_opt(0 * HOUR),
                // USA Timezones
                b"edt" => FixedOffset::west_opt(4 * HOUR),
                b"est" | b"cdt" => FixedOffset::west_opt(5 * HOUR),
                b"cst" | b"mdt" => FixedOffset::west_opt(6 * HOUR),
                b"mst" | b"pdt" => FixedOffset::west_opt(7 * HOUR),
                b"pst" => FixedOffset::west_opt(8 * HOUR),
                // Military Timezone UTC
                b"z" => FixedOffset::west_opt(0 * HOUR),
                // Military Timezones East
                b"a" => FixedOffset::east_opt(1 * HOUR),
                b"b" => FixedOffset::east_opt(2 * HOUR),
                b"c" => FixedOffset::east_opt(3 * HOUR),
                b"d" => FixedOffset::east_opt(4 * HOUR),
                b"e" => FixedOffset::east_opt(5 * HOUR),
                b"f" => FixedOffset::east_opt(6 * HOUR),
                b"g" => FixedOffset::east_opt(7 * HOUR),
                b"h" => FixedOffset::east_opt(8 * HOUR),
                b"i" => FixedOffset::east_opt(9 * HOUR),
                b"k" => FixedOffset::east_opt(10 * HOUR),
                b"l" => FixedOffset::east_opt(11 * HOUR),
                b"m" => FixedOffset::east_opt(12 * HOUR),
                // Military Timezones West
                b"n" => FixedOffset::west_opt(1 * HOUR),
                b"o" => FixedOffset::west_opt(2 * HOUR),
                b"p" => FixedOffset::west_opt(3 * HOUR),
                b"q" => FixedOffset::west_opt(4 * HOUR),
                b"r" => FixedOffset::west_opt(5 * HOUR),
                b"s" => FixedOffset::west_opt(6 * HOUR),
                b"t" => FixedOffset::west_opt(7 * HOUR),
                b"u" => FixedOffset::west_opt(8 * HOUR),
                b"v" => FixedOffset::west_opt(9 * HOUR),
                b"w" => FixedOffset::west_opt(10 * HOUR),
                b"x" => FixedOffset::west_opt(11 * HOUR),
                b"y" => FixedOffset::west_opt(12 * HOUR),
                // Unknown timezone
                _ => FixedOffset::west_opt(0 * HOUR),
            }
        })
    )(input)
}

// This is a hack to handle dates that do not specify a timezone. Unfortunately
// this is quite common.
fn no_zone_eof(input: &[u8]) -> IResult<&[u8], FixedOffset> {
    #[cfg(feature = "tracing-recover")]
    warn!("missing zone from date-time");
    map_opt(value(FixedOffset::west_opt(0 * HOUR), pair(opt(cfws), eof)), |tz| tz)(input)
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use crate::print::tests::print_to_vec;

    fn date_parsed_printed(date: &[u8], printed: &[u8], parsed: DateTime) {
        assert_eq!(date_time(date).unwrap(), (&b""[..], parsed.clone()));
        let reprinted = print_to_vec(parsed);
        assert_eq!(String::from_utf8_lossy(&reprinted), String::from_utf8_lossy(printed));
    }


    #[test]
    fn test_date_time_rfc_strict() {
        date_parsed_printed(
            b"Fri, 21 Nov 1997 09:55:06 -0600",
            b"Fri, 21 Nov 1997 09:55:06 -0600",
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
            b"Sun, 18 Jun 2023 15:39:08 +0200",
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
            b"Thu, 13 Feb 1969 23:32:00 -0330",
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
            b"Fri, 21 Nov 1997 09:55:06 +0000",
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
            b"Fri, 21 Nov 2003 09:55:06 +0000",
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
            b"Fri, 21 Nov 1997 09:55:06 -0600",
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
            b"Tue, 21 Nov 2023 09:55:06 +0000",
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
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 -0000",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 Z",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 GMT",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 UT",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 UTC",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
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
            b"Tue, 21 Nov 2023 04:04:04 -0600",
            DateTime(
                FixedOffset::west_opt(6 * HOUR)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 4, 4, 4)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_oob_zone_hours() {
        date_parsed_printed(
            b"26 Aug 2316 09:06:21 -4508",
            b"Sat, 26 Aug 2316 09:06:21 -2108",
            DateTime(
                FixedOffset::west_opt(21 * HOUR + 08 * MIN)
                    .unwrap()
                    .with_ymd_and_hms(2316, 08, 26, 9, 6, 21)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_oob_zone_mins() {
        assert!(date_time(b"26 Aug 2316 09:06:21 -2160").is_err());
    }

    #[test]
    fn test_date_time_no_zone() {
        date_parsed_printed(
            b"21 Nov 2023 07:07:07 ",
            b"Tue, 21 Nov 2023 07:07:07 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2023, 11, 21, 7, 7, 7)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_date_time_unknown_zone() {
        date_parsed_printed(
            b" Mon, 20 Nov 1995 16:54:06 MET",
            b"Mon, 20 Nov 1995 16:54:06 +0000",
            DateTime(
                FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(1995, 11, 20, 16, 54, 06)
                    .unwrap()
            )
        );
    }
}
