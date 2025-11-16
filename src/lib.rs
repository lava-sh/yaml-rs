mod decoder;
mod dumps;
mod loads;

use crate::{
    decoder::encode,
    dumps::python_to_yaml,
    loads::{format_error, yaml_to_python},
};

use pyo3::{
    create_exception,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyString},
};

#[cfg(feature = "default")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

create_exception!(yaml_rs, YAMLDecodeError, PyValueError);
create_exception!(yaml_rs, YAMLEncodeError, PyTypeError);

#[pyfunction]
fn _loads(
    py: Python,
    obj: Py<PyAny>,
    parse_datetime: bool,
    encoding: Option<String>,
    encoder_errors: Option<String>,
) -> PyResult<Py<PyAny>> {
    let obj = obj.bind(py);
    let data = if let Ok(str) = obj.cast::<PyString>() {
        // We assume obj to be a path to a file
        std::fs::read(str.to_str()?)?
    } else if let Ok(b) = obj.cast::<PyBytes>() {
        b.as_unbound().extract(py)?
    } else {
        // We assume/expect BinaryIO type. Read the whole file.
        obj.call_method0("read")?.extract()?
    };
    let s = py
        .detach(|| encode(&data, encoding.as_deref(), encoder_errors.as_deref()))
        .map_err(|err| {
            PyErr::new::<PyValueError, _>(format!("Failed to encode data to UTF-8 string: {err}"))
        })?;
    let yaml = py
        .detach(|| {
            let mut loader = saphyr::YamlLoader::default();
            loader.early_parse(false);
            let mut parser = saphyr_parser::Parser::new_from_str(&s);
            parser.load(&mut loader, true)?;
            Ok::<_, saphyr_parser::ScanError>(loader.into_documents())
        })
        .map_err(|err| YAMLDecodeError::new_err(format_error(&s, &err)))?;
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
    m.add_function(wrap_pyfunction!(_loads, m)?)?;
    m.add_function(wrap_pyfunction!(_dumps, m)?)?;
    m.add("_version", env!("CARGO_PKG_VERSION"))?;
    m.add("YAMLDecodeError", m.py().get_type::<YAMLDecodeError>())?;
    m.add("YAMLEncodeError", m.py().get_type::<YAMLEncodeError>())?;
    Ok(())
}
