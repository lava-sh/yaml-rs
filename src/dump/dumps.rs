use jiff::{
    Zoned,
    civil::{Date, DateTime, Time},
};
use pyo3::{
    Bound, PyAny, PyResult,
    types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDate, PyDateTime, PyDict, PyDictMethods, PyFloat,
        PyFrozenSet, PyFrozenSetMethods, PyInt, PyList, PyListMethods, PySet, PySetMethods,
        PyString, PyStringMethods, PyTime, PyTuple, PyTupleMethods, PyTzInfoAccess,
    },
};
use saphyr::{MappingOwned, ScalarOwned, ScalarStyle, YamlOwned, YamlOwned::Value};

use crate::{
    YAMLEncodeError,
    dump::{
        helpers::{get_decimal, sequence_to_yaml, set_to_yaml, to_yaml_float},
        normalize::normalize_decimal,
    },
};

pub(crate) fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<YamlOwned> {
    match obj {
        obj if let Ok(str) = obj.cast::<PyString>() => Ok(Value(ScalarOwned::String(
            str.to_string_lossy().into_owned(),
        ))),
        obj if let Ok(bool) = obj.cast::<PyBool>() => {
            Ok(Value(ScalarOwned::Boolean(bool.is_true())))
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
        obj if obj.is_none() => Ok(Value(ScalarOwned::Null)),
        obj if let Ok(datetime) = obj.cast::<PyDateTime>() => {
            let datetime_str = if datetime.get_tzinfo().is_some() {
                let zoned: Zoned = obj.extract()?;
                let mut datetime_str = zoned
                    .timestamp()
                    .display_with_offset(zoned.offset())
                    .to_string();
                if datetime_str.ends_with("+00:00") {
                    datetime_str.truncate(datetime_str.len() - 6);
                    datetime_str.push('Z');
                }
                datetime_str
            } else {
                obj.extract::<DateTime>()?.to_string()
            };
            Ok(Value(ScalarOwned::String(datetime_str)))
        }
        obj if let Ok(time) = obj.cast::<PyTime>() => Ok(Value(ScalarOwned::String(
            time.extract::<Time>()?.to_string(),
        ))),
        obj if let Ok(date) = obj.cast::<PyDate>() => Ok(Value(ScalarOwned::String(
            date.extract::<Date>()?.to_string(),
        ))),
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
        obj if {
            let py = obj.py();
            let decimal = get_decimal(py)?;
            obj.is_instance(decimal.as_any())?
        } =>
        {
            let py_str = obj.str()?;
            Ok(YamlOwned::Representation(
                normalize_decimal(py_str.to_str()?)?.into_owned(),
                ScalarStyle::Plain,
                None,
            ))
        }

        _ => Err(YAMLEncodeError::new_err(format!(
            "Cannot serialize {obj_type} ({obj_repr}) to YAML",
            obj_type = obj.get_type(),
            obj_repr = obj
                .repr()
                .map_or_else(|_| "<repr failed>".to_string(), |repr| repr.to_string())
        ))),
    }
}
