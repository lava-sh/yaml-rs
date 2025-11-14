use std::{borrow::Cow, fmt::Write};

use pyo3::types::PyDelta;
use pyo3::{
    Bound, PyAny, PyResult, intern,
    types::{
        PyAnyMethods, PyDate, PyDateAccess, PyDateTime, PyDict, PyDictMethods, PyFrozenSet,
        PyFrozenSetMethods, PyList, PyListMethods, PySet, PySetMethods, PyTimeAccess, PyTuple,
        PyTupleMethods, PyTzInfo, PyTzInfoAccess,
    },
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
                        let days: i64 = delta.getattr(intern!(py, "days")).ok()?.extract().ok()?;
                        let seconds: i64 =
                            delta.getattr(intern!(py, "seconds")).ok()?.extract().ok()?;
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

        Ok(Yaml::Value(Scalar::String(Cow::Owned(datetime_str))))
    } else if let Ok(date) = obj.cast::<PyDate>() {
        let year = date.get_year();
        let month = date.get_month();
        let day = date.get_day();
        let mut date = String::with_capacity(10);
        write!(&mut date, "{:04}-{:02}-{:02}", year, month, day).unwrap();
        Ok(Yaml::Value(Scalar::String(Cow::Owned(date))))
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
