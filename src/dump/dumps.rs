use jiff::{
    Zoned,
    civil::{Date, DateTime, Time},
    fmt::temporal::DateTimePrinter,
    tz::Offset,
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
        normalize::{normalize_decimal, normalize_non_utc_fraction},
    },
};

#[allow(non_upper_case_globals)]
const printer: DateTimePrinter = DateTimePrinter::new();

fn needs_yaml_line_separator_escape(value: &str) -> bool {
    value.contains(['\u{85}', '\u{2028}', '\u{2029}'])
}

fn escape_yaml_double_quoted(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\0' => escaped.push_str("\\u0000"),
            '\u{01}' => escaped.push_str("\\u0001"),
            '\u{02}' => escaped.push_str("\\u0002"),
            '\u{03}' => escaped.push_str("\\u0003"),
            '\u{04}' => escaped.push_str("\\u0004"),
            '\u{05}' => escaped.push_str("\\u0005"),
            '\u{06}' => escaped.push_str("\\u0006"),
            '\u{07}' => escaped.push_str("\\u0007"),
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0b}' => escaped.push_str("\\u000b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            '\u{0e}' => escaped.push_str("\\u000e"),
            '\u{0f}' => escaped.push_str("\\u000f"),
            '\u{10}' => escaped.push_str("\\u0010"),
            '\u{11}' => escaped.push_str("\\u0011"),
            '\u{12}' => escaped.push_str("\\u0012"),
            '\u{13}' => escaped.push_str("\\u0013"),
            '\u{14}' => escaped.push_str("\\u0014"),
            '\u{15}' => escaped.push_str("\\u0015"),
            '\u{16}' => escaped.push_str("\\u0016"),
            '\u{17}' => escaped.push_str("\\u0017"),
            '\u{18}' => escaped.push_str("\\u0018"),
            '\u{19}' => escaped.push_str("\\u0019"),
            '\u{1a}' => escaped.push_str("\\u001a"),
            '\u{1b}' => escaped.push_str("\\u001b"),
            '\u{1c}' => escaped.push_str("\\u001c"),
            '\u{1d}' => escaped.push_str("\\u001d"),
            '\u{1e}' => escaped.push_str("\\u001e"),
            '\u{1f}' => escaped.push_str("\\u001f"),
            '\u{7f}' => escaped.push_str("\\u007f"),
            '\u{85}' => escaped.push_str("\\N"),
            '\u{2028}' => escaped.push_str("\\L"),
            '\u{2029}' => escaped.push_str("\\P"),
            _ => escaped.push(character),
        }
    }

    escaped
}

pub fn python_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<YamlOwned> {
    match obj {
        obj if let Ok(str) = obj.cast::<PyString>() => {
            let value = str.to_string_lossy();
            if needs_yaml_line_separator_escape(&value) {
                Ok(YamlOwned::Representation(
                    escape_yaml_double_quoted(&value),
                    ScalarStyle::DoubleQuoted,
                    None,
                ))
            } else {
                Ok(Value(ScalarOwned::String(value.into_owned())))
            }
        }
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
                if zoned.offset() == Offset::UTC {
                    printer.timestamp_to_string(&zoned.timestamp())
                } else {
                    normalize_non_utc_fraction(
                        printer.timestamp_with_offset_to_string(&zoned.timestamp(), zoned.offset()),
                    )
                }
            } else {
                printer.datetime_to_string(&obj.extract::<DateTime>()?)
            };
            Ok(Value(ScalarOwned::String(datetime_str)))
        }
        obj if let Ok(time) = obj.cast::<PyTime>() => Ok(Value(ScalarOwned::String(
            printer.time_to_string(&time.extract::<Time>()?),
        ))),
        obj if let Ok(date) = obj.cast::<PyDate>() => Ok(Value(ScalarOwned::String(
            printer.date_to_string(&date.extract::<Date>()?),
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
