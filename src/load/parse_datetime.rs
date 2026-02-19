use atoi::atoi;
use memchr::{memchr, memchr3};
use pyo3::{
    Bound, PyAny, PyErr, PyResult, Python,
    exceptions::PyValueError,
    types::{PyDate, PyDateTime, PyDelta, PyTzInfo},
};

use crate::load::rust_dec2flt::parse_digits;

static TABLE: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 10 {
        table[(b'0' + i) as usize] = i;
        i += 1;
    }
    table
};

pub(crate) fn parse_py_datetime<'py>(
    py: Python<'py>,
    s: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    const SECS_IN_DAY: i32 = 86_400;
    const SEP: u8 = b':';
    const WHITESPACE: u8 = b' ';
    const T: u8 = b'T';
    const LOWER_T: u8 = b't';
    const Z: u8 = b'Z';
    const LOWER_Z: u8 = b'z';
    const PLUS: u8 = b'+';
    const MINUS: u8 = b'-';

    let bytes = s.as_bytes();

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

    let sep_pos = match memchr3(T, LOWER_T, WHITESPACE, &bytes[10..]).map(|pos| pos + 10) {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let mut dt_end = bytes.len();
    let mut tz_start = None;

    for i in (sep_pos + 1..bytes.len()).rev() {
        // SAFETY: i from range (`sep_pos + 1..bytes.len()`), so it's a valid index.
        let b = unsafe { *bytes.get_unchecked(i) };

        match b {
            Z => {
                let mut actual_dt_end = i;
                // SAFETY: Loop condition ensures actual_dt_end > sep_pos + 1,
                // so actual_dt_end - 1 >= sep_pos + 1 > 0, making it a valid index.
                while actual_dt_end > sep_pos + 1
                    && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == WHITESPACE
                {
                    actual_dt_end -= 1;
                }
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
            LOWER_Z => return Ok(None),
            PLUS => {
                let mut actual_dt_end = i;
                // SAFETY: Loop condition ensures actual_dt_end > sep_pos + 1,
                // so actual_dt_end - 1 is a valid index.
                while actual_dt_end > sep_pos + 1
                    && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == WHITESPACE
                {
                    actual_dt_end -= 1;
                }
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
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
                    let mut actual_dt_end = i;
                    // SAFETY: Loop condition ensures actual_dt_end > sep_pos + 1,
                    // so actual_dt_end - 1 is a valid index.
                    while actual_dt_end > sep_pos + 1
                        && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == WHITESPACE
                    {
                        actual_dt_end -= 1;
                    }
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
            let mut tz_actual_start = tz_pos;
            // SAFETY: Loop increments tz_actual_start while checking it's < `bytes.len()`,
            // ensuring all accesses are within bounds.
            while tz_actual_start < bytes.len()
                && *bytes.get_unchecked(tz_actual_start) == WHITESPACE
            {
                tz_actual_start += 1;
            }

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

                    let (hours, minutes) = if let Some(colon_pos) = memchr(SEP, offset_bytes) {
                        let h = atoi::<i32>(&offset_bytes[..colon_pos]).ok_or_else(|| {
                            PyErr::new::<PyValueError, _>("Invalid timezone hour")
                        })?;
                        let m = if colon_pos + 1 < offset_bytes.len() {
                            atoi::<i32>(&offset_bytes[colon_pos + 1..]).unwrap_or(0)
                        } else {
                            0
                        };
                        (h, m)
                    } else if offset_bytes.len() <= 2 {
                        let h = atoi::<i32>(offset_bytes).ok_or_else(|| {
                            PyErr::new::<PyValueError, _>("Invalid timezone hour")
                        })?;
                        (h, 0)
                    } else {
                        // SAFETY: `offset_bytes.len()` > 2 verified by else branch,
                        // so indices 0..2 and potentially 2..4 are valid.
                        let h = parse_digits(offset_bytes, 0, 2).cast_signed();
                        let m = if offset_bytes.len() >= 4 {
                            parse_digits(offset_bytes, 2, 2).cast_signed()
                        } else {
                            0
                        };
                        (h, m)
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
