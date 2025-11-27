mod decoder;
mod dumps;
mod loads;

use std::borrow::Cow;

use pyo3::{
    create_exception,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyString,
};

use crate::{
    decoder::encode,
    dumps::python_to_yaml,
    loads::{format_error, yaml_to_python},
};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

create_exception!(yaml_rs, YAMLDecodeError, PyValueError);
create_exception!(yaml_rs, YAMLEncodeError, PyTypeError);

#[pyfunction(name = "_load")]
fn load(
    py: Python,
    obj: &Bound<'_, PyAny>,
    parse_datetime: bool,
    encoding: Option<&str>,
    encoder_errors: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let data: Cow<[u8]> = if let Ok(string) = obj.cast::<PyString>() {
        let path = string.to_str()?;
        Cow::Owned(py.detach(|| std::fs::read(path))?)
    } else {
        obj.extract().or_else(|_| {
            obj.call_method0("read")?
                .extract::<Vec<u8>>()
                .map(Cow::Owned)
        })?
    };

    let encoded_string = py
        .detach(|| encode(&data, encoding, encoder_errors))
        .map_err(YAMLDecodeError::new_err)?;

    load_yaml_from_string(py, encoded_string.as_ref(), parse_datetime)
}

#[pyfunction(name = "_loads")]
fn load_yaml_from_string(py: Python, s: &str, parse_datetime: bool) -> PyResult<Py<PyAny>> {
    let yaml = py
        .detach(|| {
            let mut loader = saphyr::YamlLoader::default();
            loader.early_parse(false);
            let mut parser = saphyr_parser::Parser::new_from_str(s);
            parser.load(&mut loader, true)?;
            Ok::<_, saphyr_parser::ScanError>(loader.into_documents())
        })
        .map_err(|err| YAMLDecodeError::new_err(format_error(s, &err)))?;
    Ok(yaml_to_python(py, &yaml, parse_datetime)?.unbind())
}

#[pyfunction(name = "_dumps")]
fn dumps_yaml(obj: &Bound<'_, PyAny>, compact: bool, multiline_strings: bool) -> PyResult<String> {
    let mut yaml = String::new();
    let mut emitter = saphyr::YamlEmitter::new(&mut yaml);

    emitter.compact(compact);
    emitter.multiline_strings(multiline_strings);
    emitter
        .dump(&(&python_to_yaml(obj)?).into())
        .map_err(|err| YAMLDecodeError::new_err(err.to_string()))?;
    Ok(yaml)
}

#[pymodule(name = "_yaml_rs")]
fn yaml_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_function(wrap_pyfunction!(load_yaml_from_string, m)?)?;
    m.add_function(wrap_pyfunction!(dumps_yaml, m)?)?;
    m.add("_version", env!("CARGO_PKG_VERSION"))?;
    m.add("YAMLDecodeError", m.py().get_type::<YAMLDecodeError>())?;
    m.add("YAMLEncodeError", m.py().get_type::<YAMLEncodeError>())?;
    Ok(())
}
