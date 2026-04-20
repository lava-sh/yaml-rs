use crate::load::{types::NodeId, value::Value};

#[derive(Debug)]
pub(crate) struct Arena<'a> {
    nodes: Vec<Value<'a>>,
}

impl<'a> Arena<'a> {
    #[inline]
    pub(crate) fn with_capacity(c: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(c),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, value: Value<'a>) -> NodeId {
        let id = self.nodes.len() as u32;
        debug_assert!(id < u32::MAX, "Arena capacity exceeded");
        self.nodes.push(value);
        id
    }

    #[inline]
    pub(crate) fn get(&self, id: NodeId) -> &Value<'_> {
        // SAFETY: `id` is produced by `push` and nodes are never removed,
        // so it is always a valid index.
        unsafe { self.nodes.get_unchecked(id as usize) }
    }
}
