//! UTC timestamps on the `time` crate, written byte for byte like chrono did.
//!
//! chrono is unmaintained (RUSTSEC-2026-0543). Its text forms are part of the
//! vault format, the sync protocol and the local store, which sorts on the
//! stored text, so they must not change:
//!
//! - serde (JSON on the wire and in the vault): `2026-11-26T12:58:37.120Z`
//! - [`to_rfc3339`] (local store, mobile DTOs): `2026-11-26T12:58:37.120+00:00`
//!
//! Both write the fraction with 0, 3, 6 or 9 digits, whichever is the
//! shortest that loses nothing (chrono's `SecondsFormat::AutoSi`).

use std::fmt::Write as _;

use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

/// A point in time, always held in UTC.
pub type Timestamp = OffsetDateTime;

/// The current time in UTC.
pub fn now() -> Timestamp {
    OffsetDateTime::now_utc()
}

/// How many digits of the fraction to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fraction {
    /// Whole seconds only.
    Secs,
    /// Always three digits.
    Millis,
    /// Always six digits.
    Micros,
    /// 0, 3, 6 or 9 digits, the shortest that loses nothing.
    Auto,
}

/// Formats like chrono's `to_rfc3339_opts(format, use_z)`.
pub fn format(t: Timestamp, fraction: Fraction, use_z: bool) -> String {
    let t = t.to_offset(UtcOffset::UTC);
    let mut s = String::with_capacity(32);
    let year = t.year();
    if (0..=9999).contains(&year) {
        let _ = write!(s, "{year:04}");
    } else {
        // Same as chrono: a sign and at least four digits outside 0..=9999.
        let _ = write!(s, "{year:+05}");
    }
    let _ = write!(
        s,
        "-{:02}-{:02}T{:02}:{:02}:{:02}",
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    );
    let nanos = t.nanosecond();
    match fraction {
        Fraction::Secs => {}
        Fraction::Millis => {
            let _ = write!(s, ".{:03}", nanos / 1_000_000);
        }
        Fraction::Micros => {
            let _ = write!(s, ".{:06}", nanos / 1_000);
        }
        Fraction::Auto => {
            if nanos == 0 {
            } else if nanos.is_multiple_of(1_000_000) {
                let _ = write!(s, ".{:03}", nanos / 1_000_000);
            } else if nanos.is_multiple_of(1_000) {
                let _ = write!(s, ".{:06}", nanos / 1_000);
            } else {
                let _ = write!(s, ".{nanos:09}");
            }
        }
    }
    s.push_str(if use_z { "Z" } else { "+00:00" });
    s
}

/// Same text as chrono's `DateTime<Utc>::to_rfc3339()`.
pub fn to_rfc3339(t: Timestamp) -> String {
    format(t, Fraction::Auto, false)
}

/// Same text as chrono's serde form of `DateTime<Utc>`.
pub fn to_json(t: Timestamp) -> String {
    format(t, Fraction::Auto, true)
}

/// Formats `t` in UTC with a `time` format description, like chrono's
/// `format("%Y-%m-%d")` on a UTC time. Formatting a UTC time with a
/// four-digit year cannot fail; should it, the result is empty.
pub fn format_utc(
    t: Timestamp,
    items: &[time::format_description::BorrowedFormatItem<'_>],
) -> String {
    t.to_offset(UtcOffset::UTC)
        .format(items)
        .unwrap_or_default()
}

/// Parses RFC 3339 into UTC, as chrono's `parse_from_rfc3339` did.
///
/// Also takes a space instead of `T`, which chrono's serde accepted.
pub fn parse_rfc3339(s: &str) -> Result<Timestamp, time::error::Parse> {
    let parsed = match OffsetDateTime::parse(s, &Rfc3339) {
        Ok(t) => t,
        Err(e) => {
            if s.len() > 10 && s.as_bytes()[10] == b' ' {
                let mut fixed = s.to_owned();
                fixed.replace_range(10..11, "T");
                OffsetDateTime::parse(&fixed, &Rfc3339)?
            } else {
                return Err(e);
            }
        }
    };
    Ok(parsed.to_offset(UtcOffset::UTC))
}

/// Unix seconds, rounded down like chrono's `timestamp()`.
pub fn unix_secs(t: Timestamp) -> i64 {
    t.unix_timestamp()
}

/// Unix milliseconds, rounded down like chrono's `timestamp_millis()`.
pub fn unix_millis(t: Timestamp) -> i64 {
    t.unix_timestamp_nanos().div_euclid(1_000_000) as i64
}

/// From Unix seconds; `None` when out of range.
pub fn from_unix_secs(secs: i64) -> Option<Timestamp> {
    OffsetDateTime::from_unix_timestamp(secs).ok()
}

/// `#[serde(with = "labterm_proto::timestamp::serde")]` for a [`Timestamp`].
pub mod serde {
    use super::{Timestamp, parse_rfc3339, to_json};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    /// Writes the chrono serde form.
    pub fn serialize<S: Serializer>(t: &Timestamp, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&to_json(*t))
    }

    /// Reads RFC 3339 into UTC.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Timestamp, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        parse_rfc3339(&s).map_err(D::Error::custom)
    }

    /// The same for `Option<Timestamp>`.
    pub mod option {
        use super::super::{Timestamp, parse_rfc3339, to_json};
        use serde::{Deserialize, Deserializer, Serializer, de::Error};

        /// Writes `null` or the chrono serde form.
        pub fn serialize<S: Serializer>(t: &Option<Timestamp>, s: S) -> Result<S::Ok, S::Error> {
            match t {
                Some(t) => s.serialize_some(&to_json(*t)),
                None => s.serialize_none(),
            }
        }

        /// Reads `null` or RFC 3339 into UTC.
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
            let s = <Option<std::borrow::Cow<'de, str>>>::deserialize(d)?;
            s.map(|s| parse_rfc3339(&s).map_err(D::Error::custom))
                .transpose()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64, nanos: u32) -> Timestamp {
        OffsetDateTime::from_unix_timestamp(secs)
            .unwrap()
            .replace_nanosecond(nanos)
            .unwrap()
    }

    // Golden strings, printed by chrono 0.4.45 (serde_json and to_rfc3339).
    const GOLDEN: &[(i64, u32, &str, &str)] = &[
        (
            1_795_000_000,
            0,
            "2026-11-18T11:06:40Z",
            "2026-11-18T11:06:40+00:00",
        ),
        (
            1_795_000_000,
            120_000_000,
            "2026-11-18T11:06:40.120Z",
            "2026-11-18T11:06:40.120+00:00",
        ),
        (
            1_795_000_000,
            123_450_000,
            "2026-11-18T11:06:40.123450Z",
            "2026-11-18T11:06:40.123450+00:00",
        ),
        (
            1_795_000_000,
            1,
            "2026-11-18T11:06:40.000000001Z",
            "2026-11-18T11:06:40.000000001+00:00",
        ),
        (0, 0, "1970-01-01T00:00:00Z", "1970-01-01T00:00:00+00:00"),
        (
            -1,
            999_000_000,
            "1969-12-31T23:59:59.999Z",
            "1969-12-31T23:59:59.999+00:00",
        ),
        (
            -62_167_219_201,
            0,
            "-0001-12-31T23:59:59Z",
            "-0001-12-31T23:59:59+00:00",
        ),
    ];

    #[test]
    fn writes_like_chrono() {
        for &(secs, nanos, json, rfc) in GOLDEN {
            let t = at(secs, nanos);
            assert_eq!(to_json(t), json);
            assert_eq!(to_rfc3339(t), rfc);
            // RFC 3339 has four-digit years only; such times never reach us.
            if (0..=9999).contains(&t.year()) {
                assert_eq!(parse_rfc3339(json).unwrap(), t);
                assert_eq!(parse_rfc3339(rfc).unwrap(), t);
            }
        }
    }

    #[test]
    fn fixed_fractions_like_chrono() {
        let t = at(1_795_000_000, 123_456_789);
        assert_eq!(format(t, Fraction::Secs, true), "2026-11-18T11:06:40Z");
        assert_eq!(
            format(t, Fraction::Millis, true),
            "2026-11-18T11:06:40.123Z"
        );
        assert_eq!(
            format(t, Fraction::Micros, false),
            "2026-11-18T11:06:40.123456+00:00"
        );
    }

    #[test]
    fn formats_like_strftime() {
        let t = at(1_795_000_000, 0);
        assert_eq!(
            format_utc(
                t,
                time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]")
            ),
            "2026-11-18 11:06"
        );
    }

    #[test]
    fn millis_round_down() {
        assert_eq!(
            unix_millis(at(1_795_000_000, 123_999_999)),
            1_795_000_000_123
        );
        assert_eq!(unix_millis(at(-1, 999_500_000)), -1);
    }

    #[test]
    fn parses_long_fractions() {
        // Some SAML IdPs send seven digits; chrono read them, truncating past nine.
        assert_eq!(
            parse_rfc3339("2026-11-18T11:06:40.1234567Z").unwrap(),
            at(1_795_000_000, 123_456_700)
        );
    }

    #[test]
    fn parses_offsets_and_space_into_utc() {
        let t = at(1_795_000_000, 0);
        assert_eq!(parse_rfc3339("2026-11-18T12:06:40+01:00").unwrap(), t);
        assert_eq!(parse_rfc3339("2026-11-18 11:06:40Z").unwrap(), t);
        assert_eq!(parse_rfc3339("2026-11-18t11:06:40z").unwrap(), t);
        assert!(parse_rfc3339("2026-11-18T11:06:40").is_err());
    }

    #[test]
    fn serde_round_trip() {
        #[derive(::serde::Serialize, ::serde::Deserialize, PartialEq, Debug)]
        struct S {
            #[serde(with = "super::serde")]
            a: Timestamp,
            #[serde(with = "super::serde::option", default)]
            b: Option<Timestamp>,
        }
        let v = S {
            a: at(1_795_000_000, 120_000_000),
            b: None,
        };
        let j = serde_json::to_string(&v).unwrap();
        assert_eq!(j, r#"{"a":"2026-11-18T11:06:40.120Z","b":null}"#);
        assert_eq!(serde_json::from_str::<S>(&j).unwrap(), v);
        assert_eq!(
            serde_json::from_str::<S>(r#"{"a":"2026-11-18T11:06:40.120Z"}"#).unwrap(),
            v
        );
    }
}
