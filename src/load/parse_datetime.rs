use pyo3::{
    Bound, PyAny, PyResult, Python,
    types::{PyDate, PyDateTime, PyDelta, PyTzInfo},
};

use crate::load::rust_dec2flt::parse_digits;

const SEP: u8 = b':';
const WHITESPACE: u8 = b' ';
const TAB: u8 = b'\t';
const T: u8 = b'T';
const LOWER_T: u8 = b't';
const Z: u8 = b'Z';
const LOWER_Z: u8 = b'z';
const PLUS: u8 = b'+';
const MINUS: u8 = b'-';

static TABLE: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 10 {
        table[(b'0' + i) as usize] = i;
        i += 1;
    }
    table
};

#[inline]
fn trim_trailing_spaces(bytes: &[u8], min_exclusive: usize, mut end: usize) -> usize {
    while end > min_exclusive && bytes[end - 1] == WHITESPACE {
        end -= 1;
    }
    end
}

#[inline]
fn trim_leading_spaces(bytes: &[u8], mut start: usize) -> usize {
    while start < bytes.len() && bytes[start] == WHITESPACE {
        start += 1;
    }
    start
}

#[inline]
fn parse_ascii_i32(bytes: &[u8]) -> Option<i32> {
    if bytes.is_empty() {
        return None;
    }

    let mut value = 0i32;

    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(i32::from(byte - b'0'))?;
    }
    Some(value)
}

#[inline]
fn parse_two_digits(a: u8, b: u8) -> Option<i32> {
    let a_ = TABLE[a as usize];
    let b_ = TABLE[b as usize];
    if a_ < 10 && b_ < 10 {
        Some(i32::from(a_) * 10 + i32::from(b_))
    } else {
        None
    }
}

#[inline]
fn parse_tz_hm(offset_bytes: &[u8]) -> Option<(i32, i32)> {
    match offset_bytes.len() {
        // -5
        1 => parse_ascii_i32(offset_bytes).map(|h| (h, 0)),
        // -05
        2 => parse_two_digits(offset_bytes[0], offset_bytes[1]).map(|h| (h, 0)),
        // -5:30
        4 if offset_bytes[1] == b':' => {
            let h = parse_ascii_i32(&offset_bytes[..1])?;
            let m = parse_two_digits(offset_bytes[2], offset_bytes[3])?;
            Some((h, m))
        }
        // -05:30
        5 if offset_bytes[2] == b':' => {
            let h = parse_two_digits(offset_bytes[0], offset_bytes[1])?;
            let m = parse_two_digits(offset_bytes[3], offset_bytes[4])?;
            Some((h, m))
        }
        _ => None,
    }
}

pub(crate) fn parse_py_datetime<'py>(
    py: Python<'py>,
    str: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    const SECS_IN_DAY: i32 = 86_400;

    let bytes = str.as_bytes();

    if bytes.len() < 10 {
        return Ok(None);
    }
    // bytes: [Y][Y][Y][Y][-][M][M][-][D][D]
    //                     ^        ^
    // index:              4        7
    // SAFETY: `bytes.len()` >= 10 verified above, so indices 4 and 7 are valid.
    if unsafe { !(*bytes.get_unchecked(4) == MINUS && *bytes.get_unchecked(7) == MINUS) } {
        return Ok(None);
    }
    // SAFETY: `bytes.len()` >= 10 and date format verified above.
    // Indices 0..4, 5..7, and 8..10 are all within bounds.
    let day = parse_digits(bytes, 8, 2) as u8;
    let month = parse_digits(bytes, 5, 2) as u8;
    let year = parse_digits(bytes, 0, 4).cast_signed();

    if bytes.len() == 10 {
        return Ok(Some(PyDate::new(py, year, month, day)?.into_any()));
    }

    let sep_pos = match bytes[10..]
        .iter()
        .position(|&byte| matches!(byte, T | LOWER_T | WHITESPACE | TAB))
        .map(|pos| pos + 10)
    {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let mut dt_end = bytes.len();
    let mut tz_start = None;

    for i in (sep_pos + 1..bytes.len()).rev() {
        // SAFETY: i from range (`sep_pos + 1..bytes.len()`), so it's a valid index.
        let b = unsafe { *bytes.get_unchecked(i) };

        match b {
            Z | PLUS => {
                let actual_dt_end = trim_trailing_spaces(bytes, sep_pos + 1, i);
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
            LOWER_Z => return Ok(None),
            MINUS if i > 10 => {
                let mut check_pos = i - 1;
                // SAFETY: Loop condition ensures check_pos > sep_pos >= 0,
                // making check_pos a valid index.
                while check_pos > sep_pos
                    && unsafe { *bytes.get_unchecked(check_pos) } == WHITESPACE
                {
                    check_pos -= 1;
                }
                // SAFETY: check_pos > sep_pos verified by loop condition above,
                // so check_pos is a valid index.
                if check_pos > sep_pos
                    && unsafe { *bytes.get_unchecked(check_pos) }.is_ascii_digit()
                {
                    let actual_dt_end = trim_trailing_spaces(bytes, sep_pos + 1, i);
                    dt_end = actual_dt_end;
                    tz_start = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }

    let time_start = sep_pos + 1;
    // SAFETY: time_start + 2 < dt_end verified by the condition,
    // and dt_end <= `bytes.len()`, so time_start + 2 is a valid index.
    if time_start + 5 > dt_end || unsafe { *bytes.get_unchecked(time_start + 2) } != SEP {
        return Ok(None);
    }

    // SAFETY: All operations within this block are safe because:
    // 1. Date indices (0..4, 5..7, 8..10) verified at function start
    // 2. time_start derived from sep_pos which is a valid index
    // 3. All subsequent indices are bounds-checked before use
    unsafe {
        let hour = parse_digits(bytes, time_start, 2) as u8;
        let minute = parse_digits(bytes, time_start + 3, 2) as u8;

        let (second, microsecond) =
            // SAFETY: time_start + 5 < dt_end verified by condition,
            // and dt_end <= `bytes.len()`, so time_start + 5 is valid.
            if time_start + 5 < dt_end && *bytes.get_unchecked(time_start + 5) == SEP {
                let second = parse_digits(bytes, time_start + 6, 2) as u8;
                // SAFETY: time_start + 8 < dt_end verified by condition,
                // so time_start + 8 is a valid index.
                let microsecond =
                    if time_start + 8 < dt_end && *bytes.get_unchecked(time_start + 8) == b'.' {
                        let frac_start = time_start + 9;
                        let frac_len = (dt_end - frac_start).min(6);

                        if frac_len == 6 {
                            parse_digits(bytes, frac_start, 6)
                        } else {
                            let mut result = 0u32;
                            let mut multiplier = 100_000u32;

                            for i in 0..frac_len {
                                // SAFETY: i < frac_len and frac_len <= dt_end - frac_start,
                                // so frac_start + i < dt_end <= `bytes.len()`.
                                let byte = *bytes.get_unchecked(frac_start + i);
                                if byte == WHITESPACE {
                                    return Ok(None);
                                }
                                let digit = TABLE[byte as usize];
                                if digit >= 10 {
                                    break;
                                }
                                result += u32::from(digit) * multiplier;
                                multiplier /= 10;
                            }
                            result
                        }
                    } else {
                        0
                    };
                (second, microsecond)
            } else {
                (0, 0)
            };

        let tz_info = if let Some(tz_pos) = tz_start {
            let tz_actual_start = trim_leading_spaces(bytes, tz_pos);

            if tz_actual_start >= bytes.len() {
                return Ok(None);
            }

            let tz_bytes = &bytes[tz_actual_start..];
            // SAFETY: tz_actual_start < `bytes.len()` verified above,
            // so tz_bytes is non-empty and index 0 is valid.
            let first_byte = *tz_bytes.get_unchecked(0);

            match first_byte {
                Z => Some(PyTzInfo::utc(py)?.to_owned()),
                PLUS | MINUS => {
                    let sign = if first_byte == PLUS { 1 } else { -1 };
                    let offset_bytes = &tz_bytes[1..];

                    let (hours, minutes) = match parse_tz_hm(offset_bytes) {
                        Some(hm) => hm,
                        None => return Ok(None),
                    };

                    let total_seconds = sign * (hours * 3600 + minutes * 60);
                    let days = total_seconds.div_euclid(SECS_IN_DAY);
                    let seconds = total_seconds.rem_euclid(SECS_IN_DAY);
                    let py_delta = PyDelta::new(py, days, seconds, 0, false)?;
                    Some(PyTzInfo::fixed_offset(py, py_delta)?)
                }
                _ => return Ok(None),
            }
        } else {
            None
        };

        Ok(Some(
            PyDateTime::new(
                py,
                year,
                month,
                day,
                hour,
                minute,
                second,
                microsecond,
                tz_info.as_ref(),
            )?
            .into_any(),
        ))
    }
}
