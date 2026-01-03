use std::borrow::Cow;

use atoi::atoi;
use memchr::{memchr, memchr3};
use pyo3::{
    IntoPyObjectExt,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDate, PyDateTime, PyDelta, PyDict, PyFrozenSet, PyList, PySet, PyTuple, PyTzInfo},
};
use saphyr::{Scalar, ScalarStyle, Tag, Yaml};

use crate::YAMLDecodeError;

pub(crate) fn yaml_to_python<'py>(
    py: Python<'py>,
    docs: &[Yaml<'_>],
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => to_python(py, &docs[0], parse_datetime, false),
        _ => {
            let py_list = PyList::empty(py);
            for doc in docs {
                py_list.append(to_python(py, doc, parse_datetime, false)?)?;
            }
            Ok(py_list.into_any())
        }
    }
}

fn to_python<'py>(
    py: Python<'py>,
    value: &Yaml<'_>,
    parse_datetime: bool,
    tagged_string: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Yaml::Value(v) => scalar(py, v, parse_datetime, tagged_string),
        Yaml::Sequence(sequence) => {
            let py_list = PyList::empty(py);
            for item in sequence {
                py_list.append(to_python(py, item, parse_datetime, false)?)?;
            }
            Ok(py_list.into_any())
        }
        Yaml::Mapping(mapping) => {
            let len = mapping.len();

            if len == 0 {
                return Ok(PyDict::new(py).into_any());
            }

            let (all_nulls, has_null_key) = mapping.iter().fold((true, false), |(a, h), (k, v)| {
                (
                    a && (matches!(v, Yaml::Value(Scalar::Null))
                        || matches!(v, Yaml::Representation(cow, _, _) if cow.as_ref() == "~")),
                    h || (matches!(k, Yaml::Value(Scalar::Null))
                        || matches!(k, Yaml::Representation(cow, _, _) if null_tag_value(cow))),
                )
            });

            if all_nulls && !has_null_key && len > 1 {
                let py_set = PySet::empty(py)?;
                for (k, _) in mapping {
                    py_set.add(yaml_key(py, k, parse_datetime)?)?;
                }
                Ok(py_set.into_any())
            } else {
                let py_dict = PyDict::new(py);
                for (k, v) in mapping {
                    py_dict.set_item(
                        yaml_key(py, k, parse_datetime)?,
                        to_python(py, v, parse_datetime, false)?,
                    )?;
                }
                Ok(py_dict.into_any())
            }
        }
        Yaml::Representation(cow, style, tag) => {
            if cow.is_empty() && tag.is_none() && *style == ScalarStyle::Plain {
                return Ok(py.None().into_bound(py));
            }

            if tag.is_none() {
                if *style == ScalarStyle::Plain {
                    let scalar = Scalar::parse_from_cow(Cow::Borrowed(cow));
                    return to_python(py, &Yaml::Value(scalar), parse_datetime, false);
                }
                return cow.into_bound_py_any(py);
            }

            let tag_ref = tag.as_ref().unwrap();

            // ```yaml
            // x: ! 15 # `15` must be string {"x": "15"}
            // y: ! true # {"x": "true"}
            // ```
            if tag_ref.handle.is_empty() && tag_ref.suffix == "!" {
                return cow.into_bound_py_any(py);
            }

            if tag_ref.is_yaml_core_schema() && *style == ScalarStyle::Plain {
                if tag_ref.suffix == "null" && (cow.is_empty() || null_tag_value(cow)) {
                    return Ok(py.None().into_bound(py));
                }

                if let Some(scalar) =
                    Scalar::parse_from_cow_and_metadata(Cow::Borrowed(cow), *style, Some(tag_ref))
                {
                    return to_python(
                        py,
                        &Yaml::Value(scalar),
                        parse_datetime,
                        is_str_tag(tag_ref),
                    );
                }

                let error_msg = if cow.is_empty() {
                    format!("Invalid tag: '!!{suffix}'", suffix = tag_ref.suffix)
                } else {
                    format!(
                        "Invalid value '{invalid}' for '!!{suffix}' tag",
                        invalid = cow,
                        suffix = tag_ref.suffix
                    )
                };

                return Err(YAMLDecodeError::new_err(error_msg));
            }

            cow.into_bound_py_any(py)
        }
        Yaml::Tagged(tag, node) => to_python(py, node, parse_datetime, is_str_tag(tag)),
        Yaml::Alias(_) | Yaml::BadValue => Ok(py.None().into_bound(py)),
    }
}

fn is_str_tag(tag: &Tag) -> bool {
    tag.is_yaml_core_schema() && tag.suffix == "str"
}

fn null_tag_value(value: &str) -> bool {
    matches!(value, "~" | "null" | "NULL" | "Null")
}

fn scalar<'py>(
    py: Python<'py>,
    scalar: &Scalar<'_>,
    parse_datetime: bool,
    tagged_string: bool,
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

            if parse_datetime && !tagged_string {
                match parse_py_datetime(py, str_ref) {
                    Ok(Some(dt)) => return Ok(dt),
                    Err(e) if e.is_instance_of::<PyValueError>(py) => return Err(e),
                    Err(_) | Ok(None) => {}
                }
            }
            str_ref.into_bound_py_any(py)
        }
    }
}

fn yaml_key<'py>(py: Python<'py>, key: &Yaml, parse_datetime: bool) -> PyResult<Bound<'py, PyAny>> {
    match key {
        Yaml::Value(scalar) => match scalar {
            Scalar::String(str) => str.into_bound_py_any(py),
            Scalar::Integer(int) => int.into_bound_py_any(py),
            Scalar::FloatingPoint(float) => float.into_inner().into_bound_py_any(py),
            Scalar::Boolean(bool) => bool.into_bound_py_any(py),
            Scalar::Null => Ok(py.None().into_bound(py)),
        },
        Yaml::Representation(cow, style, tag) => {
            if let Some(s) =
                Scalar::parse_from_cow_and_metadata(Cow::Borrowed(cow), *style, tag.as_ref())
            {
                scalar(py, &s, parse_datetime, false)
            } else {
                cow.into_bound_py_any(py)
            }
        }
        Yaml::Sequence(sequence) => {
            let mut items = Vec::with_capacity(sequence.len());
            for item in sequence {
                items.push(yaml_key(py, item, parse_datetime)?);
            }
            PyTuple::new(py, &items)?.into_bound_py_any(py)
        }
        Yaml::Mapping(mapping) => {
            let items = PyList::empty(py);
            for (k, v) in mapping {
                let tuple = PyTuple::new(
                    py,
                    &[
                        yaml_key(py, k, parse_datetime)?,
                        to_python(py, v, parse_datetime, false)?,
                    ],
                )?;
                items.append(tuple)?;
            }
            PyFrozenSet::new(py, items)?.into_bound_py_any(py)
        }
        Yaml::Tagged(_, node) => yaml_key(py, node, parse_datetime),
        Yaml::Alias(_) | Yaml::BadValue => Ok(py.None().into_bound(py)),
    }
}

static TABLE: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 10 {
        table[(b'0' + i) as usize] = i;
        i += 1;
    }
    table
};

fn parse_py_datetime<'py>(py: Python<'py>, s: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
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
    let day = unsafe { parse_digits(bytes, 8, 2) as u8 };
    let month = unsafe { parse_digits(bytes, 5, 2) as u8 };
    let year = unsafe { parse_digits(bytes, 0, 4).cast_signed() };

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

// https://github.com/rust-lang/rust/blob/1.91.1/library/core/src/num/dec2flt/common.rs#L60-L64
#[inline]
fn is_8digits(v: u64) -> bool {
    let a = v.wrapping_add(0x4646_4646_4646_4646);
    let b = v.wrapping_sub(0x3030_3030_3030_3030);
    (a | b) & 0x8080_8080_8080_8080 == 0
}

// This is based off the algorithm described in "Fast numeric string to int",
// available here: https://johnnylee-sde.github.io/Fast-numeric-string-to-int/
//
// https://github.com/rust-lang/rust/blob/1.91.0/library/core/src/num/dec2flt/parse.rs#L9-L26
#[inline]
unsafe fn parse_digits(bytes: &[u8], start: usize, count: usize) -> u32 {
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

            if is_8digits(v) {
                let mut v = v;
                v -= 0x3030_3030_3030_3030;
                v = (v * 10) + (v >> 8); // will not overflow, fits in 63 bits
                let v1 = (v & MASK).wrapping_mul(MUL1);
                let v2 = ((v >> 16) & MASK).wrapping_mul(MUL2);
                let parsed = u64::from((v1.wrapping_add(v2) >> 32) as u32);
                d = d.wrapping_mul(100_000_000).wrapping_add(parsed as u32);
                i += 8;
            } else {
                break;
            }
        }
    }

    while i < count {
        // SAFETY: `i < count` and `start + count <= bytes.len()`
        // ensures `start + i` is a valid index.
        let byte = unsafe { *bytes.get_unchecked(start + i) };
        let digit = byte.wrapping_sub(b'0');
        if digit < 10 {
            d = d * 10 + u32::from(digit);
            i += 1;
        } else {
            break;
        }
    }
    d
}
