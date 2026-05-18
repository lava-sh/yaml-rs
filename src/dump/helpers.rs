use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    prelude::{PyAnyMethods, PyStringMethods},
    sync::PyOnceLock,
    types::{PyFloat, PyType},
};
use saphyr::{MappingOwned, ScalarOwned, YamlOwned, YamlOwned::Value};
use std::fmt::Write;

use crate::dump::{dumps::python_to_yaml, normalize::normalize_float};

pub fn get_decimal(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static DECIMAL: PyOnceLock<Py<PyType>> = PyOnceLock::new();

    DECIMAL
        .get_or_try_init(py, || {
            let decimal = py
                .import("decimal")?
                .getattr("Decimal")?
                .cast_into::<PyType>()?;
            Ok(decimal.unbind())
        })
        .map(|decimal| decimal.bind(py))
}

pub fn to_yaml_float(float: &Bound<'_, PyFloat>) -> PyResult<String> {
    let py_str = float.str()?;
    let repr = py_str.to_str()?;
    Ok(normalize_float(repr))
}

pub fn sequence_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
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

pub fn set_to_yaml<'py, I>(items: I, len: usize) -> PyResult<YamlOwned>
where
    I: IntoIterator<Item = Bound<'py, PyAny>>,
{
    let mut mapping = MappingOwned::with_capacity(len);
    for item in items {
        mapping.insert(python_to_yaml(&item)?, Value(ScalarOwned::Null));
    }
    Ok(YamlOwned::Mapping(mapping))
}

pub fn has_unsafe_scalar_char(value: &str) -> bool {
    value.chars().any(|ch| {
        matches!(
            ch,
            '\0'..='\u{0008}'
                | '\u{000b}'..='\u{001f}'
                | '\u{007f}'..='\u{009f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{FEFF}'
        )
    })
}

pub fn escape_double_quoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\u{0008}' => escaped.push_str("\\b"),
            '\u{000c}' => escaped.push_str("\\f"),
            '\u{0085}' => escaped.push_str("\\N"),
            '\u{2028}' => escaped.push_str("\\L"),
            '\u{2029}' => escaped.push_str("\\P"),
            '\u{FEFF}' => escaped.push_str("\\uFEFF"),
            '\0'..='\u{001f}' | '\u{007f}'..='\u{009f}' => {
                write!(escaped, "\\u{:04X}", ch as u32).expect("write to String failed");
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}
