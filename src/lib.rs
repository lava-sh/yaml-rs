mod decoder;
mod dumps;
mod format_error;
mod loads;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use pyo3::exceptions;

pyo3::create_exception!(yaml_rs, YAMLDecodeError, exceptions::PyValueError);
pyo3::create_exception!(yaml_rs, YAMLEncodeError, exceptions::PyTypeError);

#[pyo3::pymodule(name = "_yaml_rs")]
mod yaml_rs {
    use std::borrow::Cow;

    use pyo3::{prelude::*, types::PyString};

    #[pymodule_export]
    use super::{YAMLDecodeError, YAMLEncodeError};
    use crate::{decoder, dumps, format_error::format_error, loads};

    #[pymodule_export]
    const _VERSION: &str = env!("CARGO_PKG_VERSION");

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
            .detach(|| decoder::encode(&data, encoding, encoder_errors))
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
        Ok(loads::yaml_to_python(py, &yaml, parse_datetime)?.unbind())
    }

    #[pyfunction(name = "_dumps")]
    fn dumps_yaml(
        obj: &Bound<'_, PyAny>,
        compact: bool,
        multiline_strings: bool,
    ) -> PyResult<String> {
        let mut yaml = String::new();
        let mut emitter = saphyr::YamlEmitter::new(&mut yaml);

        emitter.compact(compact);
        emitter.multiline_strings(multiline_strings);
        emitter
            .dump(&(&dumps::python_to_yaml(obj)?).into())
            .map_err(|err| YAMLDecodeError::new_err(err.to_string()))?;
        Ok(yaml)
    }
}
