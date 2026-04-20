use std::fmt::Write;

use pyo3::{
    Bound, PyAny, PyResult,
    types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDate, PyDateAccess, PyDateTime, PyDict,
        PyDictMethods, PyFloat, PyFrozenSet, PyFrozenSetMethods, PyInt, PyList, PyListMethods,
        PySet, PySetMethods, PyString, PyStringMethods, PyTime, PyTimeAccess, PyTuple,
        PyTupleMethods, PyTzInfo, PyTzInfoAccess,
    },
};
use saphyr::{MappingOwned, ScalarOwned, ScalarStyle, YamlOwned, YamlOwned::Value};

use crate::{
    YAMLEncodeError,
    dump::{
        helpers::{
            format_ms, get_decimal, get_utc_offset, sequence_to_yaml, set_to_yaml, to_yaml_float,
        },
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
            const DATETIME_BASE_LEN: usize = 19;

            let year = datetime.get_year();
            let month = datetime.get_month();
            let day = datetime.get_day();
            let hour = datetime.get_hour();
            let minute = datetime.get_minute();
            let second = datetime.get_second();
            let microsecond = datetime.get_microsecond();

            let tzinfo = datetime.get_tzinfo();

            let capacity = DATETIME_BASE_LEN
                + usize::from(microsecond > 0) * 7
                + usize::from(tzinfo.is_some()) * 6;

            let mut datetime_str = String::with_capacity(capacity);

            let py = datetime.py();
            let is_utc = match tzinfo {
                Some(ref tz) => PyTzInfo::utc(py)
                    .ok()
                    .and_then(|utc| tz.eq(utc).ok())
                    .unwrap_or(false),
                None => false,
            };

            write!(
                &mut datetime_str,
                "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}",
            )
            .unwrap();

            if microsecond > 0 {
                datetime_str.push('.');
                format_ms(&mut datetime_str, microsecond, if is_utc { 1 } else { 2 })?;
            }

            if let Some(tz) = tzinfo {
                if is_utc {
                    datetime_str.push('Z');
                } else if let Some((offset_hours, offset_minutes)) =
                    get_utc_offset(py, &tz, datetime)
                {
                    write!(&mut datetime_str, "{offset_hours:+03}:{offset_minutes:02}").unwrap();
                }
            }

            Ok(Value(ScalarOwned::String(datetime_str)))
        }
        obj if let Ok(time) = obj.cast::<PyTime>() => {
            let hour = time.get_hour();
            let minute = time.get_minute();
            let second = time.get_second();
            let microsecond = time.get_microsecond();

            let mut time_str = String::with_capacity(16);

            write!(&mut time_str, "{hour:02}:{minute:02}:{second:02}").unwrap();

            if microsecond > 0 {
                time_str.push('.');
                format_ms(&mut time_str, microsecond, 1)?;
            }

            Ok(Value(ScalarOwned::String(time_str)))
        }
        obj if let Ok(date) = obj.cast::<PyDate>() => {
            let year = date.get_year();
            let month = date.get_month();
            let day = date.get_day();
            let mut date_str = String::with_capacity(10);
            write!(&mut date_str, "{year:04}-{month:02}-{day:02}").unwrap();
            Ok(Value(ScalarOwned::String(date_str)))
        }
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
            let (isinstance, decimal) = get_decimal(py)?;
            isinstance.call1((obj, decimal))?.is_truthy()?
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
