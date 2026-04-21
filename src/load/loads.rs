use std::borrow::Cow;

use memchr::{memchr, memchr2};
use pyo3::{
    IntoPyObjectExt,
    prelude::*,
    types::{PyDict, PyFrozenSet, PyList, PySet, PyTuple},
};
use rustc_hash::FxHashMap;
use saphyr_parser::{Event, Parser, ScalarStyle, ScanError, Tag};

use crate::{
    YAMLDecodeError,
    load::{
        arena::Arena,
        options::{AliasLimits, DuplicateKeyPolicy},
        parse_datetime::parse_py_datetime,
        scalar::{
            is_bool, is_datetime, is_float, is_inf_nan, is_int, is_null, parse_float, parse_int,
        },
        types::NodeId,
        value::Value,
    },
};

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
                "str" => Value::TaggedString(value),
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
            && is_float(bytes)
            && let Some(float) = parse_float(str)
        {
            return Ok(arena.push(Value::Float(float)));
        }

        if is_int(bytes)
            && let Some(int) = parse_int(str)
        {
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
                let node = if let Some(target) = anchors.get(&id).copied() {
                    arena.push(Value::Alias {
                        target,
                        anchor_id: id,
                    })
                } else {
                    arena.push(Value::Null)
                };

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

#[derive(Debug)]
struct AliasReplayState {
    limits: AliasLimits,
    total_replayed_events: usize,
    expansions_per_anchor: FxHashMap<usize, usize>,
    replayed_event_counts: FxHashMap<NodeId, usize>,
}

impl AliasReplayState {
    #[inline]
    fn new(limits: AliasLimits) -> Self {
        Self {
            limits,
            total_replayed_events: 0,
            expansions_per_anchor: FxHashMap::default(),
            replayed_event_counts: FxHashMap::default(),
        }
    }

    fn enter_alias(
        &mut self,
        arena: &Arena<'_>,
        target: NodeId,
        anchor_id: usize,
        depth: usize,
    ) -> PyResult<usize> {
        let next_depth = depth + 1;
        if next_depth > self.limits.max_replay_stack_depth {
            return Err(YAMLDecodeError::new_err(format!(
                "alias replay stack depth exceeded: depth {next_depth}, max {}",
                self.limits.max_replay_stack_depth,
            )));
        }

        let expansions = self.expansions_per_anchor.entry(anchor_id).or_insert(0);
        *expansions = expansions
            .checked_add(1)
            .ok_or_else(|| YAMLDecodeError::new_err("alias expansion counter overflow"))?;
        if *expansions > self.limits.max_alias_expansions_per_anchor {
            return Err(YAMLDecodeError::new_err(format!(
                "alias expansion limit exceeded for anchor {anchor_id}: expansions {}, max {}",
                *expansions, self.limits.max_alias_expansions_per_anchor,
            )));
        }

        let replayed_events = self.replayed_event_count(arena, target)?;
        self.total_replayed_events = self
            .total_replayed_events
            .checked_add(replayed_events)
            .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
        if self.total_replayed_events > self.limits.max_total_replayed_events {
            return Err(YAMLDecodeError::new_err(format!(
                "alias replay limit exceeded: replayed {}, max {}",
                self.total_replayed_events, self.limits.max_total_replayed_events,
            )));
        }

        Ok(next_depth)
    }

    fn replayed_event_count(&mut self, arena: &Arena<'_>, id: NodeId) -> PyResult<usize> {
        if let Some(&count) = self.replayed_event_counts.get(&id) {
            return Ok(count);
        }

        let count = match arena.get(id) {
            Value::Null
            | Value::Boolean(_)
            | Value::Integer64(_)
            | Value::BigInteger(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::TaggedString(_)
            | Value::Alias { .. } => 1usize,
            Value::Seq(items) => {
                let mut count = 2usize;
                for &child in items {
                    count = count
                        .checked_add(self.replayed_event_count(arena, child)?)
                        .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
                }
                count
            }
            Value::Map(pairs) => {
                let mut count = 2usize;
                for (key, value) in pairs {
                    count = count
                        .checked_add(self.replayed_event_count(arena, *key)?)
                        .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
                    count = count
                        .checked_add(self.replayed_event_count(arena, *value)?)
                        .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
                }
                count
            }
        };

        self.replayed_event_counts.insert(id, count);
        Ok(count)
    }
}

#[inline]
fn resolve_value<'a>(arena: &'a Arena<'a>, id: NodeId) -> &'a Value<'a> {
    match arena.get(id) {
        Value::Alias { target, .. } => resolve_value(arena, *target),
        value => value,
    }
}

#[inline]
fn duplicate_error(key: &Bound<'_, PyAny>) -> PyErr {
    match key.repr().and_then(|repr| repr.extract::<String>()) {
        Ok(key_repr) => YAMLDecodeError::new_err(format!("duplicate mapping key: {key_repr}")),
        Err(_) => YAMLDecodeError::new_err("duplicate mapping key"),
    }
}

fn value_to_py<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    id: NodeId,
    parse_datetime: bool,
    alias_state: &mut AliasReplayState,
    alias_depth: usize,
    duplicate_key_policy: DuplicateKeyPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Boolean(bool) => bool.into_bound_py_any(py),
        Value::Integer64(int_64) => int_64.into_bound_py_any(py),
        Value::BigInteger(big_int) => big_int.into_bound_py_any(py),
        Value::Float(float) => float.into_bound_py_any(py),
        Value::TaggedString(string_tagged) => string_tagged.into_bound_py_any(py),
        Value::Alias { target, anchor_id } => {
            let next_depth = alias_state.enter_alias(arena, *target, *anchor_id, alias_depth)?;
            value_to_py(
                py,
                arena,
                *target,
                parse_datetime,
                alias_state,
                next_depth,
                duplicate_key_policy,
            )
        }
        Value::String(string) => {
            let str = string.as_ref();
            if parse_datetime
                && is_datetime(str.as_bytes())
                && let Ok(Some(dt)) = parse_py_datetime(py, str)
            {
                return Ok(dt);
            }
            str.into_bound_py_any(py)
        }
        Value::Seq(items) => to_py_list(py, items, |child| {
            value_to_py(
                py,
                arena,
                child,
                parse_datetime,
                alias_state,
                alias_depth,
                duplicate_key_policy,
            )
        }),
        Value::Map(pairs) => {
            let mut all_nulls = true;
            let mut has_null_key = false;

            for (k, v) in pairs {
                if matches!(resolve_value(arena, *k), Value::Null) {
                    has_null_key = true;
                }
                if !matches!(resolve_value(arena, *v), Value::Null) {
                    all_nulls = false;
                }
            }

            if all_nulls && !has_null_key && pairs.len() > 1 {
                let py_set = PySet::empty(py)?;
                for (k, _) in pairs {
                    let py_key = value_to_hashable(
                        py,
                        arena,
                        *k,
                        parse_datetime,
                        alias_state,
                        alias_depth,
                        duplicate_key_policy,
                    )?;

                    if matches!(duplicate_key_policy, DuplicateKeyPolicy::Error)
                        && py_set.contains(&py_key)?
                    {
                        return Err(duplicate_error(&py_key));
                    }

                    py_set.add(py_key)?;
                }
                Ok(py_set.into_any())
            } else {
                let py_dict = PyDict::new(py);
                for (k, v) in pairs {
                    let py_key = value_to_hashable(
                        py,
                        arena,
                        *k,
                        parse_datetime,
                        alias_state,
                        alias_depth,
                        duplicate_key_policy,
                    )?;

                    if !matches!(duplicate_key_policy, DuplicateKeyPolicy::LastWins)
                        && py_dict.contains(&py_key)?
                    {
                        match duplicate_key_policy {
                            DuplicateKeyPolicy::Error => return Err(duplicate_error(&py_key)),
                            DuplicateKeyPolicy::FirstWins => continue,
                            DuplicateKeyPolicy::LastWins => {}
                        }
                    }

                    py_dict.set_item(
                        py_key,
                        value_to_py(
                            py,
                            arena,
                            *v,
                            parse_datetime,
                            alias_state,
                            alias_depth,
                            duplicate_key_policy,
                        )?,
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
    alias_state: &mut AliasReplayState,
    alias_depth: usize,
    duplicate_key_policy: DuplicateKeyPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    match arena.get(id) {
        Value::Alias { target, anchor_id } => {
            let next_depth = alias_state.enter_alias(arena, *target, *anchor_id, alias_depth)?;
            value_to_hashable(
                py,
                arena,
                *target,
                parse_datetime,
                alias_state,
                next_depth,
                duplicate_key_policy,
            )
        }
        Value::Seq(items) => {
            let mut vec = Vec::with_capacity(items.len());
            for &child in items {
                vec.push(value_to_hashable(
                    py,
                    arena,
                    child,
                    parse_datetime,
                    alias_state,
                    alias_depth,
                    duplicate_key_policy,
                )?);
            }
            PyTuple::new(py, &vec)?.into_bound_py_any(py)
        }
        Value::Map(pairs) => {
            let py_dict = PyDict::new(py);
            for (k, v) in pairs {
                let py_key = value_to_hashable(
                    py,
                    arena,
                    *k,
                    parse_datetime,
                    alias_state,
                    alias_depth,
                    duplicate_key_policy,
                )?;

                if !matches!(duplicate_key_policy, DuplicateKeyPolicy::LastWins)
                    && py_dict.contains(&py_key)?
                {
                    match duplicate_key_policy {
                        DuplicateKeyPolicy::Error => return Err(duplicate_error(&py_key)),
                        DuplicateKeyPolicy::FirstWins => continue,
                        DuplicateKeyPolicy::LastWins => {}
                    }
                }

                py_dict.set_item(
                    py_key,
                    value_to_py(
                        py,
                        arena,
                        *v,
                        parse_datetime,
                        alias_state,
                        alias_depth,
                        duplicate_key_policy,
                    )?,
                )?;
            }
            PyFrozenSet::new(py, py_dict.items())?.into_bound_py_any(py)
        }
        _ => value_to_py(
            py,
            arena,
            id,
            parse_datetime,
            alias_state,
            alias_depth,
            duplicate_key_policy,
        ),
    }
}

fn to_py_list<'py, T, F>(py: Python<'py>, items: &[T], mut f: F) -> PyResult<Bound<'py, PyAny>>
where
    T: Copy,
    F: FnMut(T) -> PyResult<Bound<'py, PyAny>>,
{
    let py_list = PyList::empty(py);
    for &item in items {
        py_list.append(f(item)?)?;
    }
    Ok(py_list.into_any())
}

pub(crate) fn to_python<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    docs: &[NodeId],
    parse_datetime: bool,
    alias_limits: AliasLimits,
    duplicate_key_policy: DuplicateKeyPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    let mut alias_state = AliasReplayState::new(alias_limits);

    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => value_to_py(
            py,
            arena,
            docs[0],
            parse_datetime,
            &mut alias_state,
            0,
            duplicate_key_policy,
        ),
        _ => to_py_list(py, docs, |doc| {
            value_to_py(
                py,
                arena,
                doc,
                parse_datetime,
                &mut alias_state,
                0,
                duplicate_key_policy,
            )
        }),
    }
}
