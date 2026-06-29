use pyo3::{
    Bound, PyAny, Python,
    types::{PyDate, PyDateTime, PyDelta, PyTzInfo},
};

#[derive(Copy, Clone)]
struct DateTimeParts {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    microsecond: u32,
    offset_seconds: Option<i32>,
    has_time: bool,
}

#[inline]
fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

#[inline]
fn digit(byte: u8) -> Option<u8> {
    let digit = byte.wrapping_sub(b'0');
    (digit < 10).then_some(digit)
}

macro_rules! parse_digits {
    ($ty:ty, $len:literal, $bytes:expr, $start:expr) => {{
        let bytes = $bytes;
        let start: usize = $start;

        debug_assert!(
            start
                .checked_add($len)
                .is_some_and(|end| end <= bytes.len())
        );

        let mut index: usize = 0;
        let mut value: $ty = 0;

        'parse: {
            while index < $len {
                // SAFETY: Caller guarantees `start..start + $len` is in-bounds.
                let digit = unsafe { (*bytes.get_unchecked(start + index)).wrapping_sub(b'0') };
                if digit >= 10 {
                    break 'parse None;
                }

                let digit = digit as $ty;
                value = match value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
                {
                    Some(value) => value,
                    None => break 'parse None,
                };
                index += 1;
            }

            Some(value)
        }
    }};
}

#[inline]
fn parse_tz_hm(offset_bytes: &[u8]) -> Option<i32> {
    match offset_bytes {
        [hour_0] => Some(i32::from(digit(*hour_0)?) * 3600),
        [hour_0, hour_1] => {
            let hour = i32::from(digit(*hour_0)?) * 10 + i32::from(digit(*hour_1)?);
            Some(hour * 3600)
        }
        [hour_0, b':', minute_0, minute_1] => {
            let hour = i32::from(digit(*hour_0)?);
            let minute = i32::from(digit(*minute_0)?) * 10 + i32::from(digit(*minute_1)?);
            (minute <= 59).then_some(hour * 3600 + minute * 60)
        }
        [hour_0, hour_1, b':', minute_0, minute_1] => {
            let hour = i32::from(digit(*hour_0)?) * 10 + i32::from(digit(*hour_1)?);
            let minute = i32::from(digit(*minute_0)?) * 10 + i32::from(digit(*minute_1)?);
            (minute <= 59).then_some(hour * 3600 + minute * 60)
        }
        _ => None,
    }
}

#[inline]
fn parse_datetime_bytes(bytes: &[u8]) -> Option<DateTimeParts> {
    if bytes.len() < 10 {
        return None;
    }

    // bytes: [Y][Y][Y][Y][-][M][M][-][D][D]
    //                     ^        ^
    // index:              4        7
    // SAFETY: `bytes.len() >= 10` verified above.
    if unsafe { *bytes.get_unchecked(4) != b'-' || *bytes.get_unchecked(7) != b'-' } {
        return None;
    }

    let year = parse_digits!(i32, 4, bytes, 0)?;
    let month = parse_digits!(u8, 2, bytes, 5)?;
    let day = parse_digits!(u8, 2, bytes, 8)?;

    if bytes.len() == 10 {
        return Some(DateTimeParts {
            year,
            month,
            day,
            hour: 0,
            minute: 0,
            second: 0,
            microsecond: 0,
            offset_seconds: None,
            has_time: false,
        });
    }

    if bytes.len() < 19 {
        return None;
    }

    // SAFETY: `bytes.len() >= 19` verified above.
    let sep = unsafe { *bytes.get_unchecked(10) };
    if !matches!(sep, b'T' | b't' | b' ' | b'\t') {
        return None;
    }

    // SAFETY: `bytes.len() >= 19` verified above.
    if unsafe { *bytes.get_unchecked(13) != b':' || *bytes.get_unchecked(16) != b':' } {
        return None;
    }

    let hour = parse_digits!(u8, 2, bytes, 11)?;
    let minute = parse_digits!(u8, 2, bytes, 14)?;
    let second = parse_digits!(u8, 2, bytes, 17)?;

    let mut index = 19;
    let mut microsecond: u32 = 0;

    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let first = u32::from(digit(*bytes.get(index)?)?);
        microsecond = first * 100_000;
        index += 1;

        let mut scale: u32 = 10_000;
        while index < bytes.len() {
            let digit = bytes[index].wrapping_sub(b'0');
            if digit >= 10 {
                break;
            }
            if scale != 0 {
                microsecond += u32::from(digit) * scale;
                scale /= 10;
            }
            index += 1;
        }
    }

    if index == bytes.len() {
        return Some(DateTimeParts {
            year,
            month,
            day,
            hour,
            minute,
            second,
            microsecond,
            offset_seconds: None,
            has_time: true,
        });
    }

    while index < bytes.len() && is_space(bytes[index]) {
        index += 1;
    }

    if index == bytes.len() {
        return None;
    }

    let offset_seconds = match bytes[index] {
        b'Z' => (index + 1 == bytes.len()).then_some(0)?,
        b'+' => parse_tz_hm(&bytes[index + 1..])?,
        b'-' => -parse_tz_hm(&bytes[index + 1..])?,
        _ => return None,
    };

    Some(DateTimeParts {
        year,
        month,
        day,
        hour,
        minute,
        second,
        microsecond,
        offset_seconds: Some(offset_seconds),
        has_time: true,
    })
}

pub fn parse_py_datetime<'py>(py: Python<'py>, str: &str) -> Option<Bound<'py, PyAny>> {
    const SECS_IN_DAY: i32 = 86_400;

    let parts = parse_datetime_bytes(str.as_bytes())?;

    if !parts.has_time {
        return match PyDate::new(py, parts.year, parts.month, parts.day) {
            Ok(py_date) => Some(py_date.into_any()),
            Err(_) => None,
        };
    }

    if let Some(offset_seconds) = parts.offset_seconds {
        let days = offset_seconds.div_euclid(SECS_IN_DAY);
        let seconds = offset_seconds.rem_euclid(SECS_IN_DAY);

        let py_delta = match PyDelta::new(py, days, seconds, 0, false) {
            Ok(py_delta) => py_delta,
            Err(_) => return None,
        };

        let py_tz_info = match PyTzInfo::fixed_offset(py, py_delta) {
            Ok(py_tz_info) => py_tz_info,
            Err(_) => return None,
        };

        return make_py_datetime(
            py,
            (parts.year, parts.month, parts.day),
            (parts.hour, parts.minute, parts.second, parts.microsecond),
            Some(&py_tz_info),
        );
    }

    make_py_datetime(
        py,
        (parts.year, parts.month, parts.day),
        (parts.hour, parts.minute, parts.second, parts.microsecond),
        None,
    )
}

fn make_py_datetime<'py>(
    py: Python<'py>,
    date: (i32, u8, u8),
    time: (u8, u8, u8, u32),
    tzinfo: Option<&Bound<'py, PyTzInfo>>,
) -> Option<Bound<'py, PyAny>> {
    let (year, month, day) = date;
    let (hour, minute, second, microsecond) = time;
    Some(
        PyDateTime::new(
            py,
            year,
            month,
            day,
            hour,
            minute,
            second,
            microsecond,
            tzinfo,
        )
        .ok()?
        .into_any(),
    )
}
