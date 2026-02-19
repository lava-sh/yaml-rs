use std::borrow::Cow;

use atoi::atoi;
use memchr::{memchr, memchr2, memchr3};
use num_bigint::BigInt;
use pyo3::{
    IntoPyObjectExt,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDate, PyDateTime, PyDelta, PyDict, PyFrozenSet, PyList, PySet, PyTuple, PyTzInfo},
};
use rustc_hash::FxHashMap;
use saphyr_parser::{Event, Parser, ScalarStyle, ScanError, Tag};

use crate::rust_dec2flt::parse_digits;

const UNDERSCORE: u8 = b'_';

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Boolean(bool),
    IntegerI64(i64),
    IntegerBig(BigInt),
    Float(f64),
    String(String),
    StringExplicit(String),
    Seq(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

#[derive(Debug)]
enum Frame {
    Seq {
        anchor: usize,
        items: Vec<Value>,
    },
    Map {
        anchor: usize,
        items: Vec<(Value, Value)>,
        pending_key: Option<Value>,
    },
}

#[derive(Debug)]
pub enum BuildError {
    Scan(ScanError),
    Decode(String),
}

impl From<ScanError> for BuildError {
    fn from(err: ScanError) -> Self {
        BuildError::Scan(err)
    }
}

fn is_null(str: &str) -> bool {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: null | Null | NULL | ~
    matches!(str, "null" | "Null" | "NULL" | "~")
}

fn is_bool(str: &str) -> Option<bool> {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: true | True | TRUE | false | False | FALSE
    match str {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

fn normalize_num(str: &str) -> Cow<'_, str> {
    let bytes = str.as_bytes();

    if memchr(UNDERSCORE, bytes).is_none() {
        return Cow::Borrowed(str);
    }

    let mut out = String::with_capacity(str.len());
    for &b in bytes {
        if b != UNDERSCORE {
            out.push(b as char);
        }
    }
    Cow::Owned(out)
}

fn parse_int(str: &str) -> Option<Value> {
    let s = str.trim();
    if s.is_empty() {
        return None;
    }

    let (sign, rest) = match s.as_bytes()[0] {
        b'+' => (1i64, &s[1..]),
        b'-' => (-1i64, &s[1..]),
        _ => (1i64, s),
    };

    let norm = normalize_num(rest);
    let r = norm.as_ref();

    let (radix, digits) =
        if let Some(stripped) = r.strip_prefix("0x").or_else(|| r.strip_prefix("0X")) {
            (16u32, stripped)
        } else if let Some(stripped) = r.strip_prefix("0o").or_else(|| r.strip_prefix("0O")) {
            (8u32, stripped)
        } else if let Some(stripped) = r.strip_prefix("0b").or_else(|| r.strip_prefix("0B")) {
            (2u32, stripped)
        } else {
            (10u32, r)
        };

    if digits.is_empty() {
        return None;
    }

    if radix == 10 {
        if let Ok(i_64) = lexical_core::parse::<i64>(digits.as_bytes()) {
            return Some(Value::IntegerI64(i_64.wrapping_mul(sign)));
        }
    } else if let Ok(i_64) = i64::from_str_radix(digits, radix) {
        return Some(Value::IntegerI64(i_64.wrapping_mul(sign)));
    }

    BigInt::parse_bytes(digits.as_bytes(), radix).map(|b| {
        if sign < 0 {
            Value::IntegerBig(-b)
        } else {
            Value::IntegerBig(b)
        }
    })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SpecialFloat {
    Inf,
    Nan,
}

#[inline]
fn is_inf_nan(bytes: &[u8]) -> Option<(SpecialFloat, bool)> {
    if bytes.is_empty() {
        return None;
    }

    let mut i = 0usize;
    let mut neg = false;

    match bytes[i] {
        b'+' => {
            i += 1;
            if i >= bytes.len() {
                return None;
            }
        }
        b'-' => {
            neg = true;
            i += 1;
            if i >= bytes.len() {
                return None;
            }
        }
        _ => {}
    }

    if bytes[i] == b'.' {
        i += 1;
        if i >= bytes.len() {
            return None;
        }
    }

    let rest = &bytes[i..];
    if rest.len() != 3 {
        return None;
    }

    let a = rest[0];
    let b = rest[1];
    let c = rest[2];

    if matches!(
        (a, b, c),
        (b'i' | b'I', b'n' | b'N', b'f' | b'F')
    ) {
        return Some((SpecialFloat::Inf, neg));
    }

    if matches!(
        (a, b, c),
        (b'n' | b'N', b'a' | b'A', b'n' | b'N')
    ) {
        return Some((SpecialFloat::Nan, neg));
    }

    None
}

fn parse_float(str: &str) -> Option<f64> {
    let trimmed = str.trim();

    if trimmed.is_empty() {
        return None;
    }

    if let Some((kind, neg)) = is_inf_nan(trimmed.as_bytes()) {
        return Some(match kind {
            SpecialFloat::Nan => f64::NAN,
            SpecialFloat::Inf => {
                if neg { f64::NEG_INFINITY } else { f64::INFINITY }
            }
        });
    }

    let norm = normalize_num(trimmed);

    lexical_core::parse::<f64>(norm.as_bytes()).ok()
}

fn resolve_scalar(value: &str, style: ScalarStyle, tag: Option<&Tag>) -> Result<Value, String> {
    if let Some(tag) = tag {
        if tag.is_yaml_core_schema() {
            return match tag.suffix.as_str() {
                "int" => parse_int(value)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!int' tag")),
                "float" => parse_float(value)
                    .map(Value::Float)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!float' tag")),
                "bool" => is_bool(value)
                    .map(Value::Boolean)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!bool' tag")),
                "null" => {
                    if value.is_empty() || is_null(value) {
                        Ok(Value::Null)
                    } else {
                        Err(format!("Invalid value '{value}' for '!!null' tag"))
                    }
                }
                "binary" => Ok(Value::String(value.to_string())),
                "str" => Ok(Value::StringExplicit(value.to_string())),
                _ => Err(format!("Invalid tag: '!!{}'", tag.suffix)),
            };
        }
        return Ok(Value::String(value.to_string()));
    }

    if style == ScalarStyle::Plain {
        let trimmed = value.trim();

        if trimmed.is_empty() || is_null(trimmed) {
            return Ok(Value::Null);
        }

        if let Some(bool) = is_bool(trimmed) {
            return Ok(Value::Boolean(bool));
        }

        if let Some(int) = parse_int(trimmed) {
            return Ok(int);
        }

        let bytes = trimmed.as_bytes();

        if (is_inf_nan(bytes).is_some()
            || memchr(b'.', bytes).is_some()
            || memchr2(b'e', b'E', bytes).is_some())
            && let Some(float) = parse_float(trimmed)
        {
            return Ok(Value::Float(float));
        }
    }

    Ok(Value::String(value.to_string()))
}

pub(crate) fn build_from_events(input: &str) -> Result<Vec<Value>, BuildError> {
    let parser = Parser::new_from_str(input);
    let mut stack = Vec::new();
    let mut docs = Vec::new();
    let mut anchors = FxHashMap::default();
    let mut current_root = None;

    for event_res in parser {
        let (event, _) = event_res?;

        match event {
            Event::StreamStart | Event::StreamEnd | Event::Nothing => {}
            Event::DocumentStart(_) => {
                current_root = None;
                stack.clear();
            }
            Event::DocumentEnd => {
                docs.push(current_root.take().unwrap_or(Value::Null));
            }
            Event::Alias(id) => {
                let value = anchors.get(&id).cloned().unwrap_or(Value::Null);
                push_value(value, &mut stack, &mut current_root);
            }
            Event::Scalar(val, style, anchor_id, tag) => {
                let tag_owned = tag.map(Cow::into_owned);
                let value =
                    resolve_scalar(&val, style, tag_owned.as_ref()).map_err(BuildError::Decode)?;
                if anchor_id != 0 {
                    anchors.insert(anchor_id, value.clone());
                }
                push_value(value, &mut stack, &mut current_root);
            }
            Event::SequenceStart(anchor_id, _) => {
                stack.push(Frame::Seq {
                    anchor: anchor_id,
                    items: Vec::new(),
                });
            }
            Event::SequenceEnd => {
                if let Some(Frame::Seq { anchor, items }) = stack.pop() {
                    let value = Value::Seq(items);
                    if anchor != 0 {
                        anchors.insert(anchor, value.clone());
                    }
                    push_value(value, &mut stack, &mut current_root);
                }
            }
            Event::MappingStart(anchor_id, _) => {
                stack.push(Frame::Map {
                    anchor: anchor_id,
                    items: Vec::new(),
                    pending_key: None,
                });
            }
            Event::MappingEnd => {
                if let Some(Frame::Map { anchor, items, .. }) = stack.pop() {
                    let value = Value::Map(items);
                    if anchor != 0 {
                        anchors.insert(anchor, value.clone());
                    }
                    push_value(value, &mut stack, &mut current_root);
                }
            }
        }
    }

    Ok(docs)
}

fn push_value(value: Value, stack: &mut [Frame], root: &mut Option<Value>) {
    if let Some(top) = stack.last_mut() {
        match top {
            Frame::Seq { items, .. } => items.push(value),
            Frame::Map {
                items, pending_key, ..
            } => {
                if let Some(key) = pending_key.take() {
                    items.push((key, value));
                } else {
                    *pending_key = Some(value);
                }
            }
        }
    } else {
        *root = Some(value);
    }
}

fn value_to_py<'py>(
    py: Python<'py>,
    value: &Value,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Boolean(bool) => bool.into_bound_py_any(py),
        Value::IntegerI64(int_64) => int_64.into_bound_py_any(py),
        Value::IntegerBig(big_int) => big_int.into_bound_py_any(py),
        Value::Float(float) => float.into_bound_py_any(py),
        Value::StringExplicit(str_exp) => str_exp.into_bound_py_any(py),
        Value::String(str) => {
            if parse_datetime && let Ok(Some(dt)) = parse_py_datetime(py, str) {
                return Ok(dt);
            }
            str.into_bound_py_any(py)
        }
        Value::Seq(items) => {
            let py_list = PyList::empty(py);
            for it in items {
                py_list.append(value_to_py(py, it, parse_datetime)?)?;
            }
            Ok(py_list.into_any())
        }
        Value::Map(pairs) => {
            let mut all_nulls = true;
            let mut has_null_key = false;

            for (k, v) in pairs {
                if matches!(k, Value::Null) {
                    has_null_key = true;
                }
                if !matches!(v, Value::Null) {
                    all_nulls = false;
                }
            }

            if all_nulls && !has_null_key && pairs.len() > 1 {
                let py_set = PySet::empty(py)?;
                for (k, _) in pairs {
                    py_set.add(value_to_hashable(py, k, parse_datetime)?)?;
                }
                Ok(py_set.into_any())
            } else {
                let py_dict = PyDict::new(py);
                for (k, v) in pairs {
                    py_dict.set_item(
                        value_to_hashable(py, k, parse_datetime)?,
                        value_to_py(py, v, parse_datetime)?,
                    )?;
                }
                Ok(py_dict.into_any())
            }
        }
    }
}

fn value_to_hashable<'py>(
    py: Python<'py>,
    value: &Value,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Seq(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_hashable(py, item, parse_datetime)?);
            }
            PyTuple::new(py, &out)?.into_bound_py_any(py)
        }
        Value::Map(pairs) => {
            let py_list = PyList::empty(py);
            for (k, v) in pairs {
                let py_tuple = PyTuple::new(
                    py,
                    &[
                        value_to_hashable(py, k, parse_datetime)?,
                        value_to_py(py, v, parse_datetime)?,
                    ],
                )?;
                py_list.append(py_tuple)?;
            }
            PyFrozenSet::new(py, py_list)?.into_bound_py_any(py)
        }
        _ => value_to_py(py, value, parse_datetime),
    }
}

pub(crate) fn to_python<'py>(
    py: Python<'py>,
    docs: &[Value],
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => value_to_py(py, &docs[0], parse_datetime),
        _ => {
            let list = PyList::empty(py);
            for d in docs {
                list.append(value_to_py(py, d, parse_datetime)?)?;
            }
            Ok(list.into_any())
        }
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
