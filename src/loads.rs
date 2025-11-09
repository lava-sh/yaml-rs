use std::borrow::Cow;

use atoi::atoi;
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyDate, PyDateTime, PyDelta, PyDict, PyList, PyTzInfo},
    IntoPyObjectExt,
};
use saphyr::{Scalar, ScanError, Yaml};
use saphyr_parser::ScalarStyle;

pub(crate) fn yaml_to_python<'py>(
    py: Python<'py>,
    docs: Vec<Yaml<'_>>,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if docs.is_empty() {
        return Ok(PyDict::new(py).into_any());
    }
    _yaml_to_python(py, &docs[0], parse_datetime, false)
}

fn _yaml_to_python<'py>(
    py: Python<'py>,
    value: &Yaml<'_>,
    parse_datetime: bool,
    _tagged_string: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Yaml::Value(scalar) => match scalar {
            Scalar::Null => Ok(py.None().into_bound(py)),
            Scalar::Boolean(bool) => bool.into_bound_py_any(py),
            Scalar::Integer(int) => int.into_bound_py_any(py),
            Scalar::FloatingPoint(float) => float.into_inner().into_bound_py_any(py),
            Scalar::String(str) => {
                let _str = str.as_ref();
                if parse_datetime
                    && !_tagged_string
                    && let Ok(Some(dt)) = _parse_datetime(py, _str)
                {
                    return Ok(dt);
                }
                _str.into_bound_py_any(py)
            }
        },
        Yaml::Sequence(sequence) => {
            let py_list = PyList::empty(py);
            for item in sequence {
                py_list.append(_yaml_to_python(py, item, parse_datetime, false)?)?;
            }
            Ok(py_list.into_any())
        }
        Yaml::Mapping(map) => {
            let py_dict = PyDict::new(py);
            for (k, v) in map {
                py_dict.set_item(
                    yaml_key_to_string(k)?,
                    _yaml_to_python(py, v, parse_datetime, false)?,
                )?;
            }
            Ok(py_dict.into_any())
        }
        Yaml::Representation(cow, style, tag) => {
            if cow.is_empty() && tag.is_none() && *style == ScalarStyle::Plain {
                return Ok(py.None().into_bound(py));
            }

            if let Some(tag_ref) = tag.as_ref() {
                let tag = tag_ref.as_ref();

                if tag.handle.is_empty() && tag.suffix == "!" {
                    return cow.as_ref().into_bound_py_any(py);
                }
                if let Some(scalar) = Scalar::parse_from_cow_and_metadata(
                    Cow::Borrowed(cow.as_ref()),
                    *style,
                    Some(tag_ref),
                ) {
                    let is_str_tag = tag.handle == "tag:yaml.org,2002:" && tag.suffix == "str";
                    return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, is_str_tag);
                }
            } else if let Some(scalar) =
                Scalar::parse_from_cow_and_metadata(Cow::Borrowed(cow.as_ref()), *style, None)
            {
                return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, false);
            }
            cow.as_ref().into_bound_py_any(py)
        }
        Yaml::Tagged(tag, node) => {
            let is_str_tag =
                tag.as_ref().handle == "tag:yaml.org,2002:" && tag.as_ref().suffix == "str";

            _yaml_to_python(py, node, parse_datetime, is_str_tag)
        }
        Yaml::Alias(_) | Yaml::BadValue => Ok(py.None().into_bound(py)),
    }
}

fn yaml_key_to_string(key: &Yaml) -> PyResult<String> {
    match key {
        Yaml::Value(scalar) => match scalar {
            Scalar::String(str) => Ok(str.as_ref().to_string()),
            Scalar::Integer(int) => Ok(int.to_string()),
            Scalar::FloatingPoint(float) => Ok(float.to_string()),
            Scalar::Boolean(bool) => Ok(bool.to_string()),
            Scalar::Null => Ok("null".to_string()),
        },
        Yaml::Representation(cow, _, _) => Ok(cow.as_ref().to_string()),
        _ => Err(crate::YAMLDecodeError::new_err(
            "Complex YAML keys (sequences/mappings) are not supported",
        )),
    }
}

fn _parse_datetime<'py>(py: Python<'py>, s: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let bytes = s.as_bytes();

    if bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' {
        unsafe {
            let year = _parse_digits(bytes, 0, 4) as i32;
            let month = _parse_digits(bytes, 5, 2) as u8;
            let day = _parse_digits(bytes, 8, 2) as u8;
            return Ok(Some(PyDate::new(py, year, month, day)?.into_any()));
        }
    }

    let mut spaces = 0;
    let mut last_space = 0;
    let mut dt_end = bytes.len();
    let mut tz_start = None;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' {
            spaces += 1;
            last_space = i;
        }
    }

    if spaces == 2 {
        dt_end = last_space;
        tz_start = Some(last_space + 1);
    } else if let Some(pos) = bytes.iter().rposition(|&b| b == b'+') {
        dt_end = pos;
        tz_start = Some(pos);
    } else if bytes.last() == Some(&b'Z') {
        dt_end = bytes.len() - 1;
        tz_start = Some(bytes.len() - 1);
    } else if let Some(pos) = bytes.iter().rposition(|&b| b == b'-')
        && pos > 10
        && pos > 0
        && bytes[pos - 1].is_ascii_digit()
    {
        dt_end = pos;
        tz_start = Some(pos);
    }

    let sep_pos = bytes[..dt_end]
        .iter()
        .position(|&b| b == b'T' || b == b't' || b == b' ');
    if sep_pos.is_none() || sep_pos.unwrap() < 10 {
        return Ok(None);
    }
    let sep_pos = sep_pos.unwrap();

    if bytes[4] != b'-' || bytes[7] != b'-' {
        return Ok(None);
    }

    unsafe {
        let year = _parse_digits(bytes, 0, 4) as i32;
        let month = _parse_digits(bytes, 5, 2) as u8;
        let day = _parse_digits(bytes, 8, 2) as u8;

        let time_start = sep_pos + 1;
        if time_start + 5 > dt_end || bytes[time_start + 2] != b':' {
            return Ok(None);
        }

        let hour = _parse_digits(bytes, time_start, 2) as u8;
        let minute = _parse_digits(bytes, time_start + 3, 2) as u8;

        let (second, microsecond) = if time_start + 5 < dt_end && bytes[time_start + 5] == b':' {
            let sec = _parse_digits(bytes, time_start + 6, 2) as u8;
            let micro = if time_start + 8 < dt_end && bytes[time_start + 8] == b'.' {
                let frac_start = time_start + 9;
                let frac_end = dt_end.min(frac_start + 6);
                let mut result = 0u32;
                let mut multiplier = 100_000u32;

                for &byte in bytes.iter().skip(frac_start).take(frac_end - frac_start) {
                    let digit = byte.wrapping_sub(b'0');
                    if digit > 9 {
                        break;
                    }
                    result += (digit as u32) * multiplier;
                    multiplier /= 10;
                }
                result
            } else {
                0
            };
            (sec, micro)
        } else {
            (0, 0)
        };

        let tzinfo = if let Some(tz_start) = tz_start {
            let tz_bytes = &bytes[tz_start..];

            if tz_bytes[0] == b'Z' {
                Some(PyTzInfo::utc(py)?.to_owned())
            } else {
                let (sign, offset_bytes) = match tz_bytes[0] {
                    b'+' => (1, &tz_bytes[1..]),
                    b'-' => (-1, &tz_bytes[1..]),
                    _ => return Ok(None),
                };

                let (hours, minutes) = if let Some(colon_pos) =
                    offset_bytes.iter().position(|&b| b == b':')
                {
                    let h = atoi::<i32>(&offset_bytes[..colon_pos])
                        .ok_or_else(|| PyErr::new::<PyValueError, _>("Invalid timezone hour"))?;
                    let m = if colon_pos + 1 < offset_bytes.len() {
                        atoi::<i32>(&offset_bytes[colon_pos + 1..]).unwrap_or(0)
                    } else {
                        0
                    };
                    (h, m)
                } else if offset_bytes.len() <= 2 {
                    let h = atoi::<i32>(offset_bytes)
                        .ok_or_else(|| PyErr::new::<PyValueError, _>("Invalid timezone hour"))?;
                    (h, 0)
                } else {
                    let h = _parse_digits(offset_bytes, 0, 2) as i32;
                    let m = if offset_bytes.len() >= 4 {
                        _parse_digits(offset_bytes, 2, 2) as i32
                    } else {
                        0
                    };
                    (h, m)
                };

                let total_seconds = sign * (hours * 3600 + minutes * 60);
                let (days, seconds) = if total_seconds < 0 {
                    (
                        total_seconds.div_euclid(86400),
                        total_seconds.rem_euclid(86400),
                    )
                } else {
                    (0, total_seconds)
                };

                let py_delta = PyDelta::new(py, days, seconds, 0, false)?;
                Some(PyTzInfo::fixed_offset(py, py_delta)?)
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
                tzinfo.as_ref(),
            )?
            .into_any(),
        ))
    }
}

// https://github.com/rust-lang/rust/blob/1.91.0/library/core/src/num/dec2flt/parse.rs#L9-L26
//
// This is based off the algorithm described in "Fast numeric string to int",
// available here: <https://johnnylee-sde.github.io/Fast-numeric-string-to-int/>.
#[inline]
unsafe fn _parse_digits(bytes: &[u8], start: usize, count: usize) -> u32 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064;
    const MUL2: u64 = 0x0000_2710_0000_0001;

    let mut d = 0u32;
    let mut i = 0;

    while i + 8 <= count {
        unsafe {
            let ptr = bytes.as_ptr().add(start + i);
            let mut tmp = [0u8; 8];
            std::ptr::copy_nonoverlapping(ptr, tmp.as_mut_ptr(), 8);
            let v = u64::from_le_bytes(tmp);
            let mut v = v;
            v -= 0x3030_3030_3030_3030;
            v = (v * 10) + (v >> 8); // will not overflow, fits in 63 bits
            let v1 = (v & MASK).wrapping_mul(MUL1);
            let v2 = ((v >> 16) & MASK).wrapping_mul(MUL2);
            let parsed = ((v1.wrapping_add(v2) >> 32) as u32) as u64;
            d = d.wrapping_mul(100_000_000).wrapping_add(parsed as u32);
        }
        i += 8;
    }

    while i < count {
        d = d * 10 + unsafe { bytes.get_unchecked(start + i).wrapping_sub(b'0') as u32 };
        i += 1;
    }
    d
}

pub(crate) fn format_error(source: &str, error: &ScanError) -> String {
    let marker = error.marker();
    let line = marker.line();
    let col = marker.col() + 1;
    let gutter = line.to_string().len();

    let error_len = error.info().len();
    let base_len = 50;
    let line_len = itoa::Buffer::new().format(line).len();
    let col_len = itoa::Buffer::new().format(col).len();

    let total_len = base_len
        + line_len
        + col_len
        + error_len
        + if let Some(error_line) = source.lines().nth(line - 1) {
            gutter + 3 + line_len + 3 + error_line.len() + 1 + gutter + 2 + marker.col() + 3 + 1
        } else {
            0
        };

    let mut err = String::with_capacity(total_len);

    err.push_str("YAML parse error at line ");
    err.push_str(itoa::Buffer::new().format(line));
    err.push_str(", column ");
    err.push_str(itoa::Buffer::new().format(col));
    err.push('\n');

    if let Some(error_line) = source.lines().nth(line - 1) {
        unsafe {
            let bytes = err.as_mut_vec();
            bytes.reserve(gutter + 3);
            for _ in 0..gutter {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'|');
            bytes.push(b'\n');
        }
        err.push_str(itoa::Buffer::new().format(line));
        err.push_str(" | ");
        err.push_str(error_line);
        err.push('\n');
        unsafe {
            let bytes = err.as_mut_vec();
            let spaces = gutter + 2 + marker.col();
            bytes.reserve(spaces + 3);

            for _ in 0..gutter {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'|');
            for _ in 0..marker.col() {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'^');
            bytes.push(b'\n');
        }
    }
    err.push_str(error.info());
    err
}
