#![feature(pointer_is_aligned_to)]

mod dump;
mod from_rust;
mod load;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use pyo3::{create_exception, exceptions};

create_exception!(yaml_rs, YAMLDecodeError, exceptions::PyValueError);
create_exception!(yaml_rs, YAMLEncodeError, exceptions::PyTypeError);

#[pyo3::pymodule(name = "_yaml_rs")]
mod yaml_rs {
    use std::borrow::Cow;

    use pyo3::{prelude::*, types::PyString};

    #[pymodule_export]
    use super::{YAMLDecodeError, YAMLEncodeError};
    #[pymodule_export]
    use crate::load::options::AliasLimits;
    use crate::{
        dump::dumps,
        load::{
            decoder,
            format_error::format_error,
            loads::{BuildError, build_from_events, to_python},
        },
    };

    #[pymodule_export]
    const _VERSION: &str = env!("CARGO_PKG_VERSION");

    #[pyfunction(name = "_load")]
    fn load<'py>(
        py: Python<'py>,
        obj: &Bound<'_, PyAny>,
        parse_datetime: bool,
        encoding: Option<&str>,
        encoder_errors: Option<&str>,
        alias_limits: Option<PyRef<'_, AliasLimits>>,
    ) -> PyResult<Bound<'py, PyAny>> {
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

        load_yaml_from_string(py, encoded_string.as_ref(), parse_datetime, alias_limits)
    }

    #[pyfunction(name = "_loads")]
    #[allow(clippy::needless_pass_by_value)]
    fn load_yaml_from_string<'py>(
        py: Python<'py>,
        yaml_string: &str,
        parse_datetime: bool,
        alias_limits: Option<PyRef<'_, AliasLimits>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let alias_limits = alias_limits
            .as_ref()
            .map(|limits| **limits)
            .unwrap_or_default();

        let (arena, docs) = py
            .detach(|| build_from_events(yaml_string))
            .map_err(|error| match error {
                BuildError::Scan(err) => YAMLDecodeError::new_err(format_error(yaml_string, &err)),
                BuildError::Decode(msg) => YAMLDecodeError::new_err(msg),
            })?;

        to_python(py, &arena, &docs, parse_datetime, alias_limits)
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
