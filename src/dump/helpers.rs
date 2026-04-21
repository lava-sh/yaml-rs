use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    prelude::{PyAnyMethods, PyStringMethods},
    sync::PyOnceLock,
    types::{PyFloat, PyType},
};
use saphyr::{MappingOwned, ScalarOwned, YamlOwned, YamlOwned::Value};

use crate::dump::{dumps::python_to_yaml, normalize::normalize_float};

pub(crate) fn get_decimal(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
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

pub(crate) fn to_yaml_float(float: &Bound<'_, PyFloat>) -> PyResult<String> {
    let py_str = float.str()?;
    let repr = py_str.to_str()?;
    Ok(normalize_float(repr))
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
