use std::{borrow::Cow, fmt::Write};

use pyo3::{
    Bound, Py, PyAny, PyResult, Python, intern,
    sync::PyOnceLock,
    types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDate, PyDateAccess, PyDateTime, PyDelta,
        PyDeltaAccess, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyFrozenSetMethods, PyInt,
        PyList, PyListMethods, PySet, PySetMethods, PyString, PyStringMethods, PyTime,
        PyTimeAccess, PyTuple, PyTupleMethods, PyType, PyTzInfo, PyTzInfoAccess,
    },
};
use saphyr::{MappingOwned, ScalarOwned, ScalarStyle, YamlOwned, YamlOwned::Value};
use simdutf8::basic::from_utf8;

use crate::{
    YAMLEncodeError,
    dump::helpers::{has_nan_payload, normalize_float},
};

pub(crate) fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<YamlOwned> {
    match obj {
        obj if let Ok(str) = obj.cast::<PyString>() => Ok(Value(ScalarOwned::String(
            str.to_string_lossy().into_owned(),
        ))),
        obj if obj.is_none() => Ok(Value(ScalarOwned::Null)),
        obj if let Ok(bool) = obj.cast::<PyBool>() => {
            Ok(Value(ScalarOwned::Boolean(bool.is_true())))
        }
        obj if let Ok(datetime) = obj.cast::<PyDateTime>() => {
            let year = datetime.get_year();
            let month = datetime.get_month();
            let day = datetime.get_day();
            let hour = datetime.get_hour();
            let minute = datetime.get_minute();
            let second = datetime.get_second();
            let microsecond = datetime.get_microsecond();

            let tzinfo = datetime.get_tzinfo();

            let capacity = if tzinfo.is_some() { 35 } else { 26 };
            let mut datetime_str = String::with_capacity(capacity);

            let py = datetime.py();
            let is_utc = match tzinfo {
                Some(ref tz) => PyTzInfo::utc(py)
                    .ok()
                    .and_then(|utc| tz.eq(utc).ok())
                    .unwrap_or(false),
                None => false,
            };

            write!(
                &mut datetime_str,
                "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}",
            )
            .unwrap();

            if microsecond > 0 {
                datetime_str.push('.');
                format_ms(&mut datetime_str, microsecond, if is_utc { 1 } else { 2 })?;
            }

            if let Some(tz) = tzinfo {
                if is_utc {
                    datetime_str.push('Z');
                } else if let Some((offset_hours, offset_minutes)) =
                    get_utc_offset(datetime.py(), &tz, datetime)
                {
                    write!(&mut datetime_str, "{offset_hours:+03}:{offset_minutes:02}").unwrap();
                }
            }

            Ok(Value(ScalarOwned::String(datetime_str)))
        }
        obj if let Ok(time) = obj.cast::<PyTime>() => {
            let hour = time.get_hour();
            let minute = time.get_minute();
            let second = time.get_second();
            let microsecond = time.get_microsecond();

            let mut time_str = String::with_capacity(16);

            write!(&mut time_str, "{hour:02}:{minute:02}:{second:02}").unwrap();

            if microsecond > 0 {
                time_str.push('.');
                format_ms(&mut time_str, microsecond, 1)?;
            }

            Ok(Value(ScalarOwned::String(time_str)))
        }
        obj if let Ok(date) = obj.cast::<PyDate>() => {
            let year = date.get_year();
            let month = date.get_month();
            let day = date.get_day();
            let mut date_str = String::with_capacity(10);
            write!(&mut date_str, "{year:04}-{month:02}-{day:02}").unwrap();
            Ok(Value(ScalarOwned::String(date_str)))
        }
        obj if let Ok(tuple) = obj.cast::<PyTuple>() => sequence_to_yaml(tuple.iter(), tuple.len()),
        obj if let Ok(list) = obj.cast::<PyList>() => sequence_to_yaml(list.iter(), list.len()),
        obj if let Ok(set) = obj.cast::<PySet>() => set_to_yaml(set.iter(), set.len()),
        obj if let Ok(frozenset) = obj.cast::<PyFrozenSet>() => {
            set_to_yaml(frozenset.iter(), frozenset.len())
        }
        obj if let Ok(dict) = obj.cast::<PyDict>() => {
            let len = dict.len();
            if len == 0 {
                return Ok(YamlOwned::Mapping(MappingOwned::new()));
            }
            let mut mapping = MappingOwned::with_capacity(dict.len());
            for (k, v) in dict.iter() {
                mapping.insert(python_to_yaml(&k)?, python_to_yaml(&v)?);
            }
            Ok(YamlOwned::Mapping(mapping))
        }
        obj if get_isinstance(obj.py())?
            .call1((obj, get_decimal_type(obj.py())?))?
            .is_truthy()? =>
        {
            let py_str = obj.str()?;
            Ok(YamlOwned::Representation(
                normalize_decimal(py_str.to_str()?)?.into_owned(),
                ScalarStyle::Plain,
                None,
            ))
        }
        obj if let Ok(int) = obj.cast::<PyInt>() => match int.extract::<i64>() {
            Ok(value) => Ok(Value(ScalarOwned::Integer(value))),
            Err(_) => Ok(YamlOwned::Representation(
                int.str()?.to_str()?.to_owned(),
                ScalarStyle::Plain,
                None,
            )),
        },
        obj if let Ok(float) = obj.cast::<PyFloat>() => Ok(YamlOwned::Representation(
            to_yaml_float(float)?,
            ScalarStyle::Plain,
            None,
        )),
        _ => Err(YAMLEncodeError::new_err(format!(
            "Cannot serialize {obj_type} ({obj_repr}) to YAML",
            obj_type = obj.get_type(),
            obj_repr = obj
                .repr()
                .map_or_else(|_| "<repr failed>".to_string(), |r| r.to_string())
        ))),
    }
}

fn to_yaml_float(float: &Bound<'_, PyFloat>) -> PyResult<String> {
    let py_str = float.str()?;
    let repr = py_str.to_str()?;
    Ok(normalize_float(repr))
}

fn format_ms(buf: &mut String, microsecond: u32, min_len: usize) -> PyResult<()> {
    let mut buffer = itoa::Buffer::new();
    let formatted = buffer.format(microsecond);

    let padding = 6 - formatted.len();
    let mut padded = [b'0'; 6];
    padded[padding..].copy_from_slice(formatted.as_bytes());

    let mut padded_len = 6;
    while padded_len > min_len && padded[padded_len - 1] == b'0' {
        padded_len -= 1;
    }

    let value = from_utf8(&padded[..padded_len])
        .map_err(|err| YAMLEncodeError::new_err(err.to_string()))?;
    buf.push_str(value);
    Ok(())
}

fn get_utc_offset<'py>(
    py: Python<'py>,
    tz: &Bound<'py, PyTzInfo>,
    datetime: &Bound<'py, PyDateTime>,
) -> Option<(i32, i32)> {
    tz.call_method1(intern!(py, "utcoffset"), (datetime,))
        .ok()
        .filter(|d| !d.is_none())
        .and_then(|offset_delta| {
            let delta = offset_delta.cast::<PyDelta>().ok()?;
            let days = delta.get_days();
            let seconds = delta.get_seconds();
            let total_seconds = days * 86400 + seconds;
            let total_minutes = total_seconds / 60;
            Some((total_minutes / 60, (total_minutes % 60).abs()))
        })
}

fn sequence_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
where
    I: IntoIterator<Item = Bound<'py, PyAny>>,
{
    if len == 0 {
        return Ok(YamlOwned::Sequence(Vec::new()));
    }

    let mut sequence = Vec::with_capacity(len);
    for item in items {
        sequence.push(python_to_yaml(&item)?);
    }
    Ok(YamlOwned::Sequence(sequence))
}

fn set_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
where
    I: IntoIterator<Item = Bound<'py, PyAny>>,
{
    let mut mapping = MappingOwned::with_capacity(len);
    for item in items {
        mapping.insert(python_to_yaml(&item)?, Value(ScalarOwned::Null));
    }
    Ok(YamlOwned::Mapping(mapping))
}

fn get_decimal_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static DECIMAL_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

    DECIMAL_TYPE.import(py, "decimal", "Decimal")
}

fn get_isinstance(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    static ISINSTANCE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

    ISINSTANCE
        .get_or_try_init(py, || {
            py.import("builtins")?
                .getattr("isinstance")
                .map(Bound::unbind)
        })
        .map(|f| f.bind(py))
}

fn normalize_decimal(repr: &str) -> PyResult<Cow<'_, str>> {
    let bytes = repr.as_bytes();
    let mut start = 0usize;
    let mut end = bytes.len();

    // SAFETY: `start < end <= bytes.len()` is maintained by the loop conditions.
    while start < end && unsafe { bytes.get_unchecked(start) }.is_ascii_whitespace() {
        start += 1;
    }

    // SAFETY: `end - 1 < bytes.len()` whenever the loop condition holds.
    while start < end && unsafe { bytes.get_unchecked(end - 1) }.is_ascii_whitespace() {
        end -= 1;
    }

    // SAFETY: `start..end` stays within the original string bounds.
    let trimmed = unsafe { repr.get_unchecked(start..end) };
    let bytes = trimmed.as_bytes();

    let mut offset = 0usize;
    let mut neg = false;

    if !bytes.is_empty() {
        // SAFETY: `bytes` is non-empty in this branch, so index `0` is valid.
        match unsafe { *bytes.get_unchecked(0) } {
            b'-' => {
                neg = true;
                offset = 1;
            }
            b'+' => {
                offset = 1;
            }
            _ => {}
        }
    }

    // SAFETY: `offset` is either 0 or 1 and never exceeds `bytes.len()`.
    let rest = unsafe { bytes.get_unchecked(offset..) };
    let len = rest.len();

    if len == 3 {
        // SAFETY: `len == 3`, so indices `0..3` are valid.
        let a = unsafe { *rest.get_unchecked(0) } | 0x20;
        let b = unsafe { *rest.get_unchecked(1) } | 0x20;
        let c = unsafe { *rest.get_unchecked(2) } | 0x20;

        if (a, b, c) == (b'n', b'a', b'n') {
            return Ok(Cow::Borrowed(".nan"));
        }
        if (a, b, c) == (b'i', b'n', b'f') {
            return Ok(if neg {
                Cow::Borrowed("-.inf")
            } else {
                Cow::Borrowed(".inf")
            });
        }
    }

    if len == 4 {
        // SAFETY: `len == 4`, so indices `0..4` are valid.
        let a = unsafe { *rest.get_unchecked(0) } | 0x20;
        let b = unsafe { *rest.get_unchecked(1) } | 0x20;
        let c = unsafe { *rest.get_unchecked(2) } | 0x20;
        let d = unsafe { *rest.get_unchecked(3) } | 0x20;

        if (a, b, c, d) == (b's', b'n', b'a', b'n') {
            return Ok(Cow::Borrowed(".nan"));
        }
    }

    if has_nan_payload(rest, 3, *b"nan")
        || has_nan_payload(rest, 4, *b"snan")
    {
        return Err(YAMLEncodeError::new_err(format!(
            "Cannot serialize invalid decimal.Decimal('{trimmed}') to YAML"
        )));
    }

    if len == 8 {
        let inf = b"infinity";
        let mut matches = true;
        let mut i = 0usize;
        while i < 8 {
            // SAFETY: `len == 8`, so `i < 8` keeps both reads in bounds.
            if (unsafe { *rest.get_unchecked(i) } | 0x20) != unsafe { *inf.get_unchecked(i) } {
                matches = false;
                break;
            }
            i += 1;
        }

        if matches {
            return Ok(if neg {
                Cow::Borrowed("-.inf")
            } else {
                Cow::Borrowed(".inf")
            });
        }
    }

    let mut has_dot = false;
    let mut has_exp = false;
    let mut i = 0usize;

    while i < bytes.len() {
        // SAFETY: the loop condition guarantees `i < bytes.len()`.
        match unsafe { *bytes.get_unchecked(i) } {
            b'.' => has_dot = true,
            b'e' | b'E' => has_exp = true,
            _ => {}
        }
        i += 1;
    }

    if !has_dot && !has_exp {
        let mut normalized = String::with_capacity(trimmed.len() + 2);
        normalized.push_str(trimmed);
        normalized.push_str(".0");
        return Ok(Cow::Owned(normalized));
    }

    Ok(Cow::Borrowed(trimmed))
}
