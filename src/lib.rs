mod loads;

use crate::loads::{format_error, yaml_to_python};

use pyo3::{create_exception, exceptions::PyValueError, prelude::*};

#[cfg(feature = "default")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

create_exception!(yaml_rs, YAMLDecodeError, PyValueError);

#[pyfunction]
fn _loads(py: Python, s: &str, parse_datetime: bool) -> PyResult<Py<PyAny>> {
    let yaml = py
        .detach(|| {
            let mut loader = saphyr::YamlLoader::default();
            loader.early_parse(false);
            let mut parser = saphyr_parser::Parser::new_from_str(s);
            parser.load(&mut loader, true)?;
            Ok::<_, saphyr::ScanError>(loader.into_documents())
        })
        .map_err(|err| YAMLDecodeError::new_err(format_error(s, &err)))?;
    Ok(yaml_to_python(py, yaml, parse_datetime)?.unbind())
}

#[pymodule(name = "_yaml_rs")]
fn toml_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(_loads, m)?)?;
    m.add("_version", env!("CARGO_PKG_VERSION"))?;
    m.add("YAMLDecodeError", m.py().get_type::<YAMLDecodeError>())?;
    Ok(())
}
