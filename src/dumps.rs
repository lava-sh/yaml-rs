use std::fmt::Write;

use ordered_float::OrderedFloat;
use pyo3::{
    Bound, PyAny, PyResult, intern,
    types::{
        PyAnyMethods, PyBool, PyBoolMethods, PyDate, PyDateAccess, PyDateTime, PyDelta,
        PyDeltaAccess, PyDict, PyDictMethods, PyFloat, PyFloatMethods, PyFrozenSet,
        PyFrozenSetMethods, PyInt, PyList, PyListMethods, PySet, PySetMethods, PyString,
        PyStringMethods, PyTimeAccess, PyTuple, PyTupleMethods, PyTzInfo, PyTzInfoAccess,
    },
};
use saphyr::{
    MappingOwned, ScalarOwned,
    ScalarOwned::{Boolean, FloatingPoint, Integer, Null},
    YamlOwned,
    YamlOwned::Value,
};

pub(crate) fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<YamlOwned> {
    if let Ok(str) = obj.cast::<PyString>() {
        return Ok(Value(ScalarOwned::String(
            str.to_string_lossy().into_owned(),
        )));
    }

    if obj.is_none() {
        Ok(Value(Null))
    } else if let Ok(bool) = obj.cast::<PyBool>() {
        Ok(Value(Boolean(bool.is_true())))
    } else if let Ok(int) = obj.cast::<PyInt>() {
        Ok(Value(Integer(int.extract()?)))
    } else if let Ok(float) = obj.cast::<PyFloat>() {
        Ok(Value(FloatingPoint(OrderedFloat(float.value()))))
    } else if let Ok(datetime) = obj.cast::<PyDateTime>() {
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
        let is_utc = if let Some(ref tz) = tzinfo {
            PyTzInfo::utc(py)
                .ok()
                .and_then(|utc| tz.eq(utc).ok())
                .unwrap_or(false)
        } else {
            false
        };

        write!(
            &mut datetime_str,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year, month, day, hour, minute, second
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
                    write!(
                        &mut datetime_str,
                        "{:+03}:{:02}",
                        offset_hours, offset_minutes
                    )
                    .unwrap();
                }
            }
        }

        Ok(Value(ScalarOwned::String(datetime_str)))
    } else if let Ok(date) = obj.cast::<PyDate>() {
        let year = date.get_year();
        let month = date.get_month();
        let day = date.get_day();
        let mut date = String::with_capacity(10);
        write!(&mut date, "{:04}-{:02}-{:02}", year, month, day).unwrap();
        Ok(Value(ScalarOwned::String(date)))
    } else if let Ok(tuple) = obj.cast::<PyTuple>() {
        let len = tuple.len();
        if len == 0 {
            return Ok(YamlOwned::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in tuple.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        Ok(YamlOwned::Sequence(sequence))
    } else if let Ok(list) = obj.cast::<PyList>() {
        let len = list.len();
        if len == 0 {
            return Ok(YamlOwned::Sequence(Vec::new()));
        }
        let mut sequence = Vec::with_capacity(len);
        for item in list.iter() {
            sequence.push(python_to_yaml(&item)?);
        }
        Ok(YamlOwned::Sequence(sequence))
    } else if let Ok(set) = obj.cast::<PySet>() {
        let mut mapping = MappingOwned::with_capacity(set.len());
        for item in set.iter() {
            mapping.insert(python_to_yaml(&item)?, Value(Null));
        }
        Ok(YamlOwned::Mapping(mapping))
    } else if let Ok(frozenset) = obj.cast::<PyFrozenSet>() {
        let mut mapping = MappingOwned::with_capacity(frozenset.len());
        for item in frozenset.iter() {
            mapping.insert(python_to_yaml(&item)?, Value(Null));
        }
        Ok(YamlOwned::Mapping(mapping))
    } else if let Ok(dict) = obj.cast::<PyDict>() {
        let len = dict.len();
        if len == 0 {
            return Ok(YamlOwned::Mapping(MappingOwned::new()));
        }
        let mut mapping = MappingOwned::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            mapping.insert(python_to_yaml(&k)?, python_to_yaml(&v)?);
        }
        Ok(YamlOwned::Mapping(mapping))
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
