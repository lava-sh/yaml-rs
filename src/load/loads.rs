use std::borrow::Cow;

use memchr::{memchr, memchr2};
use num_bigint::BigInt;
use pyo3::{
    IntoPyObjectExt,
    prelude::*,
    types::{PyDict, PyFrozenSet, PyList, PySet, PyTuple},
};
use rustc_hash::FxHashMap;
use saphyr_parser::{Event, Parser, ScalarStyle, ScanError, Tag};

use crate::load::{
    arena::{Arena, NodeId},
    parse_datetime::parse_py_datetime,
    value::Value,
};

const UNDERSCORE: u8 = b'_';

#[derive(Debug)]
enum Frame {
    Seq {
        anchor: usize,
        items: Vec<NodeId>,
    },
    Map {
        anchor: usize,
        items: Vec<(NodeId, NodeId)>,
        pending_key: Option<NodeId>,
    },
}

#[derive(Debug)]
pub enum BuildError {
    Scan(ScanError),
    Decode(String),
}

impl From<ScanError> for BuildError {
    fn from(err: ScanError) -> Self {
        BuildError::Scan(err)
    }
}

#[inline]
fn is_null(str: &str) -> bool {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: null | Null | NULL | ~
    matches!(str, "null" | "Null" | "NULL" | "~")
}

#[inline]
fn is_bool(str: &str) -> Option<bool> {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: true | True | TRUE | false | False | FALSE
    match str {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

#[inline]
fn normalize_num(str: &str) -> Cow<'_, str> {
    let bytes = str.as_bytes();

    if memchr(UNDERSCORE, bytes).is_none() {
        return Cow::Borrowed(str);
    }

    let mut num = String::with_capacity(str.len());

    for &byte in bytes {
        if byte != UNDERSCORE {
            num.push(byte as char);
        }
    }

    Cow::Owned(num)
}

fn parse_int<'a>(str: &str) -> Option<Value<'a>> {
    if str.is_empty() {
        return None;
    }

    let (sign, rest) = match str.as_bytes()[0] {
        b'+' => (1i64, &str[1..]),
        b'-' => (-1i64, &str[1..]),
        _ => (1i64, str),
    };

    let norm = normalize_num(rest);
    let r = norm.as_ref();

    let (radix, digits) = match r.as_bytes() {
        [b'0', b'x' | b'X', ..] => (16u32, &r[2..]),
        [b'0', b'o' | b'O', ..] => (8u32, &r[2..]),
        [b'0', b'b' | b'B', ..] => (2u32, &r[2..]),
        _ => (10u32, r),
    };

    if digits.is_empty() {
        return None;
    }

    if radix == 10 {
        if let Ok(i_64) = lexical_core::parse::<i64>(digits.as_bytes()) {
            return Some(Value::IntegerI64(i_64.wrapping_mul(sign)));
        }
    } else if let Ok(i_64) = i64::from_str_radix(digits, radix) {
        return Some(Value::IntegerI64(i_64.wrapping_mul(sign)));
    }

    BigInt::parse_bytes(digits.as_bytes(), radix).map(|big_int| {
        if sign < 0 {
            Value::IntegerBig(-big_int)
        } else {
            Value::IntegerBig(big_int)
        }
    })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SpecialFloat {
    Inf,
    Nan,
}

#[inline]
fn is_inf_nan(bytes: &[u8]) -> Option<(SpecialFloat, bool)> {
    if bytes.is_empty() {
        return None;
    }

    let mut i = 0usize;
    let mut neg = false;

    match bytes[i] {
        b'+' => {
            i += 1;
            if i >= bytes.len() {
                return None;
            }
        }
        b'-' => {
            neg = true;
            i += 1;
            if i >= bytes.len() {
                return None;
            }
        }
        _ => {}
    }

    if bytes[i] == b'.' {
        i += 1;
        if i >= bytes.len() {
            return None;
        }
    }

    let rest = &bytes[i..];

    if rest.len() != 3 {
        return None;
    }

    let (a, b, c) = (rest[0], rest[1], rest[2]);

    if matches!((a, b, c), (b'i' | b'I', b'n' | b'N', b'f' | b'F')) {
        return Some((SpecialFloat::Inf, neg));
    }

    if matches!((a, b, c), (b'n' | b'N', b'a' | b'A', b'n' | b'N')) {
        return Some((SpecialFloat::Nan, neg));
    }

    None
}

fn parse_float(str: &str) -> Option<f64> {
    if str.is_empty() {
        return None;
    }

    if let Some((kind, neg)) = is_inf_nan(str.as_bytes()) {
        return Some(match kind {
            SpecialFloat::Nan => f64::NAN,
            SpecialFloat::Inf => {
                if neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }
            }
        });
    }

    let norm = normalize_num(str);

    lexical_core::parse::<f64>(norm.as_bytes()).ok()
}

fn resolve_scalar<'a>(
    arena: &mut Arena<'a>,
    value: Cow<'a, str>,
    style: ScalarStyle,
    tag: Option<&Tag>,
) -> Result<NodeId, String> {
    if let Some(tag) = tag {
        if tag.is_yaml_core_schema() {
            let v = match tag.suffix.as_str() {
                "int" => parse_int(value.as_ref())
                    .ok_or_else(|| format!("Invalid value '{}' for '!!int' tag", value.as_ref()))?,
                "float" => parse_float(value.as_ref())
                    .map(Value::Float)
                    .ok_or_else(|| {
                        format!("Invalid value '{}' for '!!float' tag", value.as_ref())
                    })?,
                "bool" => is_bool(value.as_ref()).map(Value::Boolean).ok_or_else(|| {
                    format!("Invalid value '{}' for '!!bool' tag", value.as_ref())
                })?,
                "null" => {
                    let str = value.as_ref();
                    if str.is_empty() || is_null(str) {
                        Value::Null
                    } else {
                        return Err(format!("Invalid value '{str}' for '!!null' tag"));
                    }
                }
                "binary" => Value::String(value),
                "str" => Value::StringExplicit(value),
                _ => return Err(format!("Invalid tag: '!!{}'", tag.suffix)),
            };
            return Ok(arena.push(v));
        }

        return Ok(arena.push(Value::String(value)));
    }

    if style == ScalarStyle::Plain {
        let str = value.as_ref();

        if str.is_empty() || is_null(str) {
            return Ok(arena.push(Value::Null));
        }

        if let Some(bool) = is_bool(str) {
            return Ok(arena.push(Value::Boolean(bool)));
        }

        let bytes = str.as_bytes();

        if (is_inf_nan(bytes).is_some()
            || memchr(b'.', bytes).is_some()
            || memchr2(b'e', b'E', bytes).is_some())
            && let Some(float) = parse_float(str)
        {
            return Ok(arena.push(Value::Float(float)));
        }

        if let Some(int) = parse_int(str) {
            return Ok(arena.push(int));
        }
    }

    Ok(arena.push(Value::String(value)))
}

pub(crate) fn build_from_events(input: &'_ str) -> Result<(Arena<'_>, Vec<NodeId>), BuildError> {
    let parser = Parser::new_from_str(input);

    let mut arena = Arena::with_capacity((input.len() / 8).max(64));

    let mut stack: Vec<Frame> = Vec::new();
    let mut docs: Vec<NodeId> = Vec::new();
    let mut anchors: FxHashMap<usize, NodeId> = FxHashMap::default();
    let mut current_root: Option<NodeId> = None;

    for event_res in parser {
        let (event, _) = event_res?;

        match event {
            Event::StreamStart | Event::StreamEnd | Event::Nothing => {}
            Event::DocumentStart(_) => {
                current_root = None;
                stack.clear();
            }
            Event::DocumentEnd => {
                let root = current_root
                    .take()
                    .unwrap_or_else(|| arena.push(Value::Null));
                docs.push(root);
            }
            Event::Alias(id) => {
                let node = anchors
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| arena.push(Value::Null));

                push_value(node, &mut stack, &mut current_root);
            }
            Event::Scalar(val, style, anchor_id, tag) => {
                let node = resolve_scalar(&mut arena, val, style, tag.as_deref())
                    .map_err(BuildError::Decode)?;

                if anchor_id != 0 {
                    anchors.insert(anchor_id, node);
                }

                push_value(node, &mut stack, &mut current_root);
            }
            Event::SequenceStart(anchor_id, _) => {
                stack.push(Frame::Seq {
                    anchor: anchor_id,
                    items: Vec::new(),
                });
            }
            Event::SequenceEnd => {
                if let Some(Frame::Seq { anchor, items }) = stack.pop() {
                    let node = arena.push(Value::Seq(items));

                    if anchor != 0 {
                        anchors.insert(anchor, node);
                    }

                    push_value(node, &mut stack, &mut current_root);
                }
            }
            Event::MappingStart(anchor_id, _) => {
                stack.push(Frame::Map {
                    anchor: anchor_id,
                    items: Vec::new(),
                    pending_key: None,
                });
            }
            Event::MappingEnd => {
                if let Some(Frame::Map { anchor, items, .. }) = stack.pop() {
                    let node = arena.push(Value::Map(items));

                    if anchor != 0 {
                        anchors.insert(anchor, node);
                    }

                    push_value(node, &mut stack, &mut current_root);
                }
            }
        }
    }

    Ok((arena, docs))
}

#[inline]
fn push_value(value: NodeId, stack: &mut [Frame], root: &mut Option<NodeId>) {
    if let Some(top) = stack.last_mut() {
        match top {
            Frame::Seq { items, .. } => items.push(value),
            Frame::Map {
                items, pending_key, ..
            } => {
                if let Some(key) = pending_key.take() {
                    items.push((key, value));
                } else {
                    *pending_key = Some(value);
                }
            }
        }
    } else {
        *root = Some(value);
    }
}

fn value_to_py<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    id: NodeId,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Boolean(bool) => bool.into_bound_py_any(py),
        Value::IntegerI64(int_64) => int_64.into_bound_py_any(py),
        Value::IntegerBig(big_int) => big_int.into_bound_py_any(py),
        Value::Float(float) => float.into_bound_py_any(py),
        Value::StringExplicit(string_exp) => string_exp.into_bound_py_any(py),
        Value::String(string) => {
            let str = string.as_ref();
            if parse_datetime && let Ok(Some(dt)) = parse_py_datetime(py, str) {
                return Ok(dt);
            }
            str.into_bound_py_any(py)
        }
        Value::Seq(items) => {
            let py_list = PyList::empty(py);
            for &child in items {
                py_list.append(value_to_py(py, arena, child, parse_datetime)?)?;
            }
            Ok(py_list.into_any())
        }
        Value::Map(pairs) => {
            let mut all_nulls = true;
            let mut has_null_key = false;

            for (k, v) in pairs {
                if matches!(arena.get(*k), Value::Null) {
                    has_null_key = true;
                }
                if !matches!(arena.get(*v), Value::Null) {
                    all_nulls = false;
                }
            }

            if all_nulls && !has_null_key && pairs.len() > 1 {
                let py_set = PySet::empty(py)?;
                for (k, _) in pairs {
                    py_set.add(value_to_hashable(py, arena, *k, parse_datetime)?)?;
                }
                Ok(py_set.into_any())
            } else {
                let py_dict = PyDict::new(py);
                for (k, v) in pairs {
                    py_dict.set_item(
                        value_to_hashable(py, arena, *k, parse_datetime)?,
                        value_to_py(py, arena, *v, parse_datetime)?,
                    )?;
                }
                Ok(py_dict.into_any())
            }
        }
    }
}

fn value_to_hashable<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    id: NodeId,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Seq(items) => {
            let mut vec = Vec::with_capacity(items.len());
            for &child in items {
                vec.push(value_to_hashable(py, arena, child, parse_datetime)?);
            }
            PyTuple::new(py, &vec)?.into_bound_py_any(py)
        }
        Value::Map(pairs) => {
            let py_list = PyList::empty(py);
            for (k, v) in pairs {
                let py_tuple = PyTuple::new(
                    py,
                    &[
                        value_to_hashable(py, arena, *k, parse_datetime)?,
                        value_to_py(py, arena, *v, parse_datetime)?,
                    ],
                )?;
                py_list.append(py_tuple)?;
            }
            PyFrozenSet::new(py, py_list)?.into_bound_py_any(py)
        }
        _ => value_to_py(py, arena, id, parse_datetime),
    }
}

pub(crate) fn to_python<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    docs: &[NodeId],
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => value_to_py(py, arena, docs[0], parse_datetime),
        _ => {
            let py_list = PyList::empty(py);
            for &doc in docs {
                py_list.append(value_to_py(py, arena, doc, parse_datetime)?)?;
            }
            Ok(py_list.into_any())
        }
    }
}
