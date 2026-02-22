use std::fmt::Write;

use pyo3::{
    Bound, PyAny, PyResult, intern,
    types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDate, PyDateAccess, PyDateTime, PyDelta,
        PyDeltaAccess, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyFrozenSetMethods, PyInt,
        PyList, PyListMethods, PySet, PySetMethods, PyString, PyStringMethods, PyTimeAccess,
        PyTuple, PyTupleMethods, PyTzInfo, PyTzInfoAccess,
    },
};
use saphyr::{MappingOwned, ScalarOwned, ScalarStyle, YamlOwned, YamlOwned::Value};

use crate::YAMLEncodeError;

pub(crate) fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<YamlOwned> {
    if let Ok(str) = obj.cast::<PyString>() {
        return Ok(Value(ScalarOwned::String(
            str.to_string_lossy().into_owned(),
        )));
    }
    if obj.is_none() {
        return Ok(Value(ScalarOwned::Null));
    }
    if let Ok(bool) = obj.cast::<PyBool>() {
        return Ok(Value(ScalarOwned::Boolean(bool.is_true())));
    }
    if let Ok(int) = obj.cast::<PyInt>() {
        return match int.extract::<i64>() {
            Ok(value) => Ok(Value(ScalarOwned::Integer(value))),
            Err(_) => Ok(YamlOwned::Representation(
                int.str()?.to_str()?.to_owned(),
                ScalarStyle::Plain,
                None,
            )),
        };
    }
    if let Ok(float) = obj.cast::<PyFloat>() {
        return Ok(YamlOwned::Representation(
            to_yaml_float(float)?,
            ScalarStyle::Plain,
            None,
        ));
    }
    if let Ok(datetime) = obj.cast::<PyDateTime>() {
        let year = datetime.get_year();
        let month = datetime.get_month();
        let day = datetime.get_day();
        let hour = datetime.get_hour();
        let minute = datetime.get_minute();
        let second = datetime.get_second();
        let microsecond = datetime.get_microsecond();

        let tzinfo = datetime.get_tzinfo();

        let capacity = if tzinfo.is_some() { 35 } else { 26 };
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
            let mut buffer = itoa::Buffer::new();
            let formatted = buffer.format(microsecond);

            let padding = 6 - formatted.len();
            let mut padded = String::with_capacity(6);
            for _ in 0..padding {
                padded.push('0');
            }
            padded.push_str(formatted);

            let min_len = if is_utc { 1 } else { 2 };
            while padded.ends_with('0') && padded.len() > min_len {
                padded.pop();
            }

            datetime_str.push('.');
            datetime_str.push_str(&padded);
        }

        if let Some(tz) = tzinfo {
            if is_utc {
                datetime_str.push('Z');
            } else {
                let result = tz
                    .call_method1(intern!(py, "utcoffset"), (py.None(),))
                    .ok()
                    .filter(|d| !d.is_none())
                    .and_then(|offset_delta| {
                        let delta = offset_delta.cast::<PyDelta>().ok()?;
                        let days = delta.get_days();
                        let seconds = delta.get_seconds();
                        let total_seconds = days * 86400 + seconds;
                        let total_minutes = total_seconds / 60;
                        let offset_hours = total_minutes / 60;
                        let offset_minutes = (total_minutes % 60).abs();
                        Some((offset_hours, offset_minutes))
                    });

                if let Some((offset_hours, offset_minutes)) = result {
                    write!(&mut datetime_str, "{offset_hours:+03}:{offset_minutes:02}",).unwrap();
                }
            }
        }

        return Ok(Value(ScalarOwned::String(datetime_str)));
    }
    if let Ok(date) = obj.cast::<PyDate>() {
        let year = date.get_year();
        let month = date.get_month();
        let day = date.get_day();
        let mut date = String::with_capacity(10);
        write!(&mut date, "{year:04}-{month:02}-{day:02}").unwrap();
        return Ok(Value(ScalarOwned::String(date)));
    }
    if let Ok(tuple) = obj.cast::<PyTuple>() {
        let len = tuple.len();
        if len == 0 {
            return Ok(YamlOwned::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in tuple.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        return Ok(YamlOwned::Sequence(sequence));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        let len = list.len();
        if len == 0 {
            return Ok(YamlOwned::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in list.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        return Ok(YamlOwned::Sequence(sequence));
    }
    if let Ok(set) = obj.cast::<PySet>() {
        let mut mapping = MappingOwned::with_capacity(set.len());
        for item in set.iter() {
            mapping.insert(python_to_yaml(&item)?, Value(ScalarOwned::Null));
        }
        return Ok(YamlOwned::Mapping(mapping));
    }
    if let Ok(frozenset) = obj.cast::<PyFrozenSet>() {
        let mut mapping = MappingOwned::with_capacity(frozenset.len());
        for item in frozenset.iter() {
            mapping.insert(python_to_yaml(&item)?, Value(ScalarOwned::Null));
        }
        return Ok(YamlOwned::Mapping(mapping));
    }
    if let Ok(dict) = obj.cast::<PyDict>() {
        let len = dict.len();
        if len == 0 {
            return Ok(YamlOwned::Mapping(MappingOwned::new()));
        }
        let mut mapping = MappingOwned::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            mapping.insert(python_to_yaml(&k)?, python_to_yaml(&v)?);
        }
        return Ok(YamlOwned::Mapping(mapping));
    }
    Err(YAMLEncodeError::new_err(format!(
        "Cannot serialize {obj_type} ({obj_repr}) to YAML",
        obj_type = obj.get_type(),
        obj_repr = obj
            .repr()
            .map_or_else(|_| "<repr failed>".to_string(), |r| r.to_string())
    )))
}

fn to_yaml_float(float: &Bound<'_, PyFloat>) -> PyResult<String> {
    let py_str = float.str()?;
    let repr = py_str.to_str()?;
    let value = match repr {
        "inf" => ".inf".to_owned(),
        "-inf" => "-.inf".to_owned(),
        "nan" => ".nan".to_owned(),
        _ => {
            if let Some(index) = repr.find(['e', 'E']) {
                let mantissa = &repr[..index];
                let exponent = &repr[index + 1..];
                if mantissa.contains('.') {
                    format!("{mantissa}e{exponent}")
                } else {
                    format!("{mantissa}.0e{exponent}")
                }
            } else {
                repr.to_owned()
            }
        }
    };
    Ok(value)
}
