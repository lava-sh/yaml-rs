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

fn is_null(str: &str) -> bool {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: null | Null | NULL | ~
    matches!(str, "null" | "Null" | "NULL" | "~")
}

fn is_bool(str: &str) -> Option<bool> {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: true | True | TRUE | false | False | FALSE
    match str {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

fn normalize_num(str: &str) -> Cow<'_, str> {
    let bytes = str.as_bytes();

    if memchr(UNDERSCORE, bytes).is_none() {
        return Cow::Borrowed(str);
    }

    let mut out = String::with_capacity(str.len());
    for &b in bytes {
        if b != UNDERSCORE {
            out.push(b as char);
        }
    }
    Cow::Owned(out)
}

fn parse_int(str: &str) -> Option<Value> {
    let s = str.trim();
    if s.is_empty() {
        return None;
    }

    let (sign, rest) = match s.as_bytes()[0] {
        b'+' => (1i64, &s[1..]),
        b'-' => (-1i64, &s[1..]),
        _ => (1i64, s),
    };

    let norm = normalize_num(rest);
    let r = norm.as_ref();

    let (radix, digits) =
        if let Some(stripped) = r.strip_prefix("0x").or_else(|| r.strip_prefix("0X")) {
            (16u32, stripped)
        } else if let Some(stripped) = r.strip_prefix("0o").or_else(|| r.strip_prefix("0O")) {
            (8u32, stripped)
        } else if let Some(stripped) = r.strip_prefix("0b").or_else(|| r.strip_prefix("0B")) {
            (2u32, stripped)
        } else {
            (10u32, r)
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

    BigInt::parse_bytes(digits.as_bytes(), radix).map(|b| {
        if sign < 0 {
            Value::IntegerBig(-b)
        } else {
            Value::IntegerBig(b)
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

    let a = rest[0];
    let b = rest[1];
    let c = rest[2];

    if matches!((a, b, c), (b'i' | b'I', b'n' | b'N', b'f' | b'F')) {
        return Some((SpecialFloat::Inf, neg));
    }

    if matches!((a, b, c), (b'n' | b'N', b'a' | b'A', b'n' | b'N')) {
        return Some((SpecialFloat::Nan, neg));
    }

    None
}

fn parse_float(str: &str) -> Option<f64> {
    let trimmed = str.trim();

    if trimmed.is_empty() {
        return None;
    }

    if let Some((kind, neg)) = is_inf_nan(trimmed.as_bytes()) {
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

    let norm = normalize_num(trimmed);

    lexical_core::parse::<f64>(norm.as_bytes()).ok()
}

fn resolve_scalar(
    arena: &mut Arena,
    value: &str,
    style: ScalarStyle,
    tag: Option<&Tag>,
) -> Result<NodeId, String> {
    if let Some(tag) = tag {
        if tag.is_yaml_core_schema() {
            let value = match tag.suffix.as_str() {
                "int" => parse_int(value)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!int' tag"))?,
                "float" => parse_float(value)
                    .map(Value::Float)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!float' tag"))?,
                "bool" => is_bool(value)
                    .map(Value::Boolean)
                    .ok_or_else(|| format!("Invalid value '{value}' for '!!bool' tag"))?,
                "null" => {
                    if value.is_empty() || is_null(value) {
                        Value::Null
                    } else {
                        return Err(format!("Invalid value '{value}' for '!!null' tag"));
                    }
                }
                "binary" => Value::String(value.to_string()),
                "str" => Value::StringExplicit(value.to_string()),
                _ => return Err(format!("Invalid tag: '!!{}'", tag.suffix)),
            };
            return Ok(arena.push(value));
        }

        return Ok(arena.push(Value::String(value.to_string())));
    }

    if style == ScalarStyle::Plain {
        let trimmed = value.trim();

        if trimmed.is_empty() || is_null(trimmed) {
            return Ok(arena.push(Value::Null));
        }

        if let Some(bool) = is_bool(trimmed) {
            return Ok(arena.push(Value::Boolean(bool)));
        }

        let bytes = trimmed.as_bytes();

        if (is_inf_nan(bytes).is_some()
            || memchr(b'.', bytes).is_some()
            || memchr2(b'e', b'E', bytes).is_some())
            && let Some(float) = parse_float(trimmed)
        {
            return Ok(arena.push(Value::Float(float)));
        }

        if let Some(int) = parse_int(trimmed) {
            return Ok(arena.push(int));
        }
    }

    Ok(arena.push(Value::String(value.to_string())))
}

pub(crate) fn build_from_events(input: &str) -> Result<(Arena, Vec<NodeId>), BuildError> {
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
                let tag_owned = tag.map(Cow::into_owned);
                let node = resolve_scalar(&mut arena, &val, style, tag_owned.as_ref())
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
    arena: &Arena,
    id: NodeId,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Boolean(bool) => bool.into_bound_py_any(py),
        Value::IntegerI64(int_64) => int_64.into_bound_py_any(py),
        Value::IntegerBig(big_int) => big_int.into_bound_py_any(py),
        Value::Float(float) => float.into_bound_py_any(py),
        Value::StringExplicit(str_exp) => str_exp.into_bound_py_any(py),
        Value::String(str) => {
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
    arena: &Arena,
    id: NodeId,
    parse_datetime: bool,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Seq(items) => {
            let mut out = Vec::with_capacity(items.len());
            for &child in items {
                out.push(value_to_hashable(py, arena, child, parse_datetime)?);
            }
            PyTuple::new(py, &out)?.into_bound_py_any(py)
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
    arena: &Arena,
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
