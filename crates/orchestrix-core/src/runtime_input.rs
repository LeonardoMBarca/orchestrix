//! Mandatory preproduction ingress profile, independent of semantic wire 1.1.
//! The lexical guard is iterative and does not replace JSON syntax validation.
use super::{error, ensure, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, SecondsFormat, Utc};

pub const INPUT_PROFILE_ID: &str = "orchestrix-runtime-json";
pub const INPUT_PROFILE_REVISION: u32 = 1;

fn hex_quad(bytes: &[u8], start: usize) -> Result<u16> {
    let digits = bytes.get(start..start + 4).ok_or_else(|| error("protocol"))?;
    let mut value = 0_u16;
    for digit in digits {
        let part = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return Err(error("protocol")),
        };
        value = value * 16 + u16::from(part);
    }
    Ok(value)
}

/// Check every raw string/number, including discarded or overwritten fields.
/// Rust strings already contain Unicode scalar values; escaped UTF-16 code
/// units must additionally form adjacent high/low pairs. No payload tree or
/// nesting-depth limit is introduced here. Serde validates the full syntax.
pub(super) fn validate_frame(frame: &str) -> Result<()> {
    let bytes = frame.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => {
                cursor += 1;
                let mut closed = false;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'"' => { cursor += 1; closed = true; break; }
                        b'\\' => {
                            cursor += 1;
                            let escape = *bytes.get(cursor).ok_or_else(|| error("protocol"))?;
                            if escape == b'u' {
                                let unit = hex_quad(bytes, cursor + 1)?;
                                cursor += 5;
                                if (0xd800..=0xdbff).contains(&unit) {
                                    ensure(bytes.get(cursor..cursor + 2) == Some(b"\\u"), "protocol")?;
                                    let low = hex_quad(bytes, cursor + 2)?;
                                    ensure((0xdc00..=0xdfff).contains(&low), "protocol")?;
                                    cursor += 6;
                                } else {
                                    ensure(!(0xdc00..=0xdfff).contains(&unit), "protocol")?;
                                }
                            } else {
                                ensure(b"\"\\/bfnrt".contains(&escape), "protocol")?;
                                cursor += 1;
                            }
                        }
                        0..=0x1f => return Err(error("protocol")),
                        _ => cursor += 1,
                    }
                }
                ensure(closed, "protocol")?;
            }
            b'-' | b'0'..=b'9' => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len() && (bytes[cursor].is_ascii_digit() || b".eE+-".contains(&bytes[cursor])) {
                    cursor += 1;
                }
                let number = frame[start..cursor].parse::<f64>().map_err(|_| error("protocol"))?;
                ensure(number.is_finite(), "protocol")?;
            }
            _ => cursor += 1,
        }
    }
    Ok(())
}

fn decimal(bytes: &[u8], start: usize, length: usize) -> Result<u32> {
    let digits = bytes.get(start..start + length).ok_or_else(|| error("protocol"))?;
    ensure(digits.iter().all(u8::is_ascii_digit), "protocol")?;
    Ok(digits.iter().fold(0, |value, digit| value * 10 + u32::from(digit - b'0')))
}

/// Exact Gregorian timestamp grammar with an explicit, known UTC offset.
/// Fractions are truncated to milliseconds; the UTC result stays in the
/// four-digit year domain so normalized values can be accepted again.
pub(super) fn normalize_timestamp(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    ensure((20..=35).contains(&bytes.len()), "protocol")?;
    ensure(bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' && bytes[13] == b':' && bytes[16] == b':', "protocol")?;
    let year = decimal(bytes, 0, 4)? as i32;
    let month = decimal(bytes, 5, 2)?;
    let day = decimal(bytes, 8, 2)?;
    let hour = decimal(bytes, 11, 2)?;
    let minute = decimal(bytes, 14, 2)?;
    let second = decimal(bytes, 17, 2)?;
    ensure(hour <= 23 && minute <= 59 && second <= 59, "protocol")?;
    let mut cursor = 19;
    let mut millis = 0;
    if bytes[cursor] == b'.' {
        cursor += 1;
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() { cursor += 1; }
        let count = cursor - start;
        ensure((1..=9).contains(&count), "protocol")?;
        for position in 0..3 {
            millis = millis * 10 + if position < count { u32::from(bytes[start + position] - b'0') } else { 0 };
        }
    }
    let offset = match bytes.get(cursor) {
        Some(b'Z') => { ensure(cursor + 1 == bytes.len(), "protocol")?; 0 }
        Some(sign @ (b'+' | b'-')) => {
            ensure(cursor + 6 == bytes.len() && bytes[cursor + 3] == b':', "protocol")?;
            let hours = decimal(bytes, cursor + 1, 2)?;
            let minutes = decimal(bytes, cursor + 4, 2)?;
            ensure(hours <= 23 && minutes <= 59 && !(*sign == b'-' && hours == 0 && minutes == 0), "protocol")?;
            let total = i64::from(hours * 60 + minutes);
            if *sign == b'-' { -total } else { total }
        }
        _ => return Err(error("protocol")),
    };
    let local = NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|date| date.and_hms_milli_opt(hour, minute, second, millis))
        .ok_or_else(|| error("protocol"))?;
    let utc = local.checked_sub_signed(Duration::minutes(offset)).ok_or_else(|| error("protocol"))?;
    ensure((0..=9999).contains(&utc.year()), "protocol")?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(utc, Utc).to_rfc3339_opts(SecondsFormat::Millis, true))
}
