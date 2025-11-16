mod decoder;
mod dumps;
mod loads;

use std::borrow::Cow;

use crate::{
    decoder::encode,
    dumps::python_to_yaml,
    loads::{format_error, yaml_to_python},
};

use pyo3::{
    create_exception,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyString,
};

#[cfg(feature = "default")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

create_exception!(yaml_rs, YAMLDecodeError, PyValueError);
create_exception!(yaml_rs, YAMLEncodeError, PyTypeError);

#[pyfunction]
fn _load(
    py: Python,
    obj: Py<PyAny>,
    parse_datetime: bool,
    encoding: Option<String>,
    encoder_errors: Option<String>,
) -> PyResult<Py<PyAny>> {
    let obj_bound = obj.bind_borrowed(py);
    let py = obj_bound.py();

    let data: Cow<[u8]> = if let Ok(string) = obj_bound.cast::<PyString>() {
        let path = string.to_str()?;
        Cow::Owned(py.detach(|| std::fs::read(path))?)
    } else {
        obj_bound.extract().or_else(|_| {
            obj_bound
                .call_method0("read")?
                .extract::<Vec<u8>>()
                .map(Cow::Owned)
        })?
    };

    let s = py
        .detach(|| encode(&data, encoding.as_deref(), encoder_errors.as_deref()))
        .map_err(|err| {
            PyErr::new::<PyValueError, _>(format!("Failed to encode bytes to UTF-8 string: {err}"))
        })?;

    _loads(py, &s, parse_datetime)
}

#[pyfunction]
fn _loads(py: Python, s: &str, parse_datetime: bool) -> PyResult<Py<PyAny>> {
    let yaml = py
        .detach(|| {
            let mut loader = saphyr::YamlLoader::default();
            loader.early_parse(false);
            let mut parser = saphyr_parser::Parser::new_from_str(s);
            parser.load(&mut loader, true)?;
            Ok::<_, saphyr_parser::ScanError>(loader.into_documents())
        })
        .map_err(|err| YAMLDecodeError::new_err(format_error(s, &err)))?;
    Ok(yaml_to_python(py, yaml, parse_datetime)?.unbind())
}

#[pyfunction]
fn _dumps(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut yaml = String::new();
    let mut emitter = saphyr::YamlEmitter::new(&mut yaml);
    emitter.multiline_strings(true);
    emitter
        .dump(&(&python_to_yaml(obj)?).into())
        .map_err(|err| YAMLDecodeError::new_err(err.to_string()))?;
    Ok(yaml)
}

#[pymodule(name = "_yaml_rs")]
fn yaml_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(_load, m)?)?;
    m.add_function(wrap_pyfunction!(_loads, m)?)?;
    m.add_function(wrap_pyfunction!(_dumps, m)?)?;
    m.add("_version", env!("CARGO_PKG_VERSION"))?;
    m.add("YAMLDecodeError", m.py().get_type::<YAMLDecodeError>())?;
    m.add("YAMLEncodeError", m.py().get_type::<YAMLEncodeError>())?;
    Ok(())
}
