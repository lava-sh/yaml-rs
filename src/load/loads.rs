use std::borrow::Cow;

use granit_parser::{Event, Parser, ScalarStyle, ScanError, Tag};
use pyo3::{
    IntoPyObjectExt,
    prelude::*,
    types::{PyDict, PyFrozenSet, PyList, PySet, PyTuple},
};
use rustc_hash::FxHashMap;

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
        is_tagged_set: bool,
    },
}

#[derive(Debug)]
pub enum BuildError {
    Scan(ScanError),
    Decode(String),
}

impl From<ScanError> for BuildError {
    #[cold]
    fn from(err: ScanError) -> Self {
        Self::Scan(err)
    }
}

struct ScalarResolver<'a, 'arena> {
    arena: &'a mut Arena<'arena>,
}

impl<'arena> ScalarResolver<'_, 'arena> {
    fn resolve(
        &mut self,
        value: Cow<'arena, str>,
        style: ScalarStyle,
        tag: Option<&Tag>,
    ) -> Result<NodeId, String> {
        if let Some(tag) = tag {
            if tag.is_yaml_core_schema() {
                return self.resolve_core_tag(value, tag);
            }
            return Ok(self.arena.push_intern(value, Value::String));
        }

        Ok(self.resolve_plain(value, style))
    }

    fn resolve_core_tag(&mut self, value: Cow<'arena, str>, tag: &Tag) -> Result<NodeId, String> {
        let value = match tag.suffix.as_str() {
            "int" => parse_int(value.as_ref())
                .ok_or_else(|| format!("Invalid value '{value}' for '!!int' tag"))?,
            "float" => parse_float(value.as_ref())
                .map(Value::Float)
                .ok_or_else(|| format!("Invalid value '{value}' for '!!float' tag"))?,
            "bool" => is_bool(value.as_ref())
                .map(Value::Boolean)
                .ok_or_else(|| format!("Invalid value '{value}' for '!!bool' tag"))?,
            "null" => {
                if value.is_empty() || is_null(value.as_ref()) {
                    Value::Null
                } else {
                    return Err(format!("Invalid value '{value}' for '!!null' tag"));
                }
            }
            "binary" => return Ok(self.arena.push_intern(value, Value::String)),
            "str" => return Ok(self.arena.push_intern(value, Value::TaggedString)),
            _ => return Err(format!("Invalid tag: '!!{}'", tag.suffix)),
        };
        Ok(self.arena.push(value))
    }

    fn resolve_plain(&mut self, value: Cow<'arena, str>, style: ScalarStyle) -> NodeId {
        if style != ScalarStyle::Plain {
            return self.arena.push_intern(value, Value::String);
        }

        if value.is_empty() || is_null(value.as_ref()) {
            return self.arena.push(Value::Null);
        }

        if let Some(bool) = is_bool(value.as_ref()) {
            return self.arena.push(Value::Boolean(bool));
        }

        let bytes = value.as_bytes();

        if (is_inf_nan(bytes).is_some() || memchr::memchr3(b'.', b'e', b'E', bytes).is_some())
            && is_float(bytes)
            && let Some(float) = parse_float(value.as_ref())
        {
            return self.arena.push(Value::Float(float));
        }

        if is_int(bytes)
            && let Some(int) = parse_int(value.as_ref())
        {
            return self.arena.push(int);
        }

        self.arena.push_intern(value, Value::String)
    }
}

struct Builder<'arena> {
    arena: Arena<'arena>,
    stack: Vec<Frame>,
    docs: Vec<NodeId>,
    anchors: FxHashMap<usize, NodeId>,
    current_root: Option<NodeId>,
}

impl<'arena> Builder<'arena> {
    fn new(input: &str) -> Self {
        Self {
            arena: Arena::with_capacity((input.len() / 8).max(64)),
            stack: Vec::new(),
            docs: Vec::new(),
            anchors: FxHashMap::default(),
            current_root: None,
        }
    }

    fn push_value(&mut self, value: NodeId) {
        if let Some(top) = self.stack.last_mut() {
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
            self.current_root = Some(value);
        }
    }

    fn handle_event<'event: 'arena>(&mut self, event: Event<'event>) -> Result<(), BuildError> {
        match event {
            Event::DocumentStart(_) => {
                self.current_root = None;
                self.stack.clear();
            }
            Event::DocumentEnd => {
                let root = self
                    .current_root
                    .take()
                    .unwrap_or_else(|| self.arena.push(Value::Null));
                self.docs.push(root);
            }
            Event::Alias(id) => {
                let node = if let Some(target) = self.anchors.get(&id).copied() {
                    self.arena.push(Value::Alias {
                        target,
                        anchor_id: id,
                    })
                } else {
                    self.arena.push(Value::Null)
                };

                self.push_value(node);
            }
            Event::Scalar(val, style, anchor_id, tag) => {
                let mut resolver = ScalarResolver {
                    arena: &mut self.arena,
                };
                let node = resolver
                    .resolve(val, style, tag.as_deref())
                    .map_err(BuildError::Decode)?;

                if anchor_id != 0 {
                    self.anchors.insert(anchor_id, node);
                }
                self.push_value(node);
            }
            Event::SequenceStart(anchor_id, _) => {
                self.stack.push(Frame::Seq {
                    anchor: anchor_id,
                    items: Vec::new(),
                });
            }
            Event::SequenceEnd => {
                if let Some(Frame::Seq { anchor, items }) = self.stack.pop() {
                    let node = self.arena.push(Value::Seq(items));
                    if anchor != 0 {
                        self.anchors.insert(anchor, node);
                    }
                    self.push_value(node);
                }
            }
            Event::MappingStart(anchor_id, tag) => {
                let is_tagged_set = tag
                    .as_deref()
                    .is_some_and(|t| t.is_yaml_core_schema_tag("set"));

                self.stack.push(Frame::Map {
                    anchor: anchor_id,
                    items: Vec::new(),
                    pending_key: None,
                    is_tagged_set,
                });
            }
            Event::MappingEnd => {
                if let Some(Frame::Map {
                    anchor,
                    items,
                    is_tagged_set,
                    ..
                }) = self.stack.pop()
                {
                    let node = self.arena.push(Value::Map(items, is_tagged_set));
                    if anchor != 0 {
                        self.anchors.insert(anchor, node);
                    }
                    self.push_value(node);
                }
            }
            Event::StreamStart | Event::StreamEnd | Event::Nothing => {}
        }
        Ok(())
    }
}

pub fn build_from_events(input: &'_ str) -> Result<(Arena<'_>, Vec<NodeId>), BuildError> {
    let parser = Parser::new_from_str(input);
    let mut builder = Builder::new(input);

    for events in parser {
        let (event, _) = events?;
        builder.handle_event(event)?;
    }

    Ok((builder.arena, builder.docs))
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
        let next_depth = depth
            .checked_add(1)
            .ok_or_else(|| YAMLDecodeError::new_err("alias depth overflow"))?;

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

        let replayed_events = self.count_replayed_events(arena, target)?;
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

    fn count_replayed_events(&mut self, arena: &Arena<'_>, id: NodeId) -> PyResult<usize> {
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
            | Value::Alias { .. } => 1,
            Value::Seq(items) => {
                let mut count: usize = 2;
                for &child in items {
                    count = count
                        .checked_add(self.count_replayed_events(arena, child)?)
                        .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
                }
                count
            }
            Value::Map(pairs, _) => {
                let mut count: usize = 2;
                for (key, value) in pairs {
                    count = count
                        .checked_add(self.count_replayed_events(arena, *key)?)
                        .ok_or_else(|| YAMLDecodeError::new_err("alias replay counter overflow"))?;
                    count = count
                        .checked_add(self.count_replayed_events(arena, *value)?)
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
fn duplicate_error(key: &Bound<'_, PyAny>) -> PyErr {
    match key.repr().and_then(|repr| repr.extract::<String>()) {
        Ok(key_repr) => YAMLDecodeError::new_err(format!("duplicate mapping key: {key_repr}")),
        Err(_) => YAMLDecodeError::new_err("duplicate mapping key"),
    }
}

struct PyConverter<'py, 'arena> {
    py: Python<'py>,
    arena: &'arena Arena<'arena>,
    parse_datetime: bool,
    duplicate_key_policy: DuplicateKeyPolicy,
}

impl<'py> PyConverter<'py, '_> {
    fn convert_node(
        &mut self,
        id: NodeId,
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.arena.get(id) {
            Value::Null => Ok(self.py.None().into_bound(self.py)),
            Value::Boolean(bool) => bool.into_bound_py_any(self.py),
            Value::Integer64(int_64) => int_64.into_bound_py_any(self.py),
            Value::BigInteger(big_int) => big_int.into_bound_py_any(self.py),
            Value::Float(float) => float.into_bound_py_any(self.py),
            Value::TaggedString(tagged_string) => (*tagged_string).into_bound_py_any(self.py),
            Value::Alias { target, anchor_id } => {
                let next_depth =
                    alias_state.enter_alias(self.arena, *target, *anchor_id, alias_depth)?;
                self.convert_node(*target, alias_state, next_depth)
            }
            Value::String(string) => {
                if self.parse_datetime
                    && is_datetime(string.as_bytes())
                    && let Ok(Some(dt)) = parse_py_datetime(self.py, string)
                {
                    return Ok(dt);
                }
                (*string).into_bound_py_any(self.py)
            }
            Value::Seq(items) => self.convert_seq(items, alias_state, alias_depth),
            Value::Map(pairs, is_tagged_set) => {
                self.convert_map(pairs, *is_tagged_set, alias_state, alias_depth)
            }
        }
    }

    fn convert_seq(
        &mut self,
        items: &[NodeId],
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py_list = PyList::empty(self.py);
        for &item in items {
            py_list.append(self.convert_node(item, alias_state, alias_depth)?)?;
        }
        Ok(py_list.into_any())
    }

    fn convert_map(
        &mut self,
        pairs: &[(NodeId, NodeId)],
        is_tagged_set: bool,
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        if is_tagged_set {
            self.convert_to_set(pairs, alias_state, alias_depth)
        } else {
            self.convert_to_dict(pairs, alias_state, alias_depth)
        }
    }

    fn convert_to_set(
        &mut self,
        pairs: &[(NodeId, NodeId)],
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py_set = PySet::empty(self.py)?;
        for (k, _) in pairs {
            let py_key = self.convert_to_hashable(*k, alias_state, alias_depth)?;
            if matches!(self.duplicate_key_policy, DuplicateKeyPolicy::Error)
                && py_set.contains(&py_key)?
            {
                return Err(duplicate_error(&py_key));
            }
            py_set.add(py_key)?;
        }
        Ok(py_set.into_any())
    }

    #[inline]
    fn apply_duplicate_key_policy<F>(&self, f: F, py_key: &Bound<'_, PyAny>) -> PyResult<bool>
    where
        F: FnOnce(&Bound<'_, PyAny>) -> PyResult<bool>,
    {
        match self.duplicate_key_policy {
            DuplicateKeyPolicy::FirstWins => Ok(!f(py_key)?),
            DuplicateKeyPolicy::Error if f(py_key)? => Err(duplicate_error(py_key)),
            DuplicateKeyPolicy::LastWins | DuplicateKeyPolicy::Error => Ok(true),
        }
    }

    fn convert_to_dict(
        &mut self,
        pairs: &[(NodeId, NodeId)],
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py_dict = PyDict::new(self.py);
        for (k, v) in pairs {
            let py_key = self.convert_to_hashable(*k, alias_state, alias_depth)?;

            if self.apply_duplicate_key_policy(|k| py_dict.contains(k), &py_key)? {
                py_dict.set_item(py_key, self.convert_node(*v, alias_state, alias_depth)?)?;
            }
        }
        Ok(py_dict.into_any())
    }

    fn convert_to_hashable(
        &mut self,
        id: NodeId,
        alias_state: &mut AliasReplayState,
        alias_depth: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.arena.get(id) {
            Value::Alias { target, anchor_id } => {
                let next_depth =
                    alias_state.enter_alias(self.arena, *target, *anchor_id, alias_depth)?;
                self.convert_to_hashable(*target, alias_state, next_depth)
            }
            Value::Seq(items) => {
                let mut vec = Vec::with_capacity(items.len());
                for &child in items {
                    vec.push(self.convert_to_hashable(child, alias_state, alias_depth)?);
                }
                PyTuple::new(self.py, &vec)?.into_bound_py_any(self.py)
            }
            Value::Map(pairs, _) => {
                let py_dict = PyDict::new(self.py);
                for (k, v) in pairs {
                    let py_key = self.convert_to_hashable(*k, alias_state, alias_depth)?;

                    if self.apply_duplicate_key_policy(|k| py_dict.contains(k), &py_key)? {
                        py_dict
                            .set_item(py_key, self.convert_node(*v, alias_state, alias_depth)?)?;
                    }
                }
                PyFrozenSet::new(self.py, py_dict.items())?.into_bound_py_any(self.py)
            }
            _ => self.convert_node(id, alias_state, alias_depth),
        }
    }
}

pub fn to_python<'py>(
    py: Python<'py>,
    arena: &Arena<'_>,
    docs: &[NodeId],
    parse_datetime: bool,
    alias_limits: AliasLimits,
    duplicate_key_policy: DuplicateKeyPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    let mut limits = AliasReplayState::new(alias_limits);
    let mut converter = PyConverter {
        py,
        arena,
        parse_datetime,
        duplicate_key_policy,
    };

    match docs.len() {
        0 => Ok(py.None().into_bound(py)),
        1 => converter.convert_node(docs[0], &mut limits, 0),
        _ => {
            let py_list = PyList::empty(py);
            for &doc in docs {
                py_list.append(converter.convert_node(doc, &mut limits, 0)?)?;
            }
            Ok(py_list.into_any())
        }
    }
}
