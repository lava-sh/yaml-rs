use pyo3::{
    Bound, Py, PyAny, PyResult, Python, intern,
    prelude::{PyAnyMethods, PyStringMethods},
    sync::PyOnceLock,
    types::{PyDateTime, PyDelta, PyDeltaAccess, PyFloat, PyType, PyTzInfo},
};
use saphyr::{MappingOwned, ScalarOwned, YamlOwned, YamlOwned::Value};
use simdutf8::basic::from_utf8;

use crate::{
    YAMLEncodeError,
    dump::{dumps::python_to_yaml, normalize::normalize_float},
};

pub(crate) fn get_decimal(py: Python<'_>) -> PyResult<(&Bound<'_, PyAny>, &Bound<'_, PyType>)> {
    static DECIMAL: PyOnceLock<(Py<PyAny>, Py<PyType>)> = PyOnceLock::new();

    DECIMAL
        .get_or_try_init(py, || {
            let isinstance = py
                .import("builtins")?
                .getattr("isinstance")
                .map(Bound::unbind)?;

            let decimal = py
                .import("decimal")?
                .getattr("Decimal")?
                .cast_into::<PyType>()?
                .unbind();

            Ok((isinstance, decimal))
        })
        .map(|(isinstance, decimal)| (isinstance.bind(py), decimal.bind(py)))
}

pub(crate) fn get_utc_offset<'py>(
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

pub(crate) fn to_yaml_float(float: &Bound<'_, PyFloat>) -> PyResult<String> {
    let py_str = float.str()?;
    let repr = py_str.to_str()?;
    Ok(normalize_float(repr))
}

pub(crate) fn format_ms(buf: &mut String, microsecond: u32, min_len: usize) -> PyResult<()> {
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

pub(crate) fn sequence_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
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

pub(crate) fn set_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
where
    I: IntoIterator<Item = Bound<'py, PyAny>>,
{
    let mut mapping = MappingOwned::with_capacity(len);
    for item in items {
        mapping.insert(python_to_yaml(&item)?, Value(ScalarOwned::Null));
    }
    Ok(YamlOwned::Mapping(mapping))
}
