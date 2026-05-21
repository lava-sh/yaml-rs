use std::{borrow::Cow, ptr};

use crate::load::{types::NodeId, value::Value};

#[derive(Debug)]
pub struct Arena<'a> {
    nodes: Vec<Value<'a>>,
    owned_strings: Vec<String>,
}

impl<'a> Arena<'a> {
    #[inline]
    pub fn with_capacity(c: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(c),
            owned_strings: Vec::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, value: Value<'a>) -> NodeId {
        let id = self.nodes.len() as u32;
        debug_assert!(id < u32::MAX, "Arena capacity exceeded");
        self.nodes.push(value);
        id
    }

    #[inline]
    pub fn get(&self, id: NodeId) -> &Value<'_> {
        // SAFETY: `id` is produced by `push` and nodes are never removed,
        // so it is always a valid index.
        unsafe { self.nodes.get_unchecked(id as usize) }
    }

    #[inline]
    pub fn intern(&mut self, cow: Cow<'a, str>) -> &'a str {
        match cow {
            Cow::Borrowed(str) => str,
            Cow::Owned(string) => {
                self.owned_strings.push(string);
                // SAFETY: We just pushed the string to owned_strings, so it has a stable address
                // for the lifetime of the arena.
                unsafe { &*ptr::from_ref::<str>(self.owned_strings.last().unwrap().as_str()) }
            }
        }
    }
}
