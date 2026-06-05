use std::{borrow::Cow, ptr};

use crate::load::{types::NodeId, value::Value};

#[derive(Debug)]
pub struct Arena<'a> {
    nodes: Vec<Value<'a>>,
    owned_strings: Vec<String>,
}

impl<'a> Arena<'a> {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(cap),
            owned_strings: Vec::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, value: Value<'a>) -> NodeId {
        let id = self.nodes.len() as NodeId;
        debug_assert!(id < NodeId::MAX, "Arena capacity exceeded");
        self.nodes.push(value);
        id
    }

    #[inline]
    pub fn push_intern(
        &mut self,
        cow: Cow<'a, str>,
        f: impl FnOnce(&'a str) -> Value<'a>,
    ) -> NodeId {
        let s: &'a str = match cow {
            Cow::Borrowed(s) => s,
            Cow::Owned(string) => {
                self.owned_strings.push(string);
                // SAFETY: `push` made the vector non-empty, and stored string buffers outlive nodes
                unsafe {
                    &*ptr::from_ref::<str>(self.owned_strings.last().unwrap_unchecked().as_str())
                }
            }
        };
        self.push(f(s))
    }

    #[inline]
    pub fn get(&self, id: NodeId) -> &Value<'_> {
        debug_assert!((id as usize) < self.nodes.len());
        // SAFETY: `id` is produced by `push` and nodes are never removed,
        // so it is always a valid index.
        unsafe { self.nodes.get_unchecked(id as usize) }
    }
}
