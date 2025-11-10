use std::borrow::Cow;

use atoi::atoi;
use pyo3::{
    IntoPyObjectExt,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDate, PyDateTime, PyDelta, PyDict, PyFrozenSet, PyList, PySet, PyTuple, PyTzInfo},
};
use saphyr::{Scalar, ScanError, Yaml};
use saphyr_parser::ScalarStyle;

pub(crate) fn yaml_to_python<'py>(
    py: Python<'py>,
    docs: Vec<Yaml<'_>>,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if docs.is_empty() {
        return Ok(py.None().into_bound(py));
    }

    if docs.len() == 1 {
        return _yaml_to_python(py, &docs[0], parse_datetime, false);
    }

    let py_list = PyList::empty(py);
    for doc in &docs {
        py_list.append(_yaml_to_python(py, doc, parse_datetime, false)?)?;
    }
    Ok(py_list.into_any())
}

fn _yaml_to_python<'py>(
    py: Python<'py>,
    value: &Yaml<'_>,
    parse_datetime: bool,
    _tagged_string: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Yaml::Value(scalar) => _scalar(py, scalar, parse_datetime, _tagged_string),
        Yaml::Sequence(sequence) => {
            let py_list = PyList::empty(py);
            for item in sequence {
                py_list.append(_yaml_to_python(py, item, parse_datetime, false)?)?;
            }
            Ok(py_list.into_any())
        }
        Yaml::Mapping(mapping) => {
            if mapping.is_empty() {
                return Ok(PyDict::new(py).into_any());
            }

            let is_set = mapping.values().all(|v| match v {
                Yaml::Value(Scalar::Null) => true,
                Yaml::Representation(cow, _, _) => cow.as_ref() == "~",
                _ => false,
            });

            if is_set {
                let py_set = PySet::empty(py)?;
                for (k, _) in mapping {
                    py_set.add(_yaml_key(py, k, parse_datetime)?)?;
                }
                Ok(py_set.into_any())
            } else {
                let py_dict = PyDict::new(py);
                for (k, v) in mapping {
                    py_dict.set_item(
                        _yaml_key(py, k, parse_datetime)?,
                        _yaml_to_python(py, v, parse_datetime, false)?,
                    )?;
                }
                Ok(py_dict.into_any())
            }
        }
        Yaml::Representation(cow, style, tag) => {
            if cow.is_empty() && tag.is_none() && *style == ScalarStyle::Plain {
                return Ok(py.None().into_bound(py));
            }

            if let Some(tag_ref) = tag.as_ref() {
                if tag_ref.handle.is_empty() && tag_ref.suffix == "!" {
                    return cow.as_ref().into_bound_py_any(py);
                }
                if let Some(scalar) = Scalar::parse_from_cow_and_metadata(
                    Cow::Borrowed(cow.as_ref()),
                    *style,
                    Some(tag_ref),
                ) {
                    let is_str_tag =
                        tag_ref.handle == "tag:yaml.org,2002:" && tag_ref.suffix == "str";
                    return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, is_str_tag);
                }
            } else if *style == ScalarStyle::Plain {
                let scalar = Scalar::parse_from_cow(Cow::Borrowed(cow.as_ref()));
                return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, false);
            } else {
                return cow.as_ref().into_bound_py_any(py);
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

fn _scalar<'py>(
    py: Python<'py>,
    scalar: &Scalar<'_>,
    parse_datetime: bool,
    _tagged_string: bool,
) -> PyResult<Bound<'py, PyAny>> {
    // Core Schema: https://yaml.org/spec/1.2.2/#103-core-schema
    match scalar {
        // Regular expression: null | Null | NULL | ~
        Scalar::Null => Ok(py.None().into_bound(py)),
        // Regular expression: true | True | TRUE | false | False | FALSE
        Scalar::Boolean(bool) => bool.into_bound_py_any(py),
        // i64
        Scalar::Integer(int) => int.into_bound_py_any(py),
        // f64
        Scalar::FloatingPoint(float) => float.into_inner().into_bound_py_any(py),
        Scalar::String(str) => {
            let str_ref = str.as_ref();
            // FIXME
            match str_ref {
                "Null" => return Ok(py.None().into_bound(py)),
                "True" | "TRUE" => return true.into_bound_py_any(py),
                "False" | "FALSE" => return false.into_bound_py_any(py),
                _ => {}
            }

            if parse_datetime && !_tagged_string {
                match _parse_datetime(py, str_ref) {
                    Ok(Some(dt)) => return Ok(dt),
                    Err(e) if e.is_instance_of::<PyValueError>(py) => return Err(e),
                    Err(_) | Ok(None) => {}
                }
            }
            str_ref.into_bound_py_any(py)
        }
    }
}

fn _yaml_key<'py>(
    py: Python<'py>,
    key: &Yaml,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match key {
        Yaml::Value(scalar) => match scalar {
            Scalar::String(str) => str.as_ref().into_bound_py_any(py),
            Scalar::Integer(int) => int.into_bound_py_any(py),
            Scalar::FloatingPoint(float) => float.into_inner().into_bound_py_any(py),
            Scalar::Boolean(bool) => bool.into_bound_py_any(py),
            Scalar::Null => Ok(py.None().into_bound(py)),
        },
        Yaml::Representation(cow, style, tag) => {
            if let Some(scalar) = Scalar::parse_from_cow_and_metadata(
                Cow::Borrowed(cow.as_ref()),
                *style,
                tag.as_ref(),
            ) {
                _scalar(py, &scalar, parse_datetime, false)
            } else {
                cow.as_ref().into_bound_py_any(py)
            }
        }
        Yaml::Sequence(sequence) => {
            let mut items = Vec::with_capacity(sequence.len());
            for item in sequence {
                items.push(_yaml_key(py, item, parse_datetime)?);
            }
            PyTuple::new(py, &items)?.into_bound_py_any(py)
        }
        Yaml::Mapping(mapping) => {
            let items = PyList::empty(py);
            for (k, v) in mapping {
                let tuple = PyTuple::new(
                    py,
                    &[
                        _yaml_key(py, k, parse_datetime)?,
                        _yaml_to_python(py, v, parse_datetime, false)?,
                    ],
                )?;
                items.append(tuple)?;
            }
            PyFrozenSet::new(py, items)?.into_bound_py_any(py)
        }
        Yaml::Tagged(_, node) => _yaml_key(py, node, parse_datetime),
        Yaml::Alias(_) | Yaml::BadValue => Ok(py.None().into_bound(py)),
    }
}

static _TABLE: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 10 {
        table[(b'0' + i) as usize] = i;
        i += 1;
    }
    table
};

fn _parse_datetime<'py>(py: Python<'py>, s: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let bytes = s.as_bytes();

    // SAFETY
    // bytes: [Y][Y][Y][Y][-][M][M][-][D][D]
    // index:  0  1  2  3  4  5  6  7  8  9
    if bytes.len() == 10
        && unsafe { *bytes.get_unchecked(4) } == b'-'
        && unsafe { *bytes.get_unchecked(7) } == b'-'
    {
        unsafe {
            let year = _parse_digits(bytes, 0, 4) as i32;
            let month = _parse_digits(bytes, 5, 2) as u8;
            let day = _parse_digits(bytes, 8, 2) as u8;
            return Ok(Some(PyDate::new(py, year, month, day)?.into_any()));
        }
    }

    let sep_pos = bytes
        .iter()
        .position(|&b| b == b'T' || b == b't' || b == b' ');
    if sep_pos.is_none() || sep_pos.unwrap() < 10 {
        return Ok(None);
    }
    let sep_pos = sep_pos.unwrap();

    // SAFETY: Already verified sep_pos >= 10, so indices 4 and 7 are valid
    if unsafe { *bytes.get_unchecked(4) } != b'-' || unsafe { *bytes.get_unchecked(7) } != b'-' {
        return Ok(None);
    }

    let mut dt_end = bytes.len();
    let mut tz_start = None;

    // SAFETY: Range (sep_pos + 1..bytes.len()) guarantees valid indices
    // Pattern: "2001-12-14 21:59:43.10  -05"
    //                                ^  ^
    //                                |  timezone indicator (-)
    //                                spaces (skip backward)
    for i in (sep_pos + 1..bytes.len()).rev() {
        let b = unsafe { *bytes.get_unchecked(i) };

        if b == b'Z' {
            // SAFETY: actual_dt_end always >= sep_pos + 1, so actual_dt_end - 1 is valid
            let mut actual_dt_end = i;
            while actual_dt_end > sep_pos + 1
                && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
            {
                actual_dt_end -= 1;
            }
            dt_end = actual_dt_end;
            tz_start = Some(i);
            break;
        } else if b == b'z' {
            return Ok(None);
        } else if b == b'+' {
            let mut actual_dt_end = i;
            while actual_dt_end > sep_pos + 1
                && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
            {
                actual_dt_end -= 1;
            }
            dt_end = actual_dt_end;
            tz_start = Some(i);
            break;
        } else if b == b'-' && i > 10 {
            // Check that '-' is timezone offset, not part of date
            // Skip spaces backward to find last digit of time
            let mut check_pos = i - 1;
            while check_pos > sep_pos && unsafe { *bytes.get_unchecked(check_pos) } == b' ' {
                check_pos -= 1;
            }

            if check_pos > sep_pos && unsafe { *bytes.get_unchecked(check_pos) }.is_ascii_digit() {
                let mut actual_dt_end = i;
                while actual_dt_end > sep_pos + 1
                    && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
                {
                    actual_dt_end -= 1;
                }
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
        }
    }

    let time_start = sep_pos + 1;
    // SAFETY: time_start + 2 must be < dt_end for valid access
    if time_start + 5 > dt_end || unsafe { *bytes.get_unchecked(time_start + 2) } != b':' {
        return Ok(None);
    }

    // SAFETY: All indices verified above, access to bytes is safe
    unsafe {
        let year = _parse_digits(bytes, 0, 4) as i32;
        let month = _parse_digits(bytes, 5, 2) as u8;
        let day = _parse_digits(bytes, 8, 2) as u8;

        let hour = _parse_digits(bytes, time_start, 2) as u8;
        let minute = _parse_digits(bytes, time_start + 3, 2) as u8;

        // Parse seconds and microseconds if present
        // Pattern: "HH:MM:SS.ffffff"
        //               ^  ^
        //               |  fractional part
        //               seconds
        let (second, microsecond) = if time_start + 5 < dt_end
            && *bytes.get_unchecked(time_start + 5) == b':'
        {
            let sec = _parse_digits(bytes, time_start + 6, 2) as u8;
            let micro = if time_start + 8 < dt_end && *bytes.get_unchecked(time_start + 8) == b'.' {
                let frac_start = time_start + 9;
                let frac_end = dt_end.min(frac_start + 6);

                // SAFETY: frac_start..frac_end guaranteed within bytes bounds
                let mut result = 0u32;
                let mut multiplier = 100_000u32;

                for i in frac_start..frac_end {
                    let byte = *bytes.get_unchecked(i);
                    if byte == b' ' {
                        return Ok(None);
                    }
                    let digit = _TABLE[byte as usize];
                    if digit >= 10 {
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

        // Parse timezone if found
        // Pattern: "...43.10  -05:00"
        //                ^  ^
        //                |  timezone offset
        //                spaces (skip)
        let tzinfo = if let Some(tz_pos) = tz_start {
            let mut tz_actual_start = tz_pos;
            while tz_actual_start < bytes.len() && *bytes.get_unchecked(tz_actual_start) == b' ' {
                tz_actual_start += 1;
            }

            if tz_actual_start >= bytes.len() {
                return Ok(None);
            }

            let tz_bytes = &bytes[tz_actual_start..];

            // SAFETY: tz_bytes is non-empty (verified above)
            if *tz_bytes.get_unchecked(0) == b'Z' {
                Some(PyTzInfo::utc(py)?.to_owned())
            } else if *tz_bytes.get_unchecked(0) == b'z' {
                return Ok(None);
            } else {
                let (sign, offset_bytes) = match *tz_bytes.get_unchecked(0) {
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
// available here: https://johnnylee-sde.github.io/Fast-numeric-string-to-int/
#[inline]
unsafe fn _parse_digits(bytes: &[u8], start: usize, count: usize) -> u32 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064;
    const MUL2: u64 = 0x0000_2710_0000_0001;

    let mut d = 0u32;
    let mut i = 0;

    while i + 8 <= count {
        // SAFETY: `i + 8 <= count` ensures we have at least 8 bytes available.
        // `start + i` is within bounds since caller guarantees `start + count <= bytes.len()`.
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
        d = d * 10
            + unsafe {
                // SAFETY: `i < count` and `start + count <= bytes.len()`
                // ensures `start + i` is a valid index.
                bytes.get_unchecked(start + i).wrapping_sub(b'0') as u32
            };
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
            // SAFETY: We only push valid ASCII bytes (spaces, '|', '\n') to the Vec<u8>.
            // String's UTF-8 invariant is maintained because all bytes are valid UTF-8.
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
            // SAFETY: We only push valid ASCII bytes (spaces, '|', '^', '\n') to the Vec<u8>.
            // All ASCII bytes are valid UTF-8, so String's invariant is preserved.
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
