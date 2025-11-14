use std::borrow::Cow;

use pyo3::{
    Bound, PyAny, PyResult,
    prelude::{
        PyAnyMethods, PyDictMethods, PyFrozenSetMethods, PyListMethods, PySetMethods,
        PyTupleMethods,
    },
    types::{PyDict, PyFrozenSet, PyList, PySet, PyTuple},
};
use saphyr::{Mapping, Scalar, Yaml};

pub(crate) fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<Yaml<'static>> {
    if let Ok(str) = obj.extract::<String>() {
        return Ok(Yaml::Value(Scalar::String(Cow::Owned(str))));
    }

    if obj.is_none() {
        Ok(Yaml::Value(Scalar::Null))
    } else if let Ok(bool) = obj.extract::<bool>() {
        Ok(Yaml::Value(Scalar::Boolean(bool)))
    } else if let Ok(int) = obj.extract::<i64>() {
        Ok(Yaml::Value(Scalar::Integer(int)))
    } else if let Ok(float) = obj.extract::<f64>() {
        Ok(Yaml::Value(Scalar::FloatingPoint(float.into())))
    } else if let Ok(tuple) = obj.cast::<PyTuple>() {
        let len = tuple.len();
        if len == 0 {
            return Ok(Yaml::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in tuple.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        Ok(Yaml::Sequence(sequence))
    } else if let Ok(list) = obj.cast::<PyList>() {
        let len = list.len();
        if len == 0 {
            return Ok(Yaml::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in list.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        Ok(Yaml::Sequence(sequence))
    } else if let Ok(set) = obj.cast::<PySet>() {
        let mut mapping = Mapping::with_capacity(set.len());
        for item in set.iter() {
            mapping.insert(python_to_yaml(&item)?, Yaml::Value(Scalar::Null));
        }
        Ok(Yaml::Mapping(mapping))
    } else if let Ok(frozenset) = obj.cast::<PyFrozenSet>() {
        let mut mapping = Mapping::with_capacity(frozenset.len());
        for item in frozenset.iter() {
            mapping.insert(python_to_yaml(&item)?, Yaml::Value(Scalar::Null));
        }
        Ok(Yaml::Mapping(mapping))
    } else if let Ok(dict) = obj.cast::<PyDict>() {
        let len = dict.len();
        if len == 0 {
            return Ok(Yaml::Mapping(Mapping::new()));
        }
        let mut mapping = Mapping::with_capacity(len);
        for (k, v) in dict.iter() {
            mapping.insert(python_to_yaml(&k)?, python_to_yaml(&v)?);
        }
        Ok(Yaml::Mapping(mapping))
    } else {
        Err(crate::YAMLEncodeError::new_err(format!(
            "Cannot serialize {obj_type} ({obj_repr}) to YAML",
            obj_type = obj.get_type(),
            obj_repr = obj
                .repr()
                .map(|r| r.to_string())
                .unwrap_or_else(|_| "<repr failed>".to_string())
        )))
    }
}
