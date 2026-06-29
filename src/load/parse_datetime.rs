use pyo3::{
    Bound, PyAny, Python,
    types::{PyDate, PyDateTime, PyDelta, PyTime, PyTzInfo},
};

#[derive(Copy, Clone)]
enum ParsedTimestamp {
    Date(DateParts),
    DateTime(DateTimeParts),
    Time(TimeParts),
}

impl ParsedTimestamp {
    fn into_py(self, py: Python<'_>) -> Option<Bound<'_, PyAny>> {
        match self {
            Self::Date(date) => Some(
                PyDate::new(py, date.year, date.month, date.day)
                    .ok()?
                    .into_any(),
            ),
            Self::DateTime(parts) => parts.into_py(py),
            Self::Time(time) => time.into_py(py),
        }
    }
}

#[derive(Copy, Clone)]
struct DateParts {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Copy, Clone)]
struct DateTimeParts {
    date: DateParts,
    time: TimeParts,
    offset_seconds: Option<i32>,
}

impl DateTimeParts {
    fn into_py(self, py: Python<'_>) -> Option<Bound<'_, PyAny>> {
        let py_tz_info = self.offset_seconds.and_then(|offset_seconds| {
            const SECS_IN_DAY: i32 = 86_400;

            let days = offset_seconds.div_euclid(SECS_IN_DAY);
            let seconds = offset_seconds.rem_euclid(SECS_IN_DAY);
            let py_delta = PyDelta::new(py, days, seconds, 0, false).ok()?;
            PyTzInfo::fixed_offset(py, py_delta).ok()
        });

        Some(
            PyDateTime::new(
                py,
                self.date.year,
                self.date.month,
                self.date.day,
                self.time.hour,
                self.time.minute,
                self.time.second,
                self.time.microsecond,
                py_tz_info.as_ref(),
            )
            .ok()?
            .into_any(),
        )
    }
}

#[derive(Copy, Clone)]
struct TimeParts {
    hour: u8,
    minute: u8,
    second: u8,
    microsecond: u32,
}

impl TimeParts {
    fn into_py(self, py: Python<'_>) -> Option<Bound<'_, PyAny>> {
        Some(
            PyTime::new(
                py,
                self.hour,
                self.minute,
                self.second,
                self.microsecond,
                None,
            )
            .ok()?
            .into_any(),
        )
    }
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
fn parse_fraction(bytes: &[u8], mut index: usize) -> Option<(usize, u32)> {
    if bytes.get(index).copied() != Some(b'.') {
        return Some((index, 0));
    }

    index += 1;
    let first = u32::from(digit(*bytes.get(index)?)?);
    let mut microsecond = first * 100_000;
    index += 1;

    for weight in [10_000_u32, 1_000, 100, 10, 1] {
        if index == bytes.len() {
            return Some((index, microsecond));
        }

        let digit = bytes[index].wrapping_sub(b'0');
        if digit >= 10 {
            return Some((index, microsecond));
        }

        microsecond += u32::from(digit) * weight;
        index += 1;
    }

    while index < bytes.len() {
        let digit = bytes[index].wrapping_sub(b'0');
        if digit >= 10 {
            break;
        }
        index += 1;
    }

    Some((index, microsecond))
}

#[inline]
fn parse_date_or_datetime_bytes(bytes: &[u8]) -> Option<ParsedTimestamp> {
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

    let date = DateParts {
        year: parse_digits!(i32, 4, bytes, 0)?,
        month: parse_digits!(u8, 2, bytes, 5)?,
        day: parse_digits!(u8, 2, bytes, 8)?,
    };

    if bytes.len() == 10 {
        return Some(ParsedTimestamp::Date(date));
    }

    if bytes.len() < 16 {
        return None;
    }

    // SAFETY: `bytes.len() >= 16` verified above.
    let sep = unsafe { *bytes.get_unchecked(10) };
    if !matches!(sep, b'T' | b't' | b' ' | b'\t') {
        return None;
    }

    // SAFETY: `bytes.len() >= 16` verified above.
    if unsafe { *bytes.get_unchecked(13) != b':' } {
        return None;
    }

    let hour = parse_digits!(u8, 2, bytes, 11)?;
    let minute = parse_digits!(u8, 2, bytes, 14)?;

    let mut second = 0;
    let mut index = 16;
    let mut microsecond = 0;
    let mut offset_seconds = None;

    if index == bytes.len() {
        offset_seconds = Some(0);
    } else if bytes[index] == b':' {
        if bytes.len() < 19 {
            return None;
        }

        // SAFETY: `bytes.len() >= 19` verified above.
        if unsafe { *bytes.get_unchecked(16) != b':' } {
            return None;
        }

        second = parse_digits!(u8, 2, bytes, 17)?;
        (index, microsecond) = parse_fraction(bytes, 19)?;
    }

    if index == bytes.len() {
        let time = TimeParts {
            hour,
            minute,
            second,
            microsecond,
        };

        return Some(ParsedTimestamp::DateTime(DateTimeParts {
            date,
            time,
            offset_seconds,
        }));
    }

    let offset_seconds = match bytes[index] {
        b'Z' => (index + 1 == bytes.len()).then_some(0)?,
        b'+' => parse_tz_hm(&bytes[index + 1..])?,
        b'-' => -parse_tz_hm(&bytes[index + 1..])?,
        b' ' | b'\t' => {
            index += 1;
            if index == bytes.len() || matches!(bytes[index], b' ' | b'\t') {
                return None;
            }

            match bytes[index] {
                b'+' => parse_tz_hm(&bytes[index + 1..])?,
                b'-' => -parse_tz_hm(&bytes[index + 1..])?,
                _ => return None,
            }
        }
        _ => return None,
    };

    let time = TimeParts {
        hour,
        minute,
        second,
        microsecond,
    };

    Some(ParsedTimestamp::DateTime(DateTimeParts {
        date,
        time,
        offset_seconds: Some(offset_seconds),
    }))
}

#[inline]
fn parse_time_bytes(bytes: &[u8]) -> Option<ParsedTimestamp> {
    if bytes.len() < 8 {
        return None;
    }

    // SAFETY: `bytes.len() >= 8` verified above.
    if unsafe { *bytes.get_unchecked(2) != b':' || *bytes.get_unchecked(5) != b':' } {
        return None;
    }

    let hour = parse_digits!(u8, 2, bytes, 0)?;
    let minute = parse_digits!(u8, 2, bytes, 3)?;
    let second = parse_digits!(u8, 2, bytes, 6)?;
    let (index, microsecond) = parse_fraction(bytes, 8)?;

    let time = TimeParts {
        hour,
        minute,
        second,
        microsecond,
    };

    (index == bytes.len()).then_some(ParsedTimestamp::Time(time))
}

pub fn parse_py_datetime<'py>(py: Python<'py>, str: &str) -> Option<Bound<'py, PyAny>> {
    parse_date_or_datetime_bytes(str.as_bytes())
        .or_else(|| parse_time_bytes(str.as_bytes()))?
        .into_py(py)
}
