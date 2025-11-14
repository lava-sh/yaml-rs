use std::borrow::Cow;

use atoi::atoi;
use pyo3::{
    IntoPyObjectExt,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDate, PyDateTime, PyDelta, PyDict, PyFrozenSet, PyList, PySet, PyTuple, PyTzInfo},
};
use saphyr::{Scalar, ScalarStyle, ScanError, Yaml};

pub(crate) fn yaml_to_python<'py>(
    py: Python<'py>,
    docs: Vec<Yaml<'_>>,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => _yaml_to_python(py, &docs[0], parse_datetime, false),
        _ => {
            let py_list = PyList::empty(py);
            for doc in &docs {
                py_list.append(_yaml_to_python(py, doc, parse_datetime, false)?)?;
            }
            Ok(py_list.into_any())
        }
    }
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

            let (all_nulls, has_null_key) =
                mapping.iter().fold((true, false), |(_a, _h), (k, v)| {
                    (
                        _a && (matches!(v, Yaml::Value(Scalar::Null))
                            || matches!(v, Yaml::Representation(cow, _, _) if cow.as_ref() == "~")),
                        _h || (matches!(k, Yaml::Value(Scalar::Null))
                            || matches!(k, Yaml::Representation(cow, _, _)
                            if matches!(cow.as_ref(), "~" | "null" | "NULL" | "Null"))),
                    )
                });

            if all_nulls && !has_null_key {
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
                    let is_str_tag = tag_ref.is_yaml_core_schema() && tag_ref.suffix == "str";
                    return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, is_str_tag);
                }
            } else if *style == ScalarStyle::Plain {
                let scalar = Scalar::parse_from_cow(Cow::Borrowed(cow.as_ref()));
                return _yaml_to_python(py, &Yaml::Value(scalar), parse_datetime, false);
            }
            cow.as_ref().into_bound_py_any(py)
        }
        Yaml::Tagged(tag, node) => {
            let is_str_tag = tag.as_ref().is_yaml_core_schema() && tag.as_ref().suffix == "str";
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

    if bytes.len() < 10 {
        return Ok(None);
    }
    // bytes: [Y][Y][Y][Y][-][M][M][-][D][D]
    //                     ^        ^
    // index:              4        7
    // SAFETY: `bytes.len()` >= 10 verified above, so indices 4 and 7 are valid.
    if unsafe { !(*bytes.get_unchecked(4) == b'-' && *bytes.get_unchecked(7) == b'-') } {
        return Ok(None);
    }
    // SAFETY: `bytes.len()` >= 10 and date format verified above.
    // Indices 0..4, 5..7, and 8..10 are all within bounds.
    let day = unsafe { _parse_digits(bytes, 8, 2) as u8 };
    let month = unsafe { _parse_digits(bytes, 5, 2) as u8 };
    let year = unsafe { _parse_digits(bytes, 0, 4) as i32 };

    if bytes.len() == 10 {
        return Ok(Some(PyDate::new(py, year, month, day)?.into_any()));
    }

    let sep_pos = match memchr::memchr3(b'T', b't', b' ', &bytes[10..]).map(|pos| pos + 10) {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let mut dt_end = bytes.len();
    let mut tz_start = None;

    for i in (sep_pos + 1..bytes.len()).rev() {
        // SAFETY: i from range (`sep_pos + 1..bytes.len()`), so it's a valid index.
        let b = unsafe { *bytes.get_unchecked(i) };

        match b {
            b'Z' => {
                let mut actual_dt_end = i;
                // SAFETY: Loop condition ensures actual_dt_end > sep_pos + 1,
                // so actual_dt_end - 1 >= sep_pos + 1 > 0, making it a valid index.
                while actual_dt_end > sep_pos + 1
                    && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
                {
                    actual_dt_end -= 1;
                }
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
            b'z' => return Ok(None),
            b'+' => {
                let mut actual_dt_end = i;
                // SAFETY: Loop condition ensures actual_dt_end > sep_pos + 1,
                // so actual_dt_end - 1 is a valid index.
                while actual_dt_end > sep_pos + 1
                    && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
                {
                    actual_dt_end -= 1;
                }
                dt_end = actual_dt_end;
                tz_start = Some(i);
                break;
            }
            b'-' if i > 10 => {
                let mut check_pos = i - 1;
                // SAFETY: Loop condition ensures check_pos > sep_pos >= 0,
                // making check_pos a valid index.
                while check_pos > sep_pos && unsafe { *bytes.get_unchecked(check_pos) } == b' ' {
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
                        && unsafe { *bytes.get_unchecked(actual_dt_end - 1) } == b' '
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
    if time_start + 5 > dt_end || unsafe { *bytes.get_unchecked(time_start + 2) } != b':' {
        return Ok(None);
    }

    // SAFETY: All operations within this block are safe because:
    // 1. Date indices (0..4, 5..7, 8..10) verified at function start
    // 2. time_start derived from sep_pos which is a valid index
    // 3. All subsequent indices are bounds-checked before use
    unsafe {
        let hour = _parse_digits(bytes, time_start, 2) as u8;
        let minute = _parse_digits(bytes, time_start + 3, 2) as u8;

        let (second, microsecond) =
            // SAFETY: time_start + 5 < dt_end verified by condition,
            // and dt_end <= `bytes.len()`, so time_start + 5 is valid.
            if time_start + 5 < dt_end && *bytes.get_unchecked(time_start + 5) == b':' {
                let _second = _parse_digits(bytes, time_start + 6, 2) as u8;
                // SAFETY: time_start + 8 < dt_end verified by condition,
                // so time_start + 8 is a valid index.
                let _microsecond =
                    if time_start + 8 < dt_end && *bytes.get_unchecked(time_start + 8) == b'.' {
                        let frac_start = time_start + 9;
                        let frac_len = (dt_end - frac_start).min(6);

                        if frac_len == 6 {
                            _parse_digits(bytes, frac_start, 6)
                        } else {
                            let mut result = 0u32;
                            let mut multiplier = 100_000u32;

                            for i in 0..frac_len {
                                // SAFETY: i < frac_len and frac_len <= dt_end - frac_start,
                                // so frac_start + i < dt_end <= `bytes.len()`.
                                let byte = *bytes.get_unchecked(frac_start + i);
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
                        }
                    } else {
                        0
                    };
                (_second, _microsecond)
            } else {
                (0, 0)
            };

        let tz_info = if let Some(tz_pos) = tz_start {
            let mut tz_actual_start = tz_pos;
            // SAFETY: Loop increments tz_actual_start while checking it's < `bytes.len()`,
            // ensuring all accesses are within bounds.
            while tz_actual_start < bytes.len() && *bytes.get_unchecked(tz_actual_start) == b' ' {
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
                b'Z' => Some(PyTzInfo::utc(py)?.to_owned()),
                b'z' => return Ok(None),
                b'+' | b'-' => {
                    let sign = if first_byte == b'+' { 1 } else { -1 };
                    let offset_bytes = &tz_bytes[1..];

                    let (hours, minutes) =
                        if let Some(colon_pos) = memchr::memchr(b':', offset_bytes) {
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

    let error_line = source.lines().nth(line - 1);

    let total_len = base_len
        + line_len
        + col_len
        + error_len
        + if let Some(error_line) = error_line {
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

    if let Some(error_line) = error_line {
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
